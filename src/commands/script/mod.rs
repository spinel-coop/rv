use clap::{Args, Subcommand};
use std::path::PathBuf;

pub mod add;
pub mod remove;
pub mod run;

pub use add::{AddScriptDependencyArgs, add_script_dependency};
pub use remove::{RemoveScriptDependencyArgs, remove_script_dependency};
pub use run::{RunScriptArgs, run_script};

#[derive(Args)]
pub struct ScriptArgs {
    #[command(subcommand)]
    pub command: ScriptCommand,
}

#[derive(Subcommand)]
pub enum ScriptCommand {
    #[command(about = "Run a Ruby script with automatic dependency resolution")]
    Run {
        /// Path to the Ruby script
        script: PathBuf,

        /// Arguments to pass to the script
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    #[command(about = "Add a dependency for script execution")]
    Add {
        /// Gem name to add
        gem: String,

        /// Gem version requirement
        #[arg(long)]
        version: Option<String>,

        /// Script this dependency is for
        #[arg(long)]
        script: Option<PathBuf>,
    },

    #[command(about = "Remove a dependency for script execution")]
    Remove {
        /// Gem name to remove
        gem: String,

        /// Script this dependency is for
        #[arg(long)]
        script: Option<PathBuf>,
    },
}
