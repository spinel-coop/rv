use std::path::PathBuf;

use anstream::stream::IsTerminal;
use clap::{Parser, Subcommand};
use config::Config;
use miette::{IntoDiagnostic, Result};
use tokio::main;
use tracing_indicatif::IndicatifLayer;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt as _, util::SubscriberInitExt as _};
use vfs::AltrootFS;

pub mod commands;
pub mod config;
pub mod dirs;
pub mod ruby;

use crate::commands::ruby::list::list as ruby_list;
use crate::commands::ruby::pin::pin as ruby_pin;
use commands::ruby::{RubyArgs, RubyCommand};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Ruby directories to search for installations
    #[arg(long = "ruby-dir")]
    ruby_dir: Vec<PathBuf>,

    /// Path to Gemfile
    #[arg(long, env = "BUNDLE_GEMFILE")]
    gemfile: Option<PathBuf>,

    #[command(flatten)]
    verbose: clap_verbosity_flag::Verbosity<clap_verbosity_flag::InfoLevel>,

    /// Root directory for testing (hidden)
    #[arg(long, hide = true, env = "RV_ROOT_DIR")]
    root_dir: Option<PathBuf>,

    #[arg(long)]
    color: Option<ColorMode>,

    #[command(subcommand)]
    command: Option<Commands>,
}

impl Cli {
    fn config(&self) -> Config {
        use vfs::{PhysicalFS, VfsPath};

        let root: VfsPath = if let Some(ref root) = self.root_dir {
            AltrootFS::new(
                VfsPath::new(PhysicalFS::new("/"))
                    .join(root.as_os_str().to_str().unwrap())
                    .unwrap(),
            )
            .into()
        } else {
            PhysicalFS::new("/").into()
        };

        Config {
            ruby_dirs: if self.ruby_dir.is_empty() {
                config::default_ruby_dirs(&root)
            } else {
                let root = VfsPath::new(PhysicalFS::new("/"));
                self.ruby_dir
                    .iter()
                    .filter_map(|path| root.join(path.to_string_lossy().as_ref()).ok())
                    .collect()
            },
            gemfile: self.gemfile.clone(),
            root,
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
                RubyCommand::Pin { version } => ruby_pin(version)?,
            },
        },
    }

    Ok(())
}
