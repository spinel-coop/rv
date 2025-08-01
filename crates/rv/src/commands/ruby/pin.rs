use miette::Diagnostic;
use owo_colors::OwoColorize;
use vfs::VfsError;

use crate::config::{self, Config};

#[derive(Debug, thiserror::Error, Diagnostic)]
pub enum Error {
    #[error(transparent)]
    ConfigError(#[from] config::Error),
    #[error(transparent)]
    VfsError(#[from] VfsError),
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
    let project_dir = config.get_project_dir()?;
    let ruby_version_path = project_dir.join(".ruby-version")?;

    let mut ruby_version_file = ruby_version_path.create_file()?;
    writeln!(ruby_version_file, "{version}")?;

    println!(
        "{0} pinned to Ruby {1}",
        project_dir.as_str().cyan(),
        version.cyan()
    );

    Ok(())
}

fn show_pinned_ruby(config: &Config) -> Result<()> {
    let ruby_version = config
        .project_dir
        .as_ref()
        .unwrap()
        .join(".ruby-version")?
        .read_to_string()?
        .trim()
        .to_string();

    println!(
        "{0} is pinned to Ruby {1}",
        config.project_dir.clone().unwrap().as_str().cyan(),
        ruby_version.cyan()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use vfs::VfsPath;

    fn test_config() -> Result<Config> {
        let memory_fs = vfs::MemoryFS::new();
        let root = VfsPath::new(memory_fs);
        let ruby_dir = root.join("/opt/rubies")?;
        ruby_dir.create_dir_all()?;

        let project_dir = root.join("/project")?;
        project_dir.create_dir_all()?;

        let current_dir = root.join("/project")?;

        Ok(Config {
            ruby_dirs: vec![ruby_dir],
            gemfile: None,
            root,
            project_dir: Some(project_dir),
            current_dir,
        })
    }

    #[test]
    fn test_pin_returns_version() {
        let config = test_config().unwrap();

        let mut ruby_version_file = config
            .project_dir
            .as_ref()
            .unwrap()
            .join(".ruby-version")
            .unwrap()
            .create_file()
            .unwrap();
        write!(ruby_version_file, "3.2.0").unwrap();
        pin(&config, None).unwrap();
        writeln!(ruby_version_file, "3.2.0").unwrap();
        pin(&config, None).unwrap();
    }

    #[test]
    fn test_pin_ruby_creates_file() {
        let config = test_config().unwrap();
        let version = "3.2.0".to_string();

        // Should not panic - basic smoke test
        pin(&config, Some(version.clone())).unwrap();

        // Verify the file was created
        let ruby_version_path = config.project_dir.unwrap().join(".ruby-version").unwrap();
        assert!(ruby_version_path.exists().unwrap());
        let content = ruby_version_path.read_to_string().unwrap();
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
        let ruby_version_path = config.project_dir.unwrap().join(".ruby-version").unwrap();
        let content = ruby_version_path.read_to_string().unwrap();
        assert_eq!(content, format!("{second_version}\n"));
    }

    #[test]
    fn test_pin_ruby_with_prerelease_version() {
        let config = test_config().unwrap();
        let version = "3.3.0-preview1".to_string();

        pin(&config, Some(version.clone())).unwrap();

        let ruby_version_path = config.project_dir.unwrap().join(".ruby-version").unwrap();
        let content = ruby_version_path.read_to_string().unwrap();
        assert_eq!(content, format!("{version}\n"));
    }

    #[test]
    fn test_pin_ruby_with_patch_version() {
        let config = test_config().unwrap();
        let version = "1.9.2-p0".to_string();

        pin(&config, Some(version.clone())).unwrap();

        let ruby_version_path = config.project_dir.unwrap().join(".ruby-version").unwrap();
        let content = ruby_version_path.read_to_string().unwrap();
        assert_eq!(content, format!("{version}\n"));
    }
}
