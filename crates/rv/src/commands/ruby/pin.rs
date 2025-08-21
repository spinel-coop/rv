use miette::Diagnostic;
use owo_colors::OwoColorize;

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
    let project_dir = config.project_dir.as_ref().ok_or_else(|| {
        Error::ConfigError(config::Error::NoProjectDir {
            current_dir: config.current_dir.clone(),
        })
    })?;
    let ruby_version_path = project_dir.join(".ruby-version");
    std::fs::write(ruby_version_path, format!("{version}\n"))?;

    println!("{0} pinned to Ruby {1}", project_dir.cyan(), version.cyan());

    Ok(())
}

fn show_pinned_ruby(config: &Config) -> Result<()> {
    let path = config.project_dir.as_ref().unwrap().join(".ruby-version");
    let ruby_version = std::fs::read_to_string(path)?;

    println!(
        "{0} is pinned to Ruby {1}",
        config.project_dir.as_ref().unwrap().cyan(),
        ruby_version.cyan()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;
    use camino::Utf8PathBuf;

    fn test_config() -> Result<Config> {
        let root = Utf8PathBuf::from(TempDir::new().unwrap().path().to_str().unwrap());
        let ruby_dir = root.join("opt/rubies");
        std::fs::create_dir_all(&ruby_dir)?;

        let project_dir = root.join("project");
        std::fs::create_dir_all(&project_dir)?;

        let current_dir = root.join("project");

        let config = Config {
            ruby_dirs: vec![ruby_dir],
            gemfile: None,
            current_exe: root.join("bin").join("rv"),
            project_dir: Some(project_dir),
            current_dir,
            cache: rv_cache::Cache::temp().unwrap(),
            root,
        };

        Ok(config)
    }

    #[test]
    fn test_pin_returns_version() {
        let config = test_config().unwrap();

        let ruby_version_file = config.project_dir.as_ref().unwrap().join(".ruby-version");
        std::fs::write(&ruby_version_file, "3.2.0").unwrap();
        pin(&config, None).unwrap();
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
        let ruby_version_path = config.project_dir.unwrap().join(".ruby-version");
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
        let ruby_version_path = config.project_dir.unwrap().join(".ruby-version");
        let content = std::fs::read_to_string(ruby_version_path).unwrap();
        assert_eq!(content, format!("{second_version}\n"));
    }

    #[test]
    fn test_pin_ruby_with_prerelease_version() {
        let config = test_config().unwrap();
        let version = "3.3.0-preview1".to_string();

        pin(&config, Some(version.clone())).unwrap();

        let ruby_version_path = config.project_dir.unwrap().join(".ruby-version");
        let content = std::fs::read_to_string(ruby_version_path).unwrap();
        assert_eq!(content, format!("{version}\n"));
    }

    #[test]
    fn test_pin_ruby_with_patch_version() {
        let config = test_config().unwrap();
        let version = "1.9.2-p0".to_string();

        pin(&config, Some(version.clone())).unwrap();

        let ruby_version_path = config.project_dir.unwrap().join(".ruby-version");
        let content = std::fs::read_to_string(ruby_version_path).unwrap();
        assert_eq!(content, format!("{version}\n"));
    }
}
