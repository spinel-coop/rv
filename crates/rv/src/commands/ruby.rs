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
    #[command(about = "List all installed Ruby versions")]
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
        version: Option<RubyRequest>,
    },

    #[command(about = "Show the directory where all Ruby versions are installed")]
    Dir,

    #[command(
        about = "Show the path to the Ruby executable for the pinned version or a specific version"
    )]
    Find {
        /// Ruby version to find
        version: Option<RubyRequest>,
    },

    #[command(about = "Install the pinned Ruby version or a specific version")]
    Install {
        /// Directory to install into
        #[arg(short, long, value_name = "DIR")]
        install_dir: Option<String>,

        /// Ruby version to install
        version: Option<RubyRequest>,

        /// Path to a local ruby tarball
        #[arg(long, value_name = "TARBALL_PATH")]
        tarball_path: Option<String>,
    },

    #[command(about = "Uninstall a specific Ruby version")]
    Uninstall {
        /// Ruby version to uninstall
        version: RubyRequest,
    },

    #[cfg(unix)]
    #[command(
        about = "Run Ruby with arguments, using the pinned version or a specific version",
        dont_delimit_trailing_values = true
    )]
    Run {
        /// By default, if your requested Ruby version isn't installed,
        /// it will be installed with `rv ruby install`'s default options.
        /// This option disables that behaviour.
        #[arg(long)]
        no_install: bool,

        /// Ruby version to run
        version: Option<RubyRequest>,

        /// Arguments passed to the `ruby` invocation
        #[arg(last = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}
