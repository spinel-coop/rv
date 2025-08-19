use std::borrow::Cow;

use crate::config;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    ConfigError(#[from] crate::config::Error),
    #[error("No Ruby installations found in configuration.")]
    NoRubyFound,
}

type Result<T> = miette::Result<T, Error>;

pub fn env(config: &config::Config) -> Result<()> {
    // Remove $RUBY_ROOT/bin from PATH
    // Remove $GEM_ROOT/bin from PATH
    // Remove $GEM_HOME/bin from PATH
    // Remove : from start and end of PATH
    println!("unset RUBY_ROOT RUBY_ENGINE RUBY_VERSION RUBYOPT GEM_ROOT GEM_HOME GEM_PATH");

    let request = config.requested_ruby()?;
    let rubies = config.rubies();
    let ruby = rubies.iter().find(|ruby| request.satisfied_by(ruby));
    if let Some(ruby) = ruby {
        println!("export PATH={}:$PATH", escape(&ruby.bin_path()));
        println!("export RUBY_ROOT={}", escape(&ruby.path));
        println!("export RUBY_ENGINE={}", escape(&ruby.version.engine.name()));
        println!("export RUBY_VERSION={}", escape(&ruby.version.to_string()));
        // export GEM_HOME="$HOME/.gem/$RUBY_ENGINE/$RUBY_VERSION"
        // export PATH="$GEM_HOME/bin:$PATH"
        // export GEM_PATH="$GEM_HOME${GEM_ROOT:+:$GEM_ROOT}${GEM_PATH:+:$GEM_PATH}"
        // export GEM_ROOT={ruby.gem_root}
        // export PATH="${GEM_ROOT:+$GEM_ROOT/bin:}$PATH"
    }
    println!("hash -r");
    Ok(())
}

fn escape(string: &impl AsRef<str>) -> Cow<'_, str> {
    shell_escape::escape(string.as_ref().into())
}
