use miette::{IntoDiagnostic, Result};
use std::fs;

pub fn pin(config: &Config, _version: Option<String>) -> Result<()> {
    let ruby_version: String = fs::read_to_string(".ruby-version").into_diagnostic()?;
    println!("{ruby_version}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use vfs::VfsPath;

    fn test_config() -> miette::Result<Config> {
        let memory_fs = vfs::MemoryFS::new();
        let root = VfsPath::new(memory_fs);
        let ruby_dir = root.join("/opt/rubies").into_diagnostic()?;
        ruby_dir.create_dir_all().into_diagnostic()?;

        let project_dir = root.join("/project").into_diagnostic()?;
        project_dir.create_dir_all().into_diagnostic()?;

        Ok(Config {
            ruby_dirs: vec![ruby_dir],
            gemfile: None,
            root: root,
        })
    }

    #[test]
    fn test_pin_returns_version() {
        let config = test_config().unwrap();

        pin(&config, None).unwrap();
    }

    #[test]
    fn test_pin_ruby_creates_file() {
        let config = test_config().unwrap();
        let version = "3.2.0".to_string();

        // Should not panic - basic smoke test
        pin(&config, Some(version)).unwrap();

        // Verify the file was created
        let ruby_version_path = config.root.join(".ruby-version").unwrap();
        assert!(ruby_version_path.exists().unwrap());
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
        let ruby_version_path = config.root.join(".ruby-version").unwrap();
        let content = ruby_version_path.read_to_string().unwrap();
        assert_eq!(content, second_version);
    }

    #[test]
    fn test_pin_ruby_with_prerelease_version() {
        let config = test_config().unwrap();
        let version = "3.3.0-preview1".to_string();

        pin(&config, Some(version.clone())).unwrap();

        let ruby_version_path = config.root.join(".ruby-version").unwrap();
        let content = ruby_version_path.read_to_string().unwrap();
        assert_eq!(content, version);
    }

    #[test]
    fn test_pin_ruby_with_patch_version() {
        let config = test_config().unwrap();
        let version = "1.9.2-p0".to_string();

        pin(&config, Some(version.clone())).unwrap();

        let ruby_version_path = config.root.join(".ruby-version").unwrap();
        let content = ruby_version_path.read_to_string().unwrap();
        assert_eq!(content, version);
    }
}
