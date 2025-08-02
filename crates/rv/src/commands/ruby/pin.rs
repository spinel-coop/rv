use miette::{IntoDiagnostic, Result};
use owo_colors::OwoColorize;
use rsfs::GenFS;
use std::io::Write;

use crate::config::Config;

pub fn pin<F: GenFS>(config: &Config<F>, version: Option<String>) -> Result<()> {
    match version {
        None => show_pinned_ruby(config),
        Some(version) => set_pinned_ruby(config, version),
    }
}

fn set_pinned_ruby<F: GenFS>(config: &Config<F>, version: String) -> Result<()> {
    let project_dir = config.project_dir.as_ref().unwrap();
    let ruby_version_path = project_dir.join(".ruby-version");

    let mut ruby_version_file = config.root.create_file(&ruby_version_path).into_diagnostic()?;
    writeln!(ruby_version_file, "{version}").into_diagnostic()?;

    println!(
        "{0} pinned to Ruby {1}",
        project_dir.display().to_string().cyan(),
        version.cyan()
    );

    Ok(())
}

fn show_pinned_ruby<F: GenFS>(config: &Config<F>) -> Result<()> {
    let project_dir = config.project_dir.as_ref().unwrap();
    let ruby_version_path = project_dir.join(".ruby-version");

    let mut ruby_version_file = config.root.open_file(&ruby_version_path).into_diagnostic()?;
    let mut ruby_version = String::new();
    std::io::Read::read_to_string(&mut ruby_version_file, &mut ruby_version).into_diagnostic()?;

    println!(
        "{0} is pinned to Ruby {1}",
        project_dir.display().to_string().cyan(),
        ruby_version.trim().cyan()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsfs::mem::FS;
    use std::io::Write;

    fn test_config() -> miette::Result<Config<FS>> {
        let fs = FS::new();
        let ruby_dir = std::path::PathBuf::from("/opt/rubies");
        let project_dir = std::path::PathBuf::from("/project");
        let current_dir = std::path::PathBuf::from("/project");

        // Create the necessary directories in the in-memory filesystem
        fs.create_dir_all(&ruby_dir).ok();
        fs.create_dir_all(&project_dir).ok();

        Ok(Config {
            ruby_dirs: vec![ruby_dir],
            gemfile: None,
            root: fs,
            project_dir: Some(project_dir),
            current_dir,
        })
    }

    #[test]
    fn test_pin_returns_version() {
        let config = test_config().unwrap();
        let project_dir = config.project_dir.as_ref().unwrap();
        let ruby_version_path = project_dir.join(".ruby-version");

        // Create the .ruby-version file with version 3.2.0
        let mut ruby_version_file = config.root.create_file(&ruby_version_path).unwrap();
        write!(ruby_version_file, "3.2.0").unwrap();

        // Test that showing the pinned ruby works
        pin(&config, None).unwrap();
    }

    #[test]
    fn test_pin_ruby_creates_file() {
        let config = test_config().unwrap();
        let version = "3.2.0".to_string();

        // Should not panic - basic smoke test
        pin(&config, Some(version.clone())).unwrap();

        // Verify the file was created and has the right content
        let project_dir = config.project_dir.as_ref().unwrap();
        let ruby_version_path = project_dir.join(".ruby-version");
        let mut ruby_version_file = config.root.open_file(&ruby_version_path).unwrap();
        let mut content = String::new();
        std::io::Read::read_to_string(&mut ruby_version_file, &mut content).unwrap();
        assert_eq!(content.trim(), version);
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
        let project_dir = config.project_dir.as_ref().unwrap();
        let ruby_version_path = project_dir.join(".ruby-version");
        let mut ruby_version_file = config.root.open_file(&ruby_version_path).unwrap();
        let mut content = String::new();
        std::io::Read::read_to_string(&mut ruby_version_file, &mut content).unwrap();
        assert_eq!(content.trim(), second_version);
    }

    #[test]
    fn test_pin_ruby_with_prerelease_version() {
        let config = test_config().unwrap();
        let version = "3.3.0-preview1".to_string();

        pin(&config, Some(version.clone())).unwrap();

        let project_dir = config.project_dir.as_ref().unwrap();
        let ruby_version_path = project_dir.join(".ruby-version");
        let mut ruby_version_file = config.root.open_file(&ruby_version_path).unwrap();
        let mut content = String::new();
        std::io::Read::read_to_string(&mut ruby_version_file, &mut content).unwrap();
        assert_eq!(content.trim(), version);
    }

    #[test]
    fn test_pin_ruby_with_patch_version() {
        let config = test_config().unwrap();
        let version = "1.9.2-p0".to_string();

        pin(&config, Some(version.clone())).unwrap();

        let project_dir = config.project_dir.as_ref().unwrap();
        let ruby_version_path = project_dir.join(".ruby-version");
        let mut ruby_version_file = config.root.open_file(&ruby_version_path).unwrap();
        let mut content = String::new();
        std::io::Read::read_to_string(&mut ruby_version_file, &mut content).unwrap();
        assert_eq!(content.trim(), version);
    }
}
