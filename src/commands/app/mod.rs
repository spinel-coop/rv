use clap::{Args, Subcommand};

pub mod add;
pub mod init;
pub mod install;
pub mod remove;
pub mod tree;
pub mod upgrade;

pub use add::add_gem;
pub use init::init_app;
pub use install::install_app;
pub use remove::remove_gem;
pub use tree::show_tree;
pub use upgrade::upgrade_gems;

#[derive(Args)]
pub struct AppArgs {
    #[command(subcommand)]
    pub command: AppCommand,
}

#[derive(Subcommand)]
pub enum AppCommand {
    #[command(about = "Initialize a new Ruby application")]
    Init {
        /// Application name
        name: Option<String>,

        /// Use specific Ruby version
        #[arg(long)]
        ruby: Option<String>,

        /// Application template
        #[arg(long)]
        template: Option<String>,
    },

    #[command(about = "Install application dependencies")]
    Install {
        /// Skip bundle install
        #[arg(long)]
        skip_bundle: bool,
    },

    #[command(about = "Add a gem to the application")]
    Add {
        /// Gem name to add
        gem: String,

        /// Gem version requirement
        #[arg(long)]
        version: Option<String>,

        /// Add to development group
        #[arg(long)]
        dev: bool,

        /// Add to test group  
        #[arg(long)]
        test: bool,
    },

    #[command(about = "Remove a gem from the application")]
    Remove {
        /// Gem name to remove
        gem: String,
    },

    #[command(about = "Upgrade application dependencies")]
    Upgrade {
        /// Specific gem to upgrade
        gem: Option<String>,
    },

    #[command(about = "Show dependency tree")]
    Tree {
        /// Show only direct dependencies
        #[arg(long)]
        direct: bool,
    },
}
