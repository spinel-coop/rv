pub mod install;
pub mod list;
pub mod run;
pub mod uninstall;

use camino::Utf8PathBuf;
use clap::{Args, Subcommand};

use crate::{GlobalArgs, commands::tool, output_format::OutputFormat};

#[derive(Args)]
pub struct ToolArgs {
    #[command(subcommand)]
    pub command: ToolCommand,
}

#[derive(Subcommand)]
pub enum ToolCommand {
    #[command(about = "Install a gem as a CLI tool, with its own dedicated environment")]
    Install {
        /// What to install. This can either be gem@version, e.g.
        /// `mygem@2.18.0`, or a gem name like `mygem`, which is equivalent
        /// to doing `mygem@latest`.
        gem: String,
        /// What gem server to use.
        #[arg(long, default_value = "https://gem.coop/")]
        gem_server: String,
        /// If true, and the tool is already installed, reinstall it.
        /// Otherwise, skip installing if the tool was already installed.
        #[arg(long, short)]
        force: bool,
    },
    #[command(about = "List installed tools")]
    List {
        /// Output format for the list
        #[arg(long, value_enum, default_value = "text")]
        format: OutputFormat,
    },
    #[command(about = "Remove an installed tool")]
    Uninstall {
        /// What to uninstall
        gem: String,
    },
    /// Run a command provided by a gem, installing it if necessary.
    ///
    /// By default, the gem name is assumed to match the command name.
    ///
    /// The name of the gem can include an exact version in the format `<package>@<version>`, e.g., `rv tool run rails@8.1.2`. If the command is provided by a different gem, use `--from`.
    #[command(about = "Run a command from a gem, installing it if necessary")]
    #[command(arg_required_else_help = true)]
    Run {
        /// Which gem to run the executable from.
        /// If not given, assumes the gem name is the same as the executable name.
        #[arg(long = "from")]
        gem: Option<String>,
        /// What gem server to use, if the tool needs to be installed.
        #[arg(long, default_value = "https://gem.coop/")]
        gem_server: String,
        /// By default, if the tool isn't installed, rv will install it.
        /// If this flag is given, rv will exit with an error instead of installing.
        #[arg(long)]
        no_install: bool,
        /// Command to run, e.g. `rerun` or `rails@8.0.2 new .`
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true, value_names = ["COMMAND", "ARGS"])]
        args: Vec<String>,
    },
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    ToolInstallError(#[from] tool::install::Error),
    #[error(transparent)]
    ToolListError(#[from] tool::list::Error),
    #[error(transparent)]
    ToolUninstallError(#[from] tool::uninstall::Error),
    #[error(transparent)]
    ToolRunError(#[from] tool::run::Error),
}

type Result<T> = miette::Result<T, Error>;

pub(crate) async fn tool(global_args: &GlobalArgs, tool_args: ToolArgs) -> Result<()> {
    match tool_args.command {
        ToolCommand::Install {
            gem,
            gem_server,
            force,
        } => install::install(global_args, gem, gem_server, force)
            .await
            .map(|_| ())?,
        ToolCommand::List { format } => list::list(global_args, format)?,
        ToolCommand::Uninstall { gem } => uninstall::uninstall(global_args, gem)?,
        ToolCommand::Run {
            gem,
            gem_server,
            no_install,
            args,
        } => run::run(global_args, gem, gem_server, no_install, args).await?,
    };

    Ok(())
}

/// The directory where this tool can be found.
fn tool_dir_for(gem_name: &str, gem_release: &str) -> Utf8PathBuf {
    tool_dir().join(format!("{gem_name}@{gem_release}"))
}

/// The directory where this tool can be found.
fn tool_dir() -> Utf8PathBuf {
    rv_dirs::user_state_dir("/".into()).join("tools")
}

/// Describes a successful installation of a tool.
#[derive(Debug)]
pub struct Installed {
    /// Which version was installed.
    pub version: rv_version::Version,
    /// The dir where the tool/gem was installed.
    pub dir: Utf8PathBuf,
}
