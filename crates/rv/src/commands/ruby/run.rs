use owo_colors::OwoColorize;
use rv_ruby::request::RubyRequest;

use crate::GlobalArgs;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    RunError(#[from] crate::commands::run::Error),
}

type Result<T> = miette::Result<T, Error>;

/// Shell out to the given ruby `request`, run it with the given arguments.
/// If given `request` is `None`, shell out to whatever version is pinned in a version
/// file, or to the default ruby version if no ruby version is found in version files.
/// By default, if the ruby isn't installed, install it (disabled via `no_install`).
pub(crate) async fn run(
    global_args: &GlobalArgs,
    request: Option<RubyRequest>,
    no_install: bool,
    args: Vec<String>,
) -> Result<()> {
    let args = [vec!["ruby".to_string()], args].concat();

    let os_cmd: Vec<_> = std::env::args().collect::<Vec<_>>();

    let mut orig_cmd = os_cmd.clone();

    // Make arbritary args to ruby CLI generic in our suggested commands. We won't try give
    // specific CLI for those since I'm not sure how to access original quoting and without that
    // we'll suggest incorrect commands.
    let arg_separator_position = orig_cmd.iter().position(|arg| arg == "--");

    if let Some(arg_separator_position) = arg_separator_position {
        orig_cmd[arg_separator_position + 1] = "<ARGS>".into();
        orig_cmd.truncate(arg_separator_position + 2);
    }

    // Also make path to `rv` generic to simplify suggestions
    orig_cmd[0] = "rv".into();

    let mut new_cmd = orig_cmd.clone();

    // Add "ruby" binary to the end, replacing "--" if given
    if let Some(arg_separator_position) = arg_separator_position {
        new_cmd[arg_separator_position] = "ruby".into();
    } else {
        new_cmd.push("ruby".into());
    }

    // Transform "ruby run" to "run"
    new_cmd.remove(1);

    // If a request was given, put `--ruby` flag to `rv run` before it
    if request.is_some() {
        let request_position = new_cmd[2..]
            .iter()
            .position(|arg| !arg.starts_with("-") && arg != "ruby")
            .expect("we know a request was given");
        new_cmd.insert(request_position + 2, "--ruby".into());
    }

    eprintln!(
        "{}: The `{}` command is deprecated, use `{}` instead",
        "DEPRECATION".red(),
        orig_cmd.join(" ").yellow(),
        new_cmd.join(" ").yellow()
    );

    let run_args = crate::commands::run::RunArgs {
        ruby: request,
        no_install,
        args: args.to_vec(),
    };

    Ok(crate::commands::run::run(global_args, run_args).await?)
}
