use clap::{Args, Subcommand};

pub mod install;
pub mod list;
pub mod pin;
pub mod uninstall;

pub use install::install_ruby;
pub use list::{OutputFormat, list_rubies};
pub use pin::pin_ruby;
pub use uninstall::uninstall_ruby;

#[derive(Args)]
pub struct RubyArgs {
    #[command(subcommand)]
    pub command: RubyCommand,
}

#[derive(Subcommand)]
pub enum RubyCommand {
    #[command(about = "List the available Ruby installations")]
    List {
        /// Output format for the Ruby list
        #[arg(long, value_enum, default_value = "text")]
        format: OutputFormat,

        /// Show only installed Ruby versions
        #[arg(long)]
        installed_only: bool,
    },

    #[command(about = "Install a Ruby version")]
    Install {
        /// Ruby version to install (e.g., "3.2.0", "jruby-9.4.0.0")
        version: Option<String>,

        /// Force reinstall if already installed
        #[arg(long)]
        force: bool,
    },

    #[command(about = "Uninstall a Ruby version")]
    Uninstall {
        /// Ruby version to uninstall
        version: String,
    },

    #[command(about = "Pin Ruby version for current project")]
    Pin {
        /// Ruby version to pin (if not provided, shows current pinned version)
        version: Option<String>,
    },
}
