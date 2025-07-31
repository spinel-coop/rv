use miette::Result;
use owo_colors::OwoColorize;

use crate::config::Config;

pub fn install(config: &Config, version: String) -> Result<()> {
    let rubies = config.rubies()?;
    let ruby = rubies.first().unwrap();

    println!(
        "Installing Ruby version {} in {}",
        version.cyan(),
        ruby.path.as_str().cyan()
    );
    Ok(())
}
