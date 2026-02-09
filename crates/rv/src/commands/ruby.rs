use clap::{Args, Subcommand};

use crate::output_format::OutputFormat;
use rv_ruby::request::RubyRequest;

use crate::Config;

pub mod dir;
pub mod find;
pub mod install;
pub mod list;
pub mod pin;
pub mod run;
pub mod uninstall;

#[derive(Args)]
pub struct RubyArgs {
    #[command(subcommand)]
    pub command: RubyCommand,
}

#[derive(Subcommand)]
pub enum RubyCommand {
    #[command(about = "List all installed and available Ruby versions")]
    List {
        /// Output format for the Ruby list
        #[arg(long, value_enum, default_value = "text")]
        format: OutputFormat,

        #[command(flatten)]
        version_filter: list::VersionFilter,
    },

    #[command(about = "Show or set the Ruby version for the current project")]
    Pin {
        /// The Ruby version to pin
        version: Option<RubyRequest>,
    },

    #[command(about = "Show the directory where all Ruby versions are installed")]
    Dir,

    #[command(
        about = "Show the path to the Ruby executable for the pinned version or a specific version"
    )]
    Find {
        /// Ruby version to find
        version: Option<RubyRequest>,
    },

    #[command(about = "Install the pinned Ruby version or a specific version")]
    Install {
        /// Directory to install into
        #[arg(short, long, value_name = "DIR")]
        install_dir: Option<String>,

        /// Ruby version to install
        version: Option<RubyRequest>,

        /// Path to a local ruby tarball
        #[arg(long, value_name = "TARBALL_PATH")]
        tarball_path: Option<String>,
    },

    #[command(about = "Uninstall a specific Ruby version")]
    Uninstall {
        /// Ruby version to uninstall
        version: RubyRequest,
    },

    #[command(
        about = "Run Ruby with arguments, using the pinned version or a specific version",
        dont_delimit_trailing_values = true
    )]
    Run {
        /// By default, if your requested Ruby version isn't installed,
        /// it will be installed with `rv ruby install`'s default options.
        /// This option disables that behaviour.
        #[arg(long)]
        no_install: bool,

        /// Ruby version to run
        version: Option<RubyRequest>,

        /// Arguments passed to the `ruby` invocation
        #[arg(last = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    FindError(#[from] find::Error),
    #[error(transparent)]
    ListError(#[from] crate::commands::ruby::list::Error),
    #[error(transparent)]
    PinError(#[from] crate::commands::ruby::pin::Error),
    #[error(transparent)]
    InstallError(#[from] crate::commands::ruby::install::Error),
    #[error(transparent)]
    UninstallError(#[from] crate::commands::ruby::uninstall::Error),
    #[error(transparent)]
    RunError(#[from] crate::commands::ruby::run::Error),
}

type Result<T> = miette::Result<T, Error>;

pub async fn ruby(config: &Config, args: RubyArgs) -> Result<()> {
    match args.command {
        RubyCommand::Find { version } => find::find(config, version)?,
        RubyCommand::List {
            format,
            version_filter,
        } => list::list(config, format, version_filter).await?,
        RubyCommand::Pin { version } => pin::pin(config, version)?,
        RubyCommand::Dir => dir::dir(config),
        RubyCommand::Install {
            version,
            install_dir,
            tarball_path,
        } => install::install(config, install_dir, version, tarball_path).await?,
        RubyCommand::Uninstall { version } => uninstall::uninstall(config, version).await?,
        RubyCommand::Run {
            version,
            no_install,
            args,
        } => run::run(
            run::Invocation::ruby(vec![]),
            config,
            version,
            no_install,
            &args,
            Default::default(),
            Default::default(),
        )
        .await
        .map(|_| ())?,
    };

    Ok(())
}
