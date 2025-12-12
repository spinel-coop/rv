use clap::{Args, Subcommand};

use crate::commands::ruby::list::OutputFormat;
use rv_ruby::request::RubyRequest;

pub mod dir;
pub mod find;
pub mod install;
pub mod list;
pub mod pin;
#[cfg(unix)]
pub mod run;
pub mod uninstall;

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

    #[command(about = "Show the Ruby installation directory")]
    Dir,

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

        /// Path to a local ruby tarball
        #[arg(long, value_name = "TARBALL_PATH")]
        tarball_path: Option<String>,

        /// Ruby version to install
        version: Option<RubyRequest>,
    },

    #[command(about = "Uninstall a Ruby version")]
    Uninstall {
        /// Ruby version to uninstall
        version: RubyRequest,
    },

    #[cfg(unix)]
    #[command(about = "Run a specific Ruby", dont_delimit_trailing_values = true)]
    Run {
        /// By default, if your requested Ruby version isn't installed,
        /// it will be installed with `rv ruby install`'s default options.
        /// This option disables that behaviour.
        #[arg(long)]
        no_install: bool,

        /// Ruby version to run
        version: RubyRequest,

        /// Arguments passed to the `ruby` invocation
        #[arg(last = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}
