use anstream::println;
use owo_colors::OwoColorize;

use crate::GlobalArgs;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {}

type Result<T> = miette::Result<T, Error>;

pub(crate) fn dir(_global_args: &GlobalArgs) -> Result<()> {
    let tool_dir = crate::commands::tool::tool_dir();

    println!("{}", tool_dir.as_str().cyan());

    Ok(())
}
