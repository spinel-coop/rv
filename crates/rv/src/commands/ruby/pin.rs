use std::borrow::Cow;

use anstream::println;
use camino::Utf8PathBuf;
use miette::Diagnostic;
use owo_colors::OwoColorize;
use rv_ruby::request::Source;

use crate::config::{self, Config};

#[derive(Debug, thiserror::Error, Diagnostic)]
pub enum Error {
    #[error(transparent)]
    ConfigError(#[from] config::Error),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
}

type Result<T> = miette::Result<T, Error>;

pub fn pin(config: &Config, version: Option<String>) -> Result<()> {
    match version {
        None => show_pinned_ruby(config),
        Some(version) => set_pinned_ruby(config, version),
    }
}

fn set_pinned_ruby(config: &Config, version: String) -> Result<()> {
    let project_dir: Cow<Utf8PathBuf> = match config.requested_ruby {
        Some((_, Source::DotToolVersions(ref path))) => Cow::Borrowed(path),
        Some((_, Source::DotRubyVersion(ref ruby_version_path))) => {
            std::fs::write(ruby_version_path, format!("{version}\n"))?;
            Cow::Borrowed(ruby_version_path)
        }
        Some((_, Source::Other)) | None => {
            let ruby_version_path = config.current_dir.join(".ruby-version");
            std::fs::write(&ruby_version_path, format!("{version}\n"))?;
            Cow::Owned(ruby_version_path)
        }
    };

    println!("{0} pinned to Ruby {1}", project_dir.cyan(), version.cyan());

    Ok(())
}

fn show_pinned_ruby(config: &Config) -> Result<()> {
    let Some((ruby, source)) = &config.requested_ruby else {
        return Err(Error::ConfigError(config::Error::NoProjectDir {
            current_dir: config.current_dir.clone(),
        }));
    };

    let project_dir: Cow<Utf8PathBuf> = match source {
        Source::DotToolVersions(path) => Cow::Borrowed(path),
        Source::DotRubyVersion(ruby_version_path) => Cow::Borrowed(ruby_version_path),
        Source::Other => {
            let ruby_version_path = config.current_dir.join(".ruby-version");
            Cow::Owned(ruby_version_path)
        }
    };

    println!(
        "{0} is pinned to Ruby {1}",
        project_dir.as_ref().cyan(),
        ruby.cyan()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;
    use camino::Utf8PathBuf;
    use indexmap::indexset;

    fn test_config() -> Result<Config> {
        let root = Utf8PathBuf::from(TempDir::new().unwrap().path().to_str().unwrap());
        let ruby_dir = root.join("opt/rubies");
        std::fs::create_dir_all(&ruby_dir)?;
        let current_dir = root.join("project");
        std::fs::create_dir_all(&current_dir)?;
        let requested_ruby = Some(("3.5.0".into(), Source::Other));

        let config = Config {
            ruby_dirs: indexset![ruby_dir],
            gemfile: None,
            current_exe: root.join("bin").join("rv"),
            requested_ruby,
            current_dir,
            cache: rv_cache::Cache::temp().unwrap(),
            root,
        };

        Ok(config)
    }

    #[test]
    fn test_pin_runs_with_no_version() {
        let config = test_config().unwrap();
        pin(&config, None).unwrap();
    }

    #[test]
    fn test_pin_runs_with_ruby_version() {
        let config = test_config().unwrap();

        let ruby_version_file = config.current_dir.join(".ruby-version");
        std::fs::write(&ruby_version_file, "3.2.0").unwrap();
        pin(&config, None).unwrap();
    }

    #[test]
    fn test_pin_runs_with_tool_versions() {
        let config = test_config().unwrap();

        pin(&config, None).unwrap();
        let ruby_version_file = config.current_dir.join(".tool-versions");
        std::fs::write(&ruby_version_file, "3.2.0").unwrap();
        pin(&config, None).unwrap();
    }

    #[test]
    fn test_pin_ruby_creates_file() {
        let config = test_config().unwrap();
        let version = "3.2.0".to_string();

        // Should not panic - basic smoke test
        pin(&config, Some(version.clone())).unwrap();

        // Verify the file was created
        let ruby_version_path = config.current_dir.join(".ruby-version");
        assert!(ruby_version_path.exists());
        let content = std::fs::read_to_string(ruby_version_path).unwrap();
        assert_eq!(content, format!("{version}\n"));
    }

    #[test]
    fn test_pin_ruby_overwrites_existing_file() {
        let config = test_config().unwrap();
        let first_version = "3.0.0".to_string();
        let second_version = "3.2.0".to_string();

        // Pin first version
        pin(&config, Some(first_version)).unwrap();

        // Pin second version (should overwrite)
        pin(&config, Some(second_version.clone())).unwrap();

        // Verify the file contains the second version
        let ruby_version_path = config.current_dir.join(".ruby-version");
        let content = std::fs::read_to_string(ruby_version_path).unwrap();
        assert_eq!(content, format!("{second_version}\n"));
    }

    #[test]
    fn test_pin_ruby_with_prerelease_version() {
        let config = test_config().unwrap();
        let version = "3.3.0-preview1".to_string();

        pin(&config, Some(version.clone())).unwrap();

        let ruby_version_path = config.current_dir.join(".ruby-version");
        let content = std::fs::read_to_string(ruby_version_path).unwrap();
        assert_eq!(content, format!("{version}\n"));
    }

    #[test]
    fn test_pin_ruby_with_patch_version() {
        let config = test_config().unwrap();
        let version = "1.9.2-p0".to_string();

        pin(&config, Some(version.clone())).unwrap();

        let ruby_version_path = config.current_dir.join(".ruby-version");
        let content = std::fs::read_to_string(ruby_version_path).unwrap();
        assert_eq!(content, format!("{version}\n"));
    }
}
