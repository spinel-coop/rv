pub mod install;
pub mod list;
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
    #[command(about = "Install a given gem as a tool")]
    Install {
        /// What to install
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
    #[command(about = "Remove tools")]
    Uninstall {
        /// What to uninstall
        gem: String,
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
