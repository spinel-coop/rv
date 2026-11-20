pub mod dir;
pub mod install;
pub mod list;
pub mod run;
pub mod uninstall;

use camino::Utf8PathBuf;
use clap::{Args, Subcommand};

use crate::{GlobalArgs, commands::tool, output_format::OutputFormat};

#[derive(Args)]
#[command(disable_help_subcommand = true)]
pub struct ToolArgs {
    #[command(subcommand)]
    pub command: ToolCommand,
}

#[derive(Args)]
pub struct InstallArgs {
    /// What to install. This can either be gem@version, e.g.
    /// `mygem@2.18.0`, or a gem name like `mygem`, which is equivalent
    /// to doing `mygem@latest`.
    #[arg()]
    gem: String,
    /// What gem server to use.
    #[arg(long, default_value = "https://gem.coop")]
    gem_server: String,
    /// If true, and the tool is already installed, reinstall it.
    /// Otherwise, skip installing if the tool was already installed.
    #[arg(long, short)]
    force: bool,
}

#[derive(Subcommand)]
pub enum ToolCommand {
    #[command(about = "Install a gem as a CLI tool, with its own dedicated environment")]
    Install(InstallArgs),
    #[command(about = "List installed tools")]
    List {
        /// Output format for the list
        #[arg(long, value_enum, default_value = "text")]
        format: OutputFormat,
    },
    #[command(about = "Remove an installed tool")]
    Uninstall {
        /// What to uninstall
        gem: String,
    },
    /// Run a command provided by a gem, installing it if necessary.
    ///
    /// By default, the gem name is assumed to match the command name.
    ///
    /// The name of the gem can include an exact version in the format `<package>@<version>`, e.g., `rv tool run rails@8.1.2`. If the command is provided by a different gem, use `--from`.
    #[command(about = "Run a command from a gem, installing it if necessary")]
    #[command(arg_required_else_help = true)]
    Run {
        /// Which gem to run the executable from.
        /// If not given, assumes the gem name is the same as the executable name.
        #[arg(long = "from")]
        gem: Option<String>,
        /// What gem server to use, if the tool needs to be installed.
        #[arg(long, default_value = "https://gem.coop")]
        gem_server: String,
        /// By default, if the tool isn't installed, rv will install it.
        /// If this flag is given, rv will exit with an error instead of installing.
        #[arg(long)]
        no_install: bool,
        /// Additional gems to install alongside the primary tool.
        #[arg(short = 'w', long, action = clap::ArgAction::Append)]
        with: Vec<String>,
        /// Command to run, e.g. `rerun` or `rails@8.0.2 new .`
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true, value_names = ["COMMAND", "ARGS"])]
        args: Vec<String>,
    },
    #[command(about = "Show the path to the rv tools directory")]
    Dir,
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    ToolInstallError(#[from] tool::install::Error),
    #[error(transparent)]
    ToolListError(#[from] tool::list::Error),
    #[error(transparent)]
    ToolUninstallError(#[from] tool::uninstall::Error),
    #[error(transparent)]
    ToolRunError(#[from] tool::run::Error),
    #[error(transparent)]
    ToolDirError(#[from] tool::dir::Error),
}

type Result<T> = miette::Result<T, Error>;

pub(crate) async fn tool(global_args: &GlobalArgs, tool_args: ToolArgs) -> Result<()> {
    match tool_args.command {
        ToolCommand::Install(args) => {
            let (gem_server, gem) = split_namespace(args.gem_server, args.gem);
            install::install(global_args, gem, gem_server, args.force)
                .await
                .map(|_| ())?
        }
        ToolCommand::List { format } => list::list(global_args, format)?,
        ToolCommand::Uninstall { gem } => uninstall::uninstall(global_args, gem)?,
        ToolCommand::Run {
            gem,
            gem_server,
            no_install,
            with,
            args,
        } => {
            let (gem_server, gem, args) = parse_namespace(gem_server, gem, args);
            run::run(global_args, Some(gem), gem_server, no_install, with, args).await?
        }
        ToolCommand::Dir => dir::dir(global_args)?,
    };

    Ok(())
}

// Normalize the gem server URL, the gem name, and the command to run.
// When BIN, return URL, BIN, [BIN, ...]
// When BIN --from GEM, return URL, GEM, [BIN, ...]
// When @NS/BIN, return URL/@NS, BIN, [BIN, ...]
// When @NS/BIN --from GEM, return URL/@NS, GEM, [BIN, ...]
// When BIN --from @NS/GEM, return URL/@NS, GEM, [BIN, ...]
fn parse_namespace(
    gem_server: String,
    gem: Option<String>,
    mut args: Vec<String>,
) -> (String, String, Vec<String>) {
    let gem_given = gem.is_some();
    let gem = gem
        .or_else(|| args.first().cloned())
        .expect("gem or first arg is required");
    let gem_server = gem_server.trim_end_matches("/").to_string();
    let (gem_server, gem) = split_namespace(gem_server, gem);
    // Without `--from`, the command name is also the gem name, so write the
    // namespace-stripped gem name back into args[0].
    // With `--from`, args[0] is the executable name and must be preserved even
    // when it differs from the gem name (e.g. `pod` provided by `cocoapods`).
    if !gem_given {
        args[0] = gem.clone();
    }
    (gem_server, gem, args)
}

fn split_namespace(gem_server: String, gem: String) -> (String, String) {
    if gem.starts_with('@')
        && let Some((namespace, inner_gem)) = gem.split_once('/')
    {
        let gem_server = [gem_server, namespace.to_string()].join("/");
        return (gem_server, inner_gem.to_string());
    }
    (gem_server, gem)
}

/// The directory where this tool can be found.
fn tool_dir_for(gem_name: &str, gem_release: &str) -> Utf8PathBuf {
    tool_dir().join(format!("{gem_name}@{gem_release}"))
}

/// The directory where this tool can be found.
fn tool_dir() -> Utf8PathBuf {
    rv_dirs::user_data_dir("/".into()).join("tools")
}

/// Describes a successful installation of a tool.
#[derive(Debug)]
pub struct Installed {
    /// Which version was installed.
    pub version: rv_version::Version,
    /// The dir where the tool/gem was installed.
    pub dir: Utf8PathBuf,
}

#[test]
fn test_split_namespace() {
    assert_eq!(
        ("https://gem.coop".to_string(), "indirect".to_string()),
        split_namespace("https://gem.coop".to_string(), "indirect".to_string())
    );
    assert_eq!(
        ("gem.coop/@namespace".to_string(), "gemname".to_string()),
        split_namespace("gem.coop".to_string(), "@namespace/gemname".to_string())
    );
    assert_eq!(
        ("gem.coop".to_string(), "gemname@latest".to_string()),
        split_namespace("gem.coop".to_string(), "gemname@latest".to_string())
    );
    assert_eq!(
        ("gem.coop".to_string(), "gem/name".to_string()),
        split_namespace("gem.coop".to_string(), "gem/name".to_string())
    );

    assert_eq!(
        ("gem.coop".to_string(), "@gemname".to_string()),
        split_namespace("gem.coop".to_string(), "@gemname".to_string())
    );
    assert_eq!(
        ("gem.coop/@".to_string(), "gemname".to_string()),
        split_namespace("gem.coop".to_string(), "@/gemname".to_string())
    );
    assert_eq!(
        ("gem.coop/@namespace".to_string(), "gem/name".to_string()),
        split_namespace("gem.coop".to_string(), "@namespace/gem/name".to_string())
    );
    assert_eq!(
        ("".to_string(), "".to_string()),
        split_namespace("".to_string(), "".to_string())
    );
    assert_eq!(
        (
            "gem.coop/@namespace".to_string(),
            "gemname@1.2.3".to_string()
        ),
        split_namespace(
            "gem.coop".to_string(),
            "@namespace/gemname@1.2.3".to_string()
        )
    );
}

#[test]
fn test_parse_namespace_without_from() {
    assert_eq!(
        (
            "https://gem.coop/@indirect".to_string(),
            "card".to_string(),
            vec!["card".to_string(), "--flag".to_string()],
        ),
        parse_namespace(
            "https://gem.coop".to_string(),
            None,
            vec!["@indirect/card".to_string(), "--flag".to_string()],
        )
    );
}

#[test]
fn test_parse_namespace_without_from_strips_namespace() {
    // Without --from, a namespaced command like `@namespace/gemname` turns
    // into `server/@namespace`, gem `gemname`, and command `gemname`.
    assert_eq!(
        (
            "gem.coop/@namespace".to_string(),
            "gemname".to_string(),
            vec!["gemname".to_string(), "--flag".to_string()],
        ),
        parse_namespace(
            "gem.coop".to_string(),
            None,
            vec!["@namespace/gemname".to_string(), "--flag".to_string()],
        )
    );
}

#[test]
fn test_parse_namespace_with_from() {
    // With --from, args[0] is the executable name and must be preserved even when
    // it differs from the gem name (e.g. `pod` provided by `cocoapods`).
    assert_eq!(
        (
            "https://gem.coop".to_string(),
            "cocoapods".to_string(),
            vec!["pod".to_string(), "--version".to_string()],
        ),
        parse_namespace(
            "https://gem.coop".to_string(),
            Some("cocoapods".to_string()),
            vec!["pod".to_string(), "--version".to_string()],
        )
    );
}

#[test]
fn test_parse_namespace_with_from_and_namespace() {
    // The namespace comes from --from, while args[0] keeps the executable name.
    assert_eq!(
        (
            "gem.coop/@namespace".to_string(),
            "cocoapods".to_string(),
            vec!["pod".to_string(), "--version".to_string()],
        ),
        parse_namespace(
            "gem.coop".to_string(),
            Some("@namespace/cocoapods".to_string()),
            vec!["pod".to_string(), "--version".to_string()],
        )
    );
}
