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
    let ruby = config.rubies().first().cloned();
    if let Some(ruby) = ruby {
        print!(
            concat!(
                "export PATH={}:$PATH\n",
                "export RUBY_ROOT={}\n",
                "export RUBY_ENGINE={}\n",
                "export RUBY_VERSION={}\n",
            ),
            ruby.bin_path(),
            ruby.path,
            ruby.version.engine,
            ruby.version,
        );
        Ok(())
    } else {
        Err(Error::NoRubyFound)
    }
}
