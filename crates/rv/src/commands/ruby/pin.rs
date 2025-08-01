use miette::{IntoDiagnostic, Result};
use owo_colors::OwoColorize;
use std::io::Write;

use crate::config::Config;

pub fn pin(config: &Config, version: Option<String>) -> Result<()> {
    match version {
        None => show_pinned_ruby(config),
        Some(version) => set_pinned_ruby(config, version),
    }
}

fn set_pinned_ruby(config: &Config, version: String) -> Result<()> {
    let project_dir = config.project_dir.as_ref().unwrap();
    let ruby_version_path = project_dir.join(".ruby-version");

    let mut ruby_version_file = std::fs::File::create(&ruby_version_path).into_diagnostic()?;
    writeln!(ruby_version_file, "{version}").into_diagnostic()?;

    println!(
        "{0} pinned to Ruby {1}",
        project_dir.display().to_string().cyan(),
        version.cyan()
    );

    Ok(())
}

fn show_pinned_ruby(config: &Config) -> Result<()> {
    let ruby_version_path = config.project_dir.as_ref().unwrap().join(".ruby-version");

    let ruby_version = std::fs::read_to_string(&ruby_version_path)
        .into_diagnostic()?
        .trim()
        .to_string();

    println!(
        "{0} is pinned to Ruby {1}",
        config
            .project_dir
            .clone()
            .unwrap()
            .display()
            .to_string()
            .cyan(),
        ruby_version.cyan()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;

    fn test_config() -> miette::Result<(Config, TempDir)> {
        let temp_dir = TempDir::new().into_diagnostic()?;
        let root = temp_dir.path().to_path_buf();
        let ruby_dir = root.join("opt").join("rubies");
        std::fs::create_dir_all(&ruby_dir).into_diagnostic()?;

        let project_dir = root.join("project");
        std::fs::create_dir_all(&project_dir).into_diagnostic()?;

        let current_dir = root.join("project");

        let config = Config {
            ruby_dirs: vec![ruby_dir],
            gemfile: None,
            root,
            project_dir: Some(project_dir),
            current_dir,
        };

        Ok((config, temp_dir))
    }

    #[test]
    fn test_pin_returns_version() {
        let (config, _temp_dir) = test_config().unwrap();

        let ruby_version_file = config.project_dir.as_ref().unwrap().join(".ruby-version");

        std::fs::write(&ruby_version_file, "3.2.0").unwrap();
        pin(&config, None).unwrap();
        std::fs::write(&ruby_version_file, "3.2.0\n").unwrap();
        pin(&config, None).unwrap();
    }

    #[test]
    fn test_pin_ruby_creates_file() {
        let (config, _temp_dir) = test_config().unwrap();
        let version = "3.2.0".to_string();

        // Should not panic - basic smoke test
        pin(&config, Some(version.clone())).unwrap();

        // Verify the file was created
        let ruby_version_path = config.project_dir.unwrap().join(".ruby-version");
        assert!(ruby_version_path.exists());
        let content = std::fs::read_to_string(&ruby_version_path).unwrap();
        assert_eq!(content, format!("{version}\n"));
    }

    #[test]
    fn test_pin_ruby_overwrites_existing_file() {
        let (config, _temp_dir) = test_config().unwrap();
        let first_version = "3.0.0".to_string();
        let second_version = "3.2.0".to_string();

        // Pin first version
        pin(&config, Some(first_version)).unwrap();

        // Pin second version (should overwrite)
        pin(&config, Some(second_version.clone())).unwrap();

        // Verify the file contains the second version
        let ruby_version_path = config.project_dir.unwrap().join(".ruby-version");
        let content = std::fs::read_to_string(&ruby_version_path).unwrap();
        assert_eq!(content, format!("{second_version}\n"));
    }

    #[test]
    fn test_pin_ruby_with_prerelease_version() {
        let (config, _temp_dir) = test_config().unwrap();
        let version = "3.3.0-preview1".to_string();

        pin(&config, Some(version.clone())).unwrap();

        let ruby_version_path = config.project_dir.unwrap().join(".ruby-version");
        let content = std::fs::read_to_string(&ruby_version_path).unwrap();
        assert_eq!(content, format!("{version}\n"));
    }

    #[test]
    fn test_pin_ruby_with_patch_version() {
        let (config, _temp_dir) = test_config().unwrap();
        let version = "1.9.2-p0".to_string();

        pin(&config, Some(version.clone())).unwrap();

        let ruby_version_path = config.project_dir.unwrap().join(".ruby-version");
        let content = std::fs::read_to_string(&ruby_version_path).unwrap();
        assert_eq!(content, format!("{version}\n"));
    }
}
