use miette::{IntoDiagnostic, Result};
use owo_colors::OwoColorize;

use crate::config::Config;

pub fn install(config: &Config, version: String) -> Result<()> {
    println!(
        "Installing Ruby version {} in {}",
        version.cyan(),
        config
            .rubies()
            .unwrap()
            .first()
            .map_or("unknown directory".to_string(), |r| r
                .path()
                .as_str()
                .cyan())
    );
}
