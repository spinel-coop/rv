use std::io;

use anstream::println;
use owo_colors::OwoColorize;

use crate::config::Config;

pub fn dir(config: &Config) -> io::Result<()> {
    let ruby_dir = match config.ruby_dirs.first() {
        Some(dir) => dir.clone(),
        None => panic!("No Ruby directories to install into"),
    };

    println!("{}", ruby_dir.as_str().cyan());
    Ok(())
}
