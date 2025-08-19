use std::{
    borrow::Cow,
    env::{join_paths, split_paths},
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
    JoinPathsError(#[from] std::env::JoinPathsError),
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
    paths.retain(|p| !old_ruby_paths.contains(p) && !old_gem_paths.contains(p));

    let request = config.requested_ruby()?;
    let rubies = config.rubies();
    let ruby = rubies.iter().find(|ruby| request.satisfied_by(ruby));

    println!("unset RUBY_ROOT RUBY_ENGINE RUBY_VERSION RUBYOPT GEM_ROOT GEM_HOME GEM_PATH");

    if let Some(ruby) = ruby {
        paths.insert(0, ruby.bin_path().into());
        let path = join_paths(paths)?;

        println!("export PATH={}", escape(&path.to_string_lossy()));
        println!("export RUBY_ROOT={}", escape(&ruby.path));
        println!("export RUBY_ENGINE={}", escape(&ruby.version.engine.name()));
        println!("export RUBY_VERSION={}", escape(&ruby.version.to_string()));
        // println!("export GEM_HOME={}", escape(&ruby.gem_home()));
        // println!("export GEM_ROOT={}", escape(&ruby.gem_root()));
        // println!("export GEM_PATH={}", escape(&join_paths(ruby.gem_paths())));
    }

    println!("hash -r");
    Ok(())
}

fn escape(string: &impl AsRef<str>) -> Cow<'_, str> {
    shell_escape::escape(string.as_ref().into())
}
