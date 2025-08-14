use anstream::stream::IsTerminal;
use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};
use config::Config;
use miette::{IntoDiagnostic, Result};
use tokio::main;
use tracing_indicatif::IndicatifLayer;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt as _, util::SubscriberInitExt as _};

pub mod commands;
pub mod config;

use crate::commands::ruby::install::install as ruby_install;
use crate::commands::ruby::list::list as ruby_list;
use crate::commands::ruby::pin::pin as ruby_pin;
use crate::commands::ruby::{RubyArgs, RubyCommand};

/// Next generation developer tooling for Ruby
#[derive(Parser)]
#[command(name = "rv", version, about, long_about = None, arg_required_else_help = true)]
struct Cli {
    /// Ruby directories to search for installations
    #[arg(long = "ruby-dir")]
    ruby_dir: Vec<Utf8PathBuf>,

    /// Path to Gemfile
    #[arg(long, env = "BUNDLE_GEMFILE")]
    gemfile: Option<Utf8PathBuf>,

    #[command(flatten)]
    verbose: clap_verbosity_flag::Verbosity<clap_verbosity_flag::InfoLevel>,

    /// Root directory for testing (hidden)
    #[arg(long, hide = true, env = "RV_ROOT_DIR")]
    root_dir: Option<Utf8PathBuf>,

    #[arg(long)]
    color: Option<ColorMode>,

    #[command(subcommand)]
    command: Option<Commands>,
}

impl Cli {
    fn config(&self) -> Config {
        let root = if self.root_dir.is_some() {
            self.root_dir.clone().unwrap()
        } else {
            Utf8PathBuf::from("/")
        };

        let current_dir = Utf8PathBuf::from(std::env::current_dir().unwrap().to_str().unwrap());
        let project_dir = Some(current_dir.clone());
        let ruby_dirs = if self.ruby_dir.is_empty() {
            config::default_ruby_dirs(&root)
        } else {
            self.ruby_dir.iter().map(|path| root.join(path)).collect()
        };

        Config {
            ruby_dirs,
            gemfile: self.gemfile.clone(),
            root,
            current_dir,
            project_dir,
        }
    }
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Manage Ruby versions and installations")]
    Ruby(RubyArgs),
}

#[derive(Debug, Copy, Clone, clap::ValueEnum)]
pub(crate) enum ColorMode {
    /// Use color output if the output supports it.
    Auto,
    /// Force color output, even if the output isn't a terminal.
    Always,
    /// Disable color output, even if the output is a compatible terminal.
    Never,
}

impl ColorMode {
    /// Returns a concrete (i.e. non-auto) `anstream::ColorChoice` for the given terminal.
    ///
    /// This is useful for passing to `anstream::AutoStream` when the underlying
    /// stream is something that is a terminal or should be treated as such,
    /// but can't be inferred due to type erasure (e.g. `Box<dyn Write>`).
    fn color_choice_for_terminal(&self, io: impl IsTerminal) -> anstream::ColorChoice {
        match self {
            ColorMode::Auto => {
                if io.is_terminal() {
                    anstream::ColorChoice::Always
                } else {
                    anstream::ColorChoice::Never
                }
            }
            ColorMode::Always => anstream::ColorChoice::Always,
            ColorMode::Never => anstream::ColorChoice::Never,
        }
    }
}

impl From<ColorMode> for anstream::ColorChoice {
    /// Maps `ColorMode` to `anstream::ColorChoice`.
    fn from(value: ColorMode) -> Self {
        match value {
            ColorMode::Auto => Self::Auto,
            ColorMode::Always => Self::Always,
            ColorMode::Never => Self::Never,
        }
    }
}

#[main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let indicatif_layer = IndicatifLayer::new();

    let color_mode = match cli.color {
        Some(color_mode) => color_mode,
        None => {
            // If `--color` wasn't specified, we first check a handful
            // of common environment variables, and then fall
            // back to `anstream`'s auto detection.
            if std::env::var("NO_COLOR").is_ok() {
                ColorMode::Never
            } else if std::env::var("FORCE_COLOR").is_ok()
                || std::env::var("CLICOLOR_FORCE").is_ok()
            {
                ColorMode::Always
            } else {
                ColorMode::Auto
            }
        }
    };

    anstream::ColorChoice::write_global(color_mode.into());

    let writer = std::sync::Mutex::new(anstream::AutoStream::new(
        Box::new(indicatif_layer.get_stderr_writer()) as Box<dyn std::io::Write + Send>,
        color_mode.color_choice_for_terminal(std::io::stderr()),
    ));

    let filter = EnvFilter::builder()
        .with_default_directive(cli.verbose.tracing_level_filter().into())
        .from_env()
        .into_diagnostic()?;

    let reg = tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .without_time()
                // NOTE: We don't need `with_ansi` here since our writer is
                // an `anstream::AutoStream` that handles color output for us.
                .with_writer(writer),
        )
        .with(filter);

    reg.with(indicatif_layer).init();

    let config = cli.config();

    match cli.command {
        None => {}
        Some(cmd) => match cmd {
            Commands::Ruby(ruby) => match ruby.command {
                RubyCommand::List {
                    format,
                    installed_only,
                } => ruby_list(&config, format, installed_only)?,
                RubyCommand::Pin { version_request } => ruby_pin(&config, version_request)?,
                RubyCommand::Install {
                    version,
                    install_dir,
                } => ruby_install(&config, install_dir, version).await?,
            },
        },
    }

    Ok(())
}
