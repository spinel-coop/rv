use clap::{Args, Subcommand};

pub mod install;
pub mod run;
pub mod uninstall;

pub use install::{InstallToolArgs, install_tool};
pub use run::{RunToolArgs, run_tool};
pub use uninstall::{UninstallToolArgs, uninstall_tool};

#[derive(Args)]
pub struct ToolArgs {
    #[command(subcommand)]
    pub command: ToolCommand,
}

#[derive(Subcommand)]
pub enum ToolCommand {
    #[command(about = "Run a tool command with automatic installation")]
    Run {
        /// Tool name to run
        tool: String,

        /// Arguments to pass to the tool
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    #[command(about = "Install a tool globally")]
    Install {
        /// Tool name to install
        tool: String,

        /// Specific version to install
        #[arg(long)]
        version: Option<String>,
    },

    #[command(about = "Uninstall a global tool")]
    Uninstall {
        /// Tool name to uninstall
        tool: String,
    },
}
