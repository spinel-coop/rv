pub mod install;

use camino::Utf8PathBuf;
use clap::{Args, Subcommand};

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
}

/// The directory where this tool can be found.
fn tool_dir(gem_name: &str, gem_version: &rv_version::Version) -> Utf8PathBuf {
    rv_dirs::user_state_dir("/".into())
        .join("tools")
        .join(format!("{gem_name}-{gem_version}"))
}
