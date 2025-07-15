use std::path::PathBuf;

use clap::{Parser, Subcommand};
use miette::Result;

pub mod commands;
pub mod config;
pub mod ruby;

use commands::{
    app::{AppArgs, AppCommand},
    gem::{GemArgs, GemCommand},
    ruby::{RubyArgs, RubyCommand},
    script::{ScriptArgs, ScriptCommand},
    tool::{ToolArgs, ToolCommand},
};
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
        Config {
            ruby_dirs: if self.ruby_dir.is_empty() {
                config::default_ruby_dirs()
            } else {
                self.ruby_dir.clone()
            },
            gemfile: self.gemfile.clone(),
            cache_dir: xdg::BaseDirectories::with_prefix("rv")
                .cache_home
                .unwrap_or_else(|| std::env::temp_dir().join("rv")),
            local_dir: xdg::BaseDirectories::with_prefix("rv")
                .data_home
                .unwrap_or_else(|| std::env::temp_dir().join("rv")),
        }
    }
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Manage Ruby versions and installations")]
    Ruby(RubyArgs),

    #[command(about = "Manage gem CLI tools")]
    Tool(ToolArgs),

    #[command(about = "Run Ruby scripts with dependency resolution")]
    Script(ScriptArgs),

    #[command(about = "Manage Ruby applications")]
    App(AppArgs),

    #[command(about = "Create and publish gems")]
    Gem(GemArgs),
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let config = cli.config();

    match cli.command {
        None => {
            println!("rv - Ruby swiss army knife");
            println!("Run 'rv --help' for usage information");
        }
        Some(cmd) => match cmd {
            Commands::Ruby(ruby) => handle_ruby_command(&config, ruby.command)?,
            Commands::Tool(tool) => handle_tool_command(tool.command)?,
            Commands::Script(script) => handle_script_command(script.command)?,
            Commands::App(app) => handle_app_command(app.command)?,
            Commands::Gem(gem) => handle_gem_command(gem.command)?,
        },
    }

    Ok(())
}

fn handle_ruby_command(config: &Config, command: RubyCommand) -> Result<()> {
    use commands::ruby::*;

    match command {
        RubyCommand::List {
            format,
            installed_only,
        } => list_rubies(config, format, installed_only),
        RubyCommand::Install { version, force } => install_ruby(version.as_deref(), force),
        RubyCommand::Uninstall { version } => uninstall_ruby(&version),
        RubyCommand::Pin { version } => pin_ruby(version.as_deref()),
    }
}

fn handle_tool_command(command: ToolCommand) -> Result<()> {
    use commands::tool::*;

    match command {
        ToolCommand::Run { tool, args } => run_tool(RunToolArgs { tool, args }),
        ToolCommand::Install { tool, version } => install_tool(InstallToolArgs { tool, version }),
        ToolCommand::Uninstall { tool } => uninstall_tool(UninstallToolArgs { tool }),
    }
}

fn handle_script_command(command: ScriptCommand) -> Result<()> {
    use commands::script::*;

    match command {
        ScriptCommand::Run { script, args } => run_script(RunScriptArgs { script, args }),
        ScriptCommand::Add {
            gem,
            version,
            script,
        } => add_script_dependency(AddScriptDependencyArgs {
            gem,
            version,
            script,
        }),
        ScriptCommand::Remove { gem, script } => {
            remove_script_dependency(RemoveScriptDependencyArgs { gem, script })
        }
    }
}

fn handle_app_command(command: AppCommand) -> Result<()> {
    use commands::app::*;

    match command {
        AppCommand::Init {
            name,
            ruby,
            template,
        } => init_app(name.as_deref(), ruby.as_deref(), template.as_deref()),
        AppCommand::Install { skip_bundle } => install_app(skip_bundle),
        AppCommand::Add {
            gem,
            version,
            dev,
            test,
        } => add_gem(&gem, version.as_deref(), dev, test),
        AppCommand::Remove { gem } => remove_gem(&gem),
        AppCommand::Upgrade { gem } => upgrade_gems(gem.as_deref()),
        AppCommand::Tree { direct } => show_tree(direct),
    }
}

fn handle_gem_command(command: GemCommand) -> Result<()> {
    use commands::gem::*;

    match command {
        GemCommand::New {
            name,
            template,
            skip_git,
        } => new_gem(&name, template.as_deref(), skip_git),
        GemCommand::Build { output } => build_gem(output.as_deref()),
        GemCommand::Publish { registry, dry_run } => publish_gem(registry.as_deref(), dry_run),
    }
}
