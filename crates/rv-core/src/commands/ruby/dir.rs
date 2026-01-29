use anstream::println;
use owo_colors::OwoColorize;

use crate::config::Config;

pub fn dir(config: &Config) {
    let Some(ruby_dir) = config.ruby_dirs.first() else {
        tracing::error!("No Ruby directories found");
        return;
    };

    println!("{}", ruby_dir.as_str().cyan());
}
