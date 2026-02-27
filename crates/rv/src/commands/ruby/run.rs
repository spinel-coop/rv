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

    let run_args = crate::commands::run::RunArgs {
        ruby: request,
        no_install,
        args: args.to_vec(),
    };

    Ok(crate::commands::run::run(global_args, run_args).await?)
}
