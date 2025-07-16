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
    LockfileParser::new(content)
}

/// Load a lockfile from a file path
pub fn load_lockfile<P: AsRef<std::path::Path>>(path: P) -> Result<LockfileParser, Error> {
    let content = std::fs::read_to_string(path)?;
    Ok(parse_lockfile(&content)?)
}

/// Load a lockfile from a specific file path with more explicit naming
pub fn load_lockfile_from_path<P: AsRef<std::path::Path>>(
    path: P,
) -> Result<LockfileParser, Error> {
    load_lockfile(path)
}

/// Load a lockfile from a directory by searching for Gemfile.lock
///
/// This function looks for a file named "Gemfile.lock" in the specified directory.
/// This is the standard name used by Bundler for lockfiles.
pub fn load_lockfile_from_directory<P: AsRef<std::path::Path>>(
    dir: P,
) -> Result<LockfileParser, Error> {
    let lockfile_path = dir.as_ref().join("Gemfile.lock");
    load_lockfile(lockfile_path)
}

/// Find and load a lockfile starting from the current directory and walking up
///
/// This function searches for a Gemfile.lock file starting from the given directory
/// and walking up the directory tree until it finds one or reaches the filesystem root.
/// This is useful when you want to find the lockfile for the current project.
pub fn find_and_load_lockfile<P: AsRef<std::path::Path>>(
    start_dir: P,
) -> Result<LockfileParser, Error> {
    let mut current_dir = start_dir.as_ref().to_path_buf();

    loop {
        let lockfile_path = current_dir.join("Gemfile.lock");

        if lockfile_path.exists() {
            return load_lockfile(lockfile_path);
        }

        // Move up one directory
        if !current_dir.pop() {
            // Reached filesystem root without finding a lockfile
            return Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No Gemfile.lock found in directory tree",
            )));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

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

    #[test]
    fn test_load_lockfile_from_existing_file() {
        // Use an existing test fixture
        let fixture_path = Path::new("tests/fixtures/minimal.gemfile.lock");
        if fixture_path.exists() {
            let parser = load_lockfile(fixture_path).unwrap();
            assert!(!parser.platforms().is_empty());
        }
    }

    #[test]
    fn test_load_lockfile_from_path_alias() {
        // Use an existing test fixture
        let fixture_path = Path::new("tests/fixtures/minimal.gemfile.lock");
        if fixture_path.exists() {
            let parser = load_lockfile_from_path(fixture_path).unwrap();
            assert!(!parser.platforms().is_empty());
        }
    }

    #[test]
    fn test_load_lockfile_from_nonexistent_file() {
        let result = load_lockfile("nonexistent_file.lock");
        assert!(result.is_err());

        if let Err(Error::Io(io_err)) = result {
            assert_eq!(io_err.kind(), std::io::ErrorKind::NotFound);
        } else {
            panic!("Expected IO error");
        }
    }

    #[test]
    fn test_load_lockfile_from_directory() {
        // Create a temporary directory with a Gemfile.lock
        let temp_dir = std::env::temp_dir().join("rv_lockfile_test");
        let _ = fs::create_dir_all(&temp_dir);

        let lockfile_path = temp_dir.join("Gemfile.lock");
        let minimal_lockfile = r#"
PLATFORMS
  ruby

BUNDLED WITH
   2.3.0
"#;

        fs::write(&lockfile_path, minimal_lockfile).unwrap();

        // Test loading from directory
        let parser = load_lockfile_from_directory(&temp_dir).unwrap();
        assert_eq!(parser.platforms().len(), 1);

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_load_lockfile_from_directory_not_found() {
        let temp_dir = std::env::temp_dir().join("rv_lockfile_test_empty");
        let _ = fs::create_dir_all(&temp_dir);

        let result = load_lockfile_from_directory(&temp_dir);
        assert!(result.is_err());

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_find_and_load_lockfile_success() {
        // Create a temporary directory structure with a Gemfile.lock
        let temp_base = std::env::temp_dir().join("rv_lockfile_find_test");
        let temp_subdir = temp_base.join("subdir");
        let _ = fs::create_dir_all(&temp_subdir);

        let lockfile_path = temp_base.join("Gemfile.lock");
        let minimal_lockfile = r#"
PLATFORMS
  ruby

BUNDLED WITH
   2.3.0
"#;

        fs::write(&lockfile_path, minimal_lockfile).unwrap();

        // Test finding from subdirectory
        let parser = find_and_load_lockfile(&temp_subdir).unwrap();
        assert_eq!(parser.platforms().len(), 1);

        // Cleanup
        let _ = fs::remove_dir_all(&temp_base);
    }

    #[test]
    fn test_find_and_load_lockfile_not_found() {
        // Create a temporary directory without any Gemfile.lock
        let temp_dir = std::env::temp_dir().join("rv_lockfile_find_test_empty");
        let _ = fs::create_dir_all(&temp_dir);

        let result = find_and_load_lockfile(&temp_dir);
        assert!(result.is_err());

        if let Err(Error::Io(io_err)) = result {
            assert_eq!(io_err.kind(), std::io::ErrorKind::NotFound);
            assert!(io_err.to_string().contains("No Gemfile.lock found"));
        } else {
            panic!("Expected IO error for not found");
        }

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
