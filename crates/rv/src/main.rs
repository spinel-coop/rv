use anstream::stream::IsTerminal;
use camino::{FromPathBufError, Utf8PathBuf};
use clap::builder::Styles;
use clap::builder::styling::AnsiColor;
use clap::{ArgAction, CommandFactory, Parser, Subcommand};
use config::Config;
use miette::Report;
use rv_cache::CacheArgs;
use tokio::main;
use tracing_indicatif::IndicatifLayer;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt as _, util::SubscriberInitExt as _};

pub mod commands;
pub mod config;

use crate::commands::cache::{CacheCommand, CacheCommandArgs, cache_clean, cache_dir};
use crate::commands::ci::{CiArgs, ci};
use crate::commands::ruby::dir::dir as ruby_dir;
use crate::commands::ruby::find::find as ruby_find;
use crate::commands::ruby::install::install as ruby_install;
use crate::commands::ruby::list::list as ruby_list;
use crate::commands::ruby::pin::pin as ruby_pin;
#[cfg(unix)]
use crate::commands::ruby::run::run as ruby_run;
use crate::commands::ruby::uninstall::uninstall as ruby_uninstall;
use crate::commands::ruby::{RubyArgs, RubyCommand};
use crate::commands::shell::completions::shell_completions;
use crate::commands::shell::env::env as shell_env;
use crate::commands::shell::init::init as shell_init;
use crate::commands::shell::{ShellArgs, ShellCommand};

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
#[command(disable_help_flag = true)]
struct Cli {
    /// Ruby directories to search for installations
    #[arg(long = "ruby-dir")]
    ruby_dir: Vec<Utf8PathBuf>,

    #[arg(long = "project-dir")]
    project_dir: Option<Utf8PathBuf>,

    /// Path to Gemfile
    #[arg(long, env = "BUNDLE_GEMFILE")]
    gemfile: Option<Utf8PathBuf>,

    #[command(flatten)]
    verbose: clap_verbosity_flag::Verbosity<clap_verbosity_flag::InfoLevel>,

    // Override the help flag --help and -h to both show HelpShort
    #[arg(short = 'h', long = "help", action = ArgAction::HelpShort, global = true)]
    _help: Option<bool>,

    #[arg(long, env = "RV_COLOR")]
    color: Option<ColorMode>,

    #[command(flatten)]
    cache_args: CacheArgs,

    #[command(subcommand)]
    command: Option<Commands>,

    /// Root directory for testing (hidden)
    #[arg(long, hide = true, env = "RV_ROOT_DIR")]
    root_dir: Option<Utf8PathBuf>,

    /// Executable path for testing (hidden)
    #[arg(long, hide = true, env = "RV_TEST_EXE")]
    current_exe: Option<Utf8PathBuf>,
}

impl Cli {
    fn config(&self) -> Result<Config> {
        let root = if self.root_dir.is_some() {
            self.root_dir.clone().unwrap()
        } else {
            "/".into()
        };

        let current_dir: Utf8PathBuf = std::env::current_dir()?.try_into()?;
        let project_dir = if let Some(project_dir) = &self.project_dir {
            Some(project_dir.clone())
        } else {
            config::find_project_dir(current_dir.clone(), root.clone())
        };
        let ruby_dirs = if self.ruby_dir.is_empty() {
            config::default_ruby_dirs(&root)
        } else {
            self.ruby_dir
                .iter()
                .map(|path: &Utf8PathBuf| root.join(path))
                .collect()
        };
        let cache = self.cache_args.to_cache()?;
        let current_exe = if let Some(exe) = self.current_exe.clone() {
            exe
        } else {
            std::env::current_exe()?.to_str().unwrap().into()
        };

        Ok(Config {
            ruby_dirs,
            gemfile: self.gemfile.clone(),
            root,
            current_dir,
            project_dir,
            cache,
            current_exe,
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
    Shell(ShellArgs),
    #[command(about = "Clean install from a Gemfile.lock")]
    Ci(CiArgs),
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
    ConfigError(#[from] config::Error),
    #[error(transparent)]
    FindError(#[from] commands::ruby::find::Error),
    #[error(transparent)]
    PinError(#[from] commands::ruby::pin::Error),
    #[error(transparent)]
    ListError(#[from] commands::ruby::list::Error),
    #[error(transparent)]
    InstallError(#[from] commands::ruby::install::Error),
    #[error(transparent)]
    UninstallError(#[from] commands::ruby::uninstall::Error),
    #[error(transparent)]
    CiError(#[from] commands::ci::Error),
    #[cfg(unix)]
    #[error(transparent)]
    RunError(#[from] commands::ruby::run::Error),
    #[error(transparent)]
    NonUtf8Path(#[from] FromPathBufError),
    #[error(transparent)]
    InitError(#[from] commands::shell::init::Error),
    #[error(transparent)]
    EnvError(#[from] commands::shell::env::Error),
}

type Result<T> = miette::Result<T, Error>;

#[main]
async fn main() {
    if let Err(err) = run().await {
        let is_tty = std::io::stderr().is_terminal();
        if is_tty {
            eprintln!("{:?}", Report::new(err));
        } else {
            eprintln!("Error: {:?}", err);
        }
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
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

    if std::env::var("RV_DISABLE_INDICATIF").is_err() {
        reg.with(indicatif_layer).init();
    } else {
        reg.init();
    }

    let config = cli.config()?;

    match cli.command {
        None => {}
        Some(cmd) => match cmd {
            Commands::Ruby(ruby) => match ruby.command {
                RubyCommand::Find { request } => ruby_find(&config, &request)?,
                RubyCommand::List {
                    format,
                    installed_only,
                } => ruby_list(&config, format, installed_only).await?,
                RubyCommand::Pin { version_request } => ruby_pin(&config, version_request)?,
                RubyCommand::Dir => ruby_dir(&config),
                RubyCommand::Install {
                    version,
                    install_dir,
                } => ruby_install(&config, install_dir, version).await?,
                RubyCommand::Uninstall {
                    version: version_request,
                } => ruby_uninstall(&config, version_request).await?,
                #[cfg(unix)]
                RubyCommand::Run { version, args } => ruby_run(&config, &version, &args)?,
            },
            Commands::Ci(ci_args) => ci(&config, ci_args).await?,
            Commands::Cache(cache) => match cache.command {
                CacheCommand::Dir => cache_dir(&config)?,
                CacheCommand::Clean => cache_clean(&config)?,
            },
            Commands::Shell(shell) => match shell.command {
                ShellCommand::Init { shell } => shell_init(&config, shell)?,
                ShellCommand::Completions { shell } => {
                    shell_completions(&mut Cli::command(), shell)
                }
                ShellCommand::Env { shell } => shell_env(&config, shell)?,
            },
        },
    }

    Ok(())
}
