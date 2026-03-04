use anstream::stream::IsTerminal;
use camino::Utf8PathBuf;
use clap::builder::Styles;
use clap::builder::styling::AnsiColor;
use clap::{ArgAction, CommandFactory, Parser, Subcommand};
use clap_verbosity_flag::tracing::LevelFilter;
use miette::Report;
use rv_cache::CacheArgs;
use tokio::main;
use tracing_indicatif::IndicatifLayer;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt as _, util::SubscriberInitExt as _};

pub mod commands;
pub mod config;
pub mod http_client;
pub mod output_format;
pub mod progress;
pub mod script_metadata;
pub mod update;

use crate::commands::cache::{CacheCommandArgs, cache};
use crate::commands::clean_install::{CleanInstallArgs, ci};
use crate::commands::ruby::{RubyArgs, ruby};
use crate::commands::run::{RunArgs, run};
use crate::commands::selfupdate::selfupdate;
use crate::commands::shell::{ShellArgs, shell};
use crate::commands::tool::{ToolArgs, tool};
use crate::update::update_if_needed;

const STYLES: Styles = Styles::styled()
    .header(AnsiColor::Green.on_default().bold())
    .usage(AnsiColor::Green.on_default().bold())
    .literal(AnsiColor::Cyan.on_default().bold())
    .placeholder(AnsiColor::Cyan.on_default());
const PROJECT_URL: &str = "https://rv.dev";
const SOFTWARE_VERSION: &str = env!("CARGO_PKG_VERSION");

struct GlobalArgs {
    /// Ruby directories to search for installations
    ruby_dir: Vec<Utf8PathBuf>,

    /// Cache related parameters
    cache_args: CacheArgs,

    /// Executable path for testing
    current_exe: Option<Utf8PathBuf>,
}

/// An extremely fast Ruby version manager.
#[derive(Parser)]
#[command(about)]
#[command(arg_required_else_help = true)]
#[command(long_about = None)]
#[command(name = "rv")]
#[command(styles=STYLES)]
#[command(version)]
#[command(disable_help_flag = true)]
#[command(after_help = {
    let header_style = AnsiColor::Green.on_default().bold();
    let value_style = AnsiColor::Cyan.on_default().bold();
    format!(
        "{header_style}Project URL:{header_style:#}      {value_style}{PROJECT_URL}{value_style:#}\n\
         {header_style}Software Version:{header_style:#} {value_style}{SOFTWARE_VERSION}{value_style:#}"
    )
})]
struct Cli {
    /// Ruby directories to search for installations
    #[cfg_attr(
        not(windows),
        arg(long = "ruby-dir", env = "RUBIES_PATH", value_delimiter = ':')
    )]
    #[cfg_attr(
        windows,
        arg(long = "ruby-dir", env = "RUBIES_PATH", value_delimiter = ';')
    )]
    ruby_dir: Vec<Utf8PathBuf>,

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
    command: Commands,

    /// Executable path for testing (hidden)
    #[arg(long, hide = true, env = "RV_TEST_EXE")]
    current_exe: Option<Utf8PathBuf>,
}

impl Cli {
    pub fn global_args(&self) -> GlobalArgs {
        GlobalArgs {
            ruby_dir: self.ruby_dir.clone(),
            cache_args: self.cache_args.clone(),
            current_exe: self.current_exe.clone(),
        }
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
    #[command(about = "Clean install from a Gemfile.lock", visible_alias = "ci")]
    CleanInstall(CleanInstallArgs),
    #[command(about = "Update rv itself, if an update is available")]
    Selfupdate,
    #[command(about = "Manage Ruby tools")]
    Tool(ToolArgs),
    #[command(
        about = "Run a command or script with Ruby",
        visible_alias = "r",
        dont_delimit_trailing_values = true
    )]
    Run(RunArgs),
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
    RubyError(#[from] commands::ruby::Error),
    #[error(transparent)]
    CiError(#[from] commands::clean_install::Error),
    #[error(transparent)]
    RunError(#[from] commands::ruby::run::Error),
    #[error(transparent)]
    ScriptRunError(#[from] commands::run::Error),
    #[error(transparent)]
    CacheError(#[from] commands::cache::Error),
    #[error(transparent)]
    SelfupdateError(#[from] commands::selfupdate::Error),
    #[error(transparent)]
    ShellError(#[from] commands::shell::Error),
    #[error(transparent)]
    ToolError(#[from] commands::tool::Error),
}

type Result<T> = miette::Result<T, Error>;

#[main]
async fn main() {
    if let Err(err) = main_inner().await {
        let is_tty = std::io::stderr().is_terminal();
        if is_tty {
            eprintln!("{:?}", Report::new(err));
        } else {
            eprintln!("Error: {:?}", err);
        }
        std::process::exit(1);
    }
}

async fn main_inner() -> Result<()> {
    let is_rvx = std::env::args().next().unwrap().ends_with("rvx");
    let cli = if is_rvx {
        let mut args = std::env::args().collect::<Vec<String>>();
        let rvx_args = ["rv", "tool", "run"].map(|s| s.to_string());
        args.splice(0..1, rvx_args);
        Cli::parse_from(args)
    } else {
        Cli::parse()
    };

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

    let global_level_filter = cli.verbose.tracing_level_filter();

    // the pubgrub crate is pretty noisy, it emits a lot of tracing::info spans when it's
    // resolving versions. So let's make it quieter, and make its log levels a bit less verbose
    let pubgrub_level_filter = match global_level_filter {
        LevelFilter::OFF => LevelFilter::OFF,
        LevelFilter::ERROR => LevelFilter::ERROR,
        LevelFilter::WARN => LevelFilter::WARN,
        LevelFilter::INFO => LevelFilter::WARN,
        LevelFilter::DEBUG => LevelFilter::INFO,
        LevelFilter::TRACE => LevelFilter::TRACE,
    };
    let h2_level_filter = LevelFilter::INFO;

    let filter = EnvFilter::builder()
        .with_default_directive(global_level_filter.into())
        .parse_lossy(format!(
            "{},pubgrub={},h2={}",
            global_level_filter, pubgrub_level_filter, h2_level_filter
        ));

    let reg = tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .without_time()
                // NOTE: We don't need `with_ansi` here since our writer is
                // an `anstream::AutoStream` that handles color output for us.
                .with_writer(writer),
        )
        .with(if cfg!(target_os = "macos") {
            Some(tracing_oslog::OsLogger::new("dev.rv.tracing", "default"))
        } else {
            None
        })
        .with(filter)
        .with(indicatif_layer);

    reg.init();

    update_if_needed().await;

    run_cmd(&cli.global_args(), cli.command).await
}

/// Run an `rv` subcommand.
/// This is like shelling out to `rv` except it reuses the current context
/// and doesn't need to start a new process.
async fn run_cmd(global_args: &GlobalArgs, command: Commands) -> Result<()> {
    match command {
        Commands::Ruby(ruby_args) => ruby(global_args, ruby_args).await?,
        Commands::CleanInstall(ci_args) => ci(global_args, ci_args).await?,
        Commands::Cache(cache_args) => cache(global_args, cache_args)?,
        Commands::Selfupdate => selfupdate(global_args).await?,
        Commands::Shell(shell_args) => shell(global_args, &mut Cli::command(), shell_args)?,
        Commands::Tool(tool_args) => tool(global_args, tool_args).await?,
        Commands::Run(run_args) => run(global_args, run_args).await?,
    };

    Ok(())
}
