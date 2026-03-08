use camino::Utf8PathBuf;
use clap::{Args, Subcommand};
use indoc::formatdoc;
use owo_colors::OwoColorize;

use crate::output_format::OutputFormat;
use rv_ruby::request::RubyRequest;

use crate::GlobalArgs;

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

        /// By default, the table view is colored.
        /// Set this to skip coloring.
        #[arg(long)]
        no_color: bool,
    },

    #[command(about = "Show or set the Ruby version for the current project")]
    Pin {
        /// The Ruby version to pin
        version: Option<String>,

        /// Write the resolved Ruby version instead of the request
        #[arg(long)]
        resolved: bool,
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

    #[command(
        about = "Install Ruby",
        after_help = {
            let header = "Examples:";
            let install_default = "rv ruby install";
            let install_latest = "rv ruby install latest";
            let install_fuzzy = "rv ruby install 3.4";
            let install_specific = "rv ruby install 4.0.1";
            let install_dev = "rv ruby install dev";
            let width = install_latest.len();
            formatdoc!(
                r#"
                    {}
                      {:width$}  # Install the latest Ruby release
                      {:width$}  # Install the latest Ruby release that matches a version
                      {:width$}  # Install a specific Ruby release
                      {:width$}  # Install the latest development version of Ruby
                      {:width$}  # Discover version to install from tool files (.ruby-version, .tool-versions, or Gemfile.lock)
                "#,
                header.green().bold(),
                install_latest.cyan(),
                install_fuzzy.cyan(),
                install_specific.cyan(),
                install_dev.cyan(),
                install_default.cyan(),
            )
        }

    )]
    Install {
        /// Directory to install into
        #[arg(short, long, value_name = "DIR")]
        install_dir: Option<String>,

        /// Ruby version to install
        version: Option<RubyRequest>,

        /// Path to a local ruby tarball
        #[arg(long, value_name = "TARBALL_PATH")]
        tarball_path: Option<Utf8PathBuf>,

        /// Overwrite an existing installed version.
        #[arg(long)]
        force: bool,
    },

    #[command(about = "Uninstall a specific Ruby version")]
    Uninstall {
        /// Ruby version to uninstall
        version: RubyRequest,
    },

    #[command(
        about = "Run Ruby with arguments, using the pinned version or a specific version",
        hide = true,
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
    DirError(#[from] crate::commands::ruby::dir::Error),
    #[error(transparent)]
    InstallError(#[from] crate::commands::ruby::install::Error),
    #[error(transparent)]
    UninstallError(#[from] crate::commands::ruby::uninstall::Error),
    #[error(transparent)]
    RunError(#[from] crate::commands::ruby::run::Error),
}

type Result<T> = miette::Result<T, Error>;

pub(crate) async fn ruby(global_args: &GlobalArgs, args: RubyArgs) -> Result<()> {
    match args.command {
        RubyCommand::Find { version } => find::find(global_args, version)?,
        RubyCommand::List {
            format,
            version_filter,
            no_color,
        } => list::list(global_args, format, version_filter, no_color).await?,
        RubyCommand::Pin { version, resolved } => pin::pin(global_args, version, resolved).await?,
        RubyCommand::Dir => dir::dir(global_args)?,
        RubyCommand::Install {
            version,
            install_dir,
            tarball_path,
            force,
        } => install::install(global_args, install_dir, version, tarball_path, force).await?,
        RubyCommand::Uninstall { version } => uninstall::uninstall(global_args, version).await?,
        RubyCommand::Run {
            version,
            no_install,
            args,
        } => {
            if env!("CARGO_PKG_VERSION_MINOR").parse::<u8>().unwrap() >= 7 {
                panic!("Remove this subcommand before releasing 0.7.0");
            };

            run::run(global_args, version, no_install, args)
                .await
                .map(|_| ())?
        }
    };

    Ok(())
}
