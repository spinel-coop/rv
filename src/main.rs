use std::{fs, path::PathBuf};

use clap::{Parser, Subcommand};
use miette::{IntoDiagnostic, Result};

pub mod commands;
pub mod config;
pub mod ruby;

use commands::ruby::{RubyArgs, RubyCommand, list_rubies};
use config::Config;
use vfs::{AltrootFS, FileSystem};

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

    /// Root directory for testing (hidden)
    #[arg(long, hide = true)]
    test_root: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

impl Cli {
    fn config(&self) -> Config {
        use vfs::{PhysicalFS, VfsPath};

        let root: VfsPath = if let Some(ref root) = self.test_root {
            AltrootFS::new(
                VfsPath::new(PhysicalFS::new("/"))
                    .join(root.as_os_str().to_str().unwrap())
                    .unwrap(),
            )
            .into()
        } else {
            PhysicalFS::new("/").into()
        };

        Config {
            ruby_dirs: if self.ruby_dir.is_empty() {
                config::default_ruby_dirs(&root)
            } else {
                let root = VfsPath::new(PhysicalFS::new("/"));
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
            root,
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
    println!("{ruby_version}");
    Ok(())
}
