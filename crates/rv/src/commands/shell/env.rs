use std::{
    borrow::Cow,
    env::{JoinPathsError, join_paths, split_paths},
};

use crate::config;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    ConfigError(#[from] crate::config::Error),
    #[error("No Ruby installations found in configuration.")]
    NoRubyFound,
    #[error(transparent)]
    EnvError(#[from] std::env::VarError),
    #[error(transparent)]
    JoinPathsError(#[from] JoinPathsError),
}

type Result<T> = miette::Result<T, Error>;

pub fn env(config: &config::Config) -> Result<()> {
    let mut paths = std::env::var("PATH").map(|p| split_paths(&p).collect::<Vec<_>>())?;

    let old_ruby_paths = [
        std::env::var("RUBY_ROOT")?.as_str(),
        std::env::var("GEM_ROOT")?.as_str(),
        std::env::var("GEM_HOME")?.as_str(),
    ]
    .map(|p| std::path::Path::new(p).join("bin"));
    let old_gem_paths = std::env::var("GEM_PATH").map(|p| split_paths(&p).collect::<Vec<_>>())?;

    // Remove old Ruby and Gem paths from the PATH
    paths.retain(|p| !old_ruby_paths.contains(p) && !old_gem_paths.contains(p));

    let request = config.requested_ruby()?;
    let rubies = config.rubies();
    let ruby = rubies.iter().find(|ruby| request.satisfied_by(ruby));
    let mut gem_paths = vec![];

    println!("unset RUBY_ROOT RUBY_ENGINE RUBY_VERSION RUBYOPT GEM_ROOT GEM_HOME GEM_PATH");

    if let Some(ruby) = ruby {
        paths.insert(0, ruby.bin_path().into());
        println!("export RUBY_ROOT={}", escape(&ruby.path));
        println!("export RUBY_ENGINE={}", escape(&ruby.version.engine.name()));
        println!("export RUBY_VERSION={}", escape(&ruby.version.to_string()));
        if let Some(gem_home) = ruby.gem_home() {
            paths.insert(0, gem_home.join("bin").into());
            gem_paths.insert(0, gem_home.join("bin"));
            println!("export GEM_HOME={}", escape(&gem_home));
        }
        if let Some(gem_root) = ruby.gem_root() {
            paths.insert(0, gem_root.join("bin").into());
            gem_paths.insert(0, gem_root.join("bin"));
            println!("export GEM_ROOT={}", escape(&gem_root));
        }
        let gem_path = join_paths(gem_paths)?;
        if let Some(gem_path) = gem_path.to_str() {
            println!("export GEM_PATH={}", escape(&gem_path));
        }
        let path = join_paths(paths)?;
        if let Some(path) = path.to_str() {
            println!("export PATH={}", escape(&path));
        }
    }

    println!("hash -r");
    Ok(())
}

fn escape(string: &impl AsRef<str>) -> Cow<'_, str> {
    shell_escape::escape(string.as_ref().into())
}
