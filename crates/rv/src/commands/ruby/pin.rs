use miette::{IntoDiagnostic, Result};
use std::fs;

pub fn pin(_version: Option<String>) -> Result<()> {
    let ruby_version: String = fs::read_to_string(".ruby-version").into_diagnostic()?;
    println!("{ruby_version}");
    Ok(())
}
