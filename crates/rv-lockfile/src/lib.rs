//! Bundler lockfile parser for rv
//!
//! This crate provides parsing and handling of Bundler lockfiles (Gemfile.lock),
//! mirroring the functionality found in the bundler gem's lockfile parser.

pub mod error;
pub mod parser;
pub mod types;

pub use error::{Error, ParseError};
pub use parser::LockfileParser;
pub use types::{Dependency, LazySpecification, Platform, Source};

/// Parse a lockfile from string content
pub fn parse_lockfile(content: &str) -> Result<LockfileParser, ParseError> {
    LockfileParser::new(content, false)
}

/// Parse a lockfile from string content with strict validation
pub fn parse_lockfile_strict(content: &str) -> Result<LockfileParser, ParseError> {
    LockfileParser::new(content, true)
}

/// Load a lockfile from a file path
pub fn load_lockfile<P: AsRef<std::path::Path>>(path: P) -> Result<LockfileParser, Error> {
    let content = std::fs::read_to_string(path)?;
    Ok(parse_lockfile(&content)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_parsing() {
        let lockfile_content = r#"
GEM
  remote: https://rubygems.org/
  specs:
    test-gem (1.0.0)

PLATFORMS
  ruby

DEPENDENCIES
  test-gem

BUNDLED WITH
   2.3.0
"#;

        let parser = parse_lockfile(lockfile_content).unwrap();
        assert_eq!(parser.platforms().len(), 1);
        assert_eq!(parser.dependencies().len(), 1);
    }
}