use anstream::stream::IsTerminal;
use camino::{FromPathBufError, Utf8PathBuf};
use clap::builder::Styles;
use clap::builder::styling::AnsiColor;
use clap::{Parser, Subcommand};
use config::Config;
use rv_cache::CacheArgs;
use tokio::main;
use tracing_indicatif::IndicatifLayer;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt as _, util::SubscriberInitExt as _};

pub mod commands;
pub mod config;

use crate::commands::cache::{CacheCommand, CacheCommandArgs, cache_clean, cache_dir};
use crate::commands::ruby::install::install as ruby_install;
use crate::commands::ruby::list::list as ruby_list;
use crate::commands::ruby::pin::pin as ruby_pin;
use crate::commands::ruby::{RubyArgs, RubyCommand};

const STYLES: Styles = Styles::styled()
    .header(AnsiColor::Green.on_default().bold())
    .usage(AnsiColor::Green.on_default().bold())
    .literal(AnsiColor::Cyan.on_default().bold())
    .placeholder(AnsiColor::Cyan.on_default());

/// An extremely fast Ruby version manager.
#[derive(Parser)]
#[command(about)]
#[command(arg_required_else_help = true)]
#[command(long_about = None)]
#[command(name = "rv")]
#[command(styles=STYLES)]
#[command(version)]
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

    #[command(flatten)]
    cache_args: CacheArgs,

    #[command(subcommand)]
    command: Option<Commands>,
}

impl Cli {
    fn config(&self) -> Result<Config> {
        let root = if self.root_dir.is_some() {
            self.root_dir.clone().unwrap()
        } else {
            Utf8PathBuf::from("/")
        };

        let current_dir: Utf8PathBuf = std::env::current_dir()?.try_into()?;
        let project_dir = Some(current_dir.clone());
        let ruby_dirs = if self.ruby_dir.is_empty() {
            config::default_ruby_dirs(&root)
        } else {
            self.ruby_dir
                .iter()
                .map(|path: &Utf8PathBuf| root.join(path))
                .collect()
        };
        let cache = self.cache_args.to_cache()?;

        Ok(Config {
            ruby_dirs,
            gemfile: self.gemfile.clone(),
            root,
            current_dir,
            project_dir,
            cache,
        })
    }
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Manage Ruby versions and installations")]
    Ruby(RubyArgs),
    #[command(about = "Manage rv's cache")]
    Cache(CacheCommandArgs),
    #[command(about = "Configure your shell to use rv")]
    Init,
    #[command(hide = true)]
    Env,
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

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    FromEnvError(#[from] tracing_subscriber::filter::FromEnvError),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    PinError(#[from] commands::ruby::pin::Error),
    #[error(transparent)]
    ListError(#[from] commands::ruby::list::Error),
    #[error(transparent)]
    InstallError(#[from] commands::ruby::install::Error),
    #[error(transparent)]
    ConfigError(#[from] config::Error),
    #[error(transparent)]
    NonUtf8Path(#[from] FromPathBufError),
}

type Result<T> = miette::Result<T, Error>;

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
        .from_env()?;

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

    let config = cli.config()?;

    match cli.command {
        None => {}
        Some(cmd) => match cmd {
            Commands::Env => {
                let ruby = config.rubies().first().cloned();
                if let Some(ruby) = ruby {
                    print!(
                        concat!(
                            "export PATH={}:$PATH\n",
                            "export RUBY_ROOT={}\n",
                            "export RUBY_ENGINE={}\n",
                            "export RUBY_VERSION={}\n",
                        ),
                        ruby.bin_path(),
                        ruby.path,
                        ruby.engine,
                        ruby.version,
                    );
                } else {
                    eprintln!("No Ruby installations found.");
                }
            }
            Commands::Init => {
                print!(
                    concat!(
                        "autoload -U add-zsh-hook\n",
                        "_rv_autoload_hook () {{\n",
                        "    eval $({} env)\n",
                        "}}\n",
                        "add-zsh-hook chpwd _rv_autoload_hook\n",
                        "_rv_autoload_hook\n",
                    ),
                    std::env::current_exe()?.to_str().unwrap()
                );
            }
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
            Commands::Cache(cache) => match cache.command {
                CacheCommand::Dir => cache_dir(&config)?,
                CacheCommand::Clean => cache_clean(&config)?,
            },
        },
    }

    Ok(())
}
