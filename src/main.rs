use std::{fs, path::PathBuf};

use clap::{Args, Parser, Subcommand};
use miette::{IntoDiagnostic, Result};

pub mod config;
pub mod env;
pub mod ruby;

use config::Config;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Ruby directories to search for installations
    #[arg(long = "ruby-dir")]
    ruby_dir: Vec<PathBuf>,

    /// Path to Gemfile
    #[arg(long, env = "BUNDLE_GEMFILE")]
    gemfile: Option<PathBuf>,

    /// Turn debugging information on
    #[arg(short, long, action = clap::ArgAction::Count)]
    debug: u8,

    #[command(subcommand)]
    command: Option<Commands>,
}

impl Cli {
    fn config(&self) -> Config {
        use vfs::{PhysicalFS, VfsPath};
        use std::sync::Arc;
        
        Config {
            ruby_dirs: if self.ruby_dir.is_empty() { 
                config::default_ruby_dirs() 
            } else { 
                let fs = PhysicalFS::new("/");
                let root = VfsPath::new(fs);
                self.ruby_dir.iter()
                    .filter_map(|path| root.join(path.to_string_lossy().as_ref()).ok())
                    .collect()
            },
            gemfile: self.gemfile.clone(),
            cache_dir: xdg::BaseDirectories::with_prefix("rv")
                .cache_home
                .unwrap_or_else(|| std::env::temp_dir().join("rv")),
            local_dir: xdg::BaseDirectories::with_prefix("rv")
                .data_home
                .unwrap_or_else(|| std::env::temp_dir().join("rv")),
            fs: Arc::new(PhysicalFS::new("/")),
        }
    }
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Manage Ruby versions and installations")]
    Ruby(RubyArgs),
}

#[derive(Args)]
struct RubyArgs {
    #[command(subcommand)]
    command: RubyCommand,
}

#[derive(Subcommand)]
enum RubyCommand {
    #[command(about = "List the available Ruby installations")]
    List {
        /// Output format for the Ruby list
        #[arg(long, value_enum, default_value = "text")]
        format: OutputFormat,
        
        /// Show only installed Ruby versions
        #[arg(long)]
        installed_only: bool,
    },
    Pin {},
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum OutputFormat {
    Text,
    Json,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let config = cli.config();
    
    match cli.command {
        None => {}
        Some(cmd) => match cmd {
            Commands::Ruby(ruby) => match ruby.command {
                RubyCommand::List { format, installed_only } => list_rubies(&config, format, installed_only)?,
                RubyCommand::Pin {} => pin_ruby()?,
            },
        },
    }

    Ok(())
}

fn list_rubies(config: &Config, format: OutputFormat, _installed_only: bool) -> Result<()> {
    let rubies = config.rubies()?;
    
    if rubies.is_empty() {
        println!("No Ruby installations found.");
        println!("Try installing Ruby with 'rv ruby install' or check your configuration.");
        return Ok(());
    }
    
    match format {
        OutputFormat::Text => {
            for ruby in rubies {
                let marker = if is_active_ruby(&ruby)? { "*" } else { " " };
                println!("{} {} {}", marker, ruby.display_name(), ruby.executable_path().display());
            }
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&rubies).into_diagnostic()?;
            println!("{}", json);
        }
    }
    
    Ok(())
}

fn is_active_ruby(_ruby: &ruby::Ruby) -> Result<bool> {
    // TODO: Implement active Ruby detection
    // 1. Check .ruby-version file in current directory
    // 2. Check global configuration
    // 3. Check PATH for currently active Ruby
    Ok(false)
}

fn pin_ruby() -> Result<()> {
    let ruby_version: String = fs::read_to_string(".ruby-version").into_diagnostic()?;
    println!("{}", ruby_version);
    Ok(())
}
