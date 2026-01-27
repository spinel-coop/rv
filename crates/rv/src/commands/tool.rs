pub mod install;
pub mod list;
pub mod run;
pub mod uninstall;

use camino::Utf8PathBuf;
use clap::{Args, Subcommand};

use crate::output_format::OutputFormat;

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
    #[command(about = "Run a tool provided by a gem, installing it if necessary")]
    #[command(arg_required_else_help = true)]
    Run {
        /// What to run.
        /// Runs the executable with this name, from the gem with this name.
        /// To override the gem, use `--from othergem`.
        /// By default, uses the latest version of the gem. If you want to set
        /// a different version, add a suffix like `@1.2.0`, e.g. `nokogiri@1.2.0`.
        executable: String,
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

        /// Arguments passed to the tool you're running.
        #[arg(last = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

/// The directory where this tool can be found.
fn tool_dir_for(gem_name: &str, gem_version: &rv_version::Version) -> Utf8PathBuf {
    tool_dir().join(format!("{gem_name}@{gem_version}"))
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
