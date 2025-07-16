use std::{fs, path::PathBuf};

use clap::{Args, Parser, Subcommand};
use miette::{IntoDiagnostic, Result};

pub mod config;
pub mod env;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Optional name to operate on
    name: Option<String>,

    ruby_dir: Vec<PathBuf>,

    #[arg(env = "BUNDLE_GEMFILE")]
    gemfile: Option<PathBuf>,

    /// Turn debugging information on
    #[arg(short, long, action = clap::ArgAction::Count)]
    debug: u8,

    #[command(subcommand)]
    command: Option<Commands>,
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
    List {},
    Pin {},
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        None => {}
        Some(cmd) => match cmd {
            Commands::Ruby(ruby) => match ruby.command {
                RubyCommand::List {} => list_rubies()?,
                RubyCommand::Pin {} => pin_ruby()?,
            },
        },
    }

    Ok(())
}

fn list_rubies() -> Result<()> {
    Ok(())
}

fn pin_ruby() -> Result<()> {
    let ruby_version: String = fs::read_to_string(".ruby-version").into_diagnostic()?;
    println!("{ruby_version}");
    Ok(())
}
