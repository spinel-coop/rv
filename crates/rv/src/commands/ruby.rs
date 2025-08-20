use clap::{Args, Subcommand};

use crate::commands::ruby::list::OutputFormat;
use rv_ruby::request::RubyRequest;

pub mod find;
pub mod install;
pub mod list;
pub mod pin;

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

    #[command(about = "Show or set the Ruby version for the current project")]
    Pin {
        /// The Ruby version to pin
        version_request: Option<String>,
    },

    #[command(about = "Search for a Ruby installation")]
    Find {
        /// Ruby version to find
        request: Option<RubyRequest>,
    },

    #[command(about = "Install a Ruby version")]
    Install {
        /// Directory to install into
        #[arg(short, long, value_name = "DIR")]
        install_dir: Option<String>,

        /// Ruby version to install
        version: RubyRequest,
    },
}
