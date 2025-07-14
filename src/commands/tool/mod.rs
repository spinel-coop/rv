use clap::{Args, Subcommand};

pub mod run;
pub mod install;
pub mod uninstall;

pub use run::{run_tool, RunToolArgs};
pub use install::{install_tool, InstallToolArgs};
pub use uninstall::{uninstall_tool, UninstallToolArgs};

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