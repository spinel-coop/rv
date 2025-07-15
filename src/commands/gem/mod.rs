use clap::{Args, Subcommand};

pub mod build;
pub mod new;
pub mod publish;

pub use build::build_gem;
pub use new::new_gem;
pub use publish::publish_gem;

#[derive(Args)]
pub struct GemArgs {
    #[command(subcommand)]
    pub command: GemCommand,
}

#[derive(Subcommand)]
pub enum GemCommand {
    #[command(about = "Create a new gem")]
    New {
        /// Gem name
        name: String,

        /// Use specific template
        #[arg(long)]
        template: Option<String>,

        /// Skip git initialization
        #[arg(long)]
        skip_git: bool,
    },

    #[command(about = "Build gem package")]
    Build {
        /// Output directory for built gem
        #[arg(long)]
        output: Option<String>,
    },

    #[command(about = "Publish gem to registry")]
    Publish {
        /// Registry to publish to
        #[arg(long)]
        registry: Option<String>,

        /// Dry run - don't actually publish
        #[arg(long)]
        dry_run: bool,
    },
}
