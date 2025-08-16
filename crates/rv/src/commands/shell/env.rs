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
    // 1. Remove $RUBY_ROOT/bin from PATH
    // 2. Remove $GEM_ROOT/bin from PATH
    // 3. Remove $GEM_HOME/bin from PATH
    // 4. Remove : from start and end of PATH
    // 5. Unset all Ruby-related environment variables
    // 6. Rehash the shell
    println!("unset RUBY_ROOT RUBY_ENGINE RUBY_VERSION RUBYOPT GEM_ROOT GEM_HOME GEM_PATH");
    println!("hash -r");

    // 7. Search for a Ruby installation that matches .ruby-version
    // 8. If found, set the environment variables accordingly

    //  export PATH="${GEM_ROOT:+$GEM_ROOT/bin:}$PATH"
    // 	export GEM_HOME="$HOME/.gem/$RUBY_ENGINE/$RUBY_VERSION"
    // 	export GEM_PATH="$GEM_HOME${GEM_ROOT:+:$GEM_ROOT}${GEM_PATH:+:$GEM_PATH}"
    // 	export PATH="$GEM_HOME/bin:$PATH"
    //  export GEM_ROOT={ruby.gem_root}

    let request = config.requested_ruby()?;
    let rubies = config.rubies();
    let ruby = rubies.iter().find(|ruby| request.satisfied_by(ruby));
    if let Some(ruby) = ruby {
        let path = std::env::var("PATH").unwrap_or_default();
        println!("export PATH={}:{}", ruby.bin_path(), path);
        println!("export RUBY_ROOT={}", ruby.path);
        println!("export RUBY_ENGINE={}", ruby.version.engine);
        println!("export RUBY_VERSION={}", ruby.version);
        Ok(())
    } else {
        Err(Error::NoRubyFound)
    }
}
