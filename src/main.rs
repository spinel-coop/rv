use std::{fs, path::PathBuf};

use clap::{Parser, Subcommand};
use miette::{IntoDiagnostic, Result};

pub mod commands;
pub mod config;
pub mod ruby;

use commands::ruby::{RubyArgs, RubyCommand, list_rubies};
use config::Config;

const APP_PREFIX: &str = "rv";

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
        use std::sync::Arc;
        use vfs::{PhysicalFS, VfsPath};

        Config {
            ruby_dirs: if self.ruby_dir.is_empty() {
                config::default_ruby_dirs()
            } else {
                let fs = PhysicalFS::new("/");
                let root = VfsPath::new(fs);
                self.ruby_dir
                    .iter()
                    .filter_map(|path| root.join(path.to_string_lossy().as_ref()).ok())
                    .collect()
            },
            gemfile: self.gemfile.clone(),
            cache_dir: xdg::BaseDirectories::with_prefix(APP_PREFIX)
                .cache_home
                .unwrap_or_else(|| std::env::temp_dir().join(APP_PREFIX)),
            local_dir: xdg::BaseDirectories::with_prefix(APP_PREFIX)
                .data_home
                .unwrap_or_else(|| std::env::temp_dir().join(APP_PREFIX)),
            fs: Arc::new(PhysicalFS::new("/")),
        }
    }
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Manage Ruby versions and installations")]
    Ruby(RubyArgs),
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let config = cli.config();

    match cli.command {
        None => {}
        Some(cmd) => match cmd {
            Commands::Ruby(ruby) => match ruby.command {
                RubyCommand::List {
                    format,
                    installed_only,
                } => list_rubies(&config, format, installed_only)?,
                RubyCommand::Pin {} => pin_ruby()?,
            },
        },
    }

    Ok(())
}

fn pin_ruby() -> Result<()> {
    let ruby_version: String = fs::read_to_string(".ruby-version").into_diagnostic()?;
    println!("{}", ruby_version);
    Ok(())
}
