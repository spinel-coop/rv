use camino::Utf8PathBuf;
use config::{
    Config as ConfigRs, Environment, File, FileStoredFormat, Format, Map, Value, ValueKind,
};

use owo_colors::OwoColorize;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error("Multiple config files found: {0:?}")]
    MultipleConfigFiles(Vec<String>),

    #[error("{} is not a valid value for {}", value, setting)]
    SettingsValidationError { value: String, setting: String },
}

type Result<T> = miette::Result<T, Error>;

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, Default)]
pub struct RvSettings {
    pub install_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RvSettingsFormat;

impl Format for RvSettingsFormat {
    fn parse(
        &self,
        _uri: Option<&String>,
        text: &str,
    ) -> std::result::Result<Map<String, Value>, Box<dyn std::error::Error + Send + Sync>> {
        use kdl::KdlDocument;

        let doc: KdlDocument = text.parse::<kdl::KdlDocument>()?;

        let root_node = doc
            .get("rv")
            .ok_or("Missing 'rv' root node in KDL document")?;

        let children = root_node
            .children()
            .ok_or("Missing children in 'rv' node")?;

        const ALLOWED_KEYS: &[&str] = &["install-path"];

        let mut map = Map::new();

        for node in children.nodes() {
            let key = node.name().value();

            if !ALLOWED_KEYS.contains(&key) {
                return Err(format!("Invalid key '{}' in rv config", key).into());
            }

            if node.entries().is_empty() {
                return Err(format!("The key '{}' expects argument(s)", key).into());
            }

            // this logic works just for the first argument. If we need to support Arrays then it
            // will require an update
            let entry = node.entry(0).unwrap();

            let value_str = match entry.value() {
                kdl::KdlValue::String(s) => s.clone(),
                other => other.to_string(),
            };

            map.insert(
                key.to_string().replace("-", "_"),
                Value::new(None, ValueKind::String(value_str)),
            );
        }

        Ok(map)
    }
}

impl FileStoredFormat for RvSettingsFormat {
    fn file_extensions(&self) -> &'static [&'static str] {
        &["kdl"]
    }
}

impl RvSettings {
    fn collect_single_file(paths: &[&str]) -> Result<Option<String>> {
        let mut found_files = Vec::new();

        for &path in paths {
            let file_path = format!("{path}.kdl");
            if std::path::Path::new(file_path.as_str()).is_file() {
                found_files.push(file_path);
            }
        }

        if found_files.len() > 1 {
            return Err(Error::MultipleConfigFiles(found_files));
        }

        Ok(found_files.into_iter().next())
    }

    pub fn new(home_dir: &Utf8PathBuf, project_dir: &Utf8PathBuf) -> Result<Self> {
        // Possible Project Paths
        let local_paths = [
            project_dir.join("rv"),
            project_dir.join(".config/rv"),
            project_dir.join(".config/rv/rv"),
        ];
        let local_paths_strs: Vec<&str> = local_paths.iter().map(|p| p.as_str()).collect();

        // Possible Global Paths
        let global_paths = [
            home_dir.join(".rv"),
            home_dir.join(".config/rv"),
            home_dir.join(".config/rv/rv"),
        ];
        let global_paths_strs: Vec<&str> = global_paths.iter().map(|p| p.as_str()).collect();

        let local_file_opt = Self::collect_single_file(&local_paths_strs)?;
        let global_file_opt = Self::collect_single_file(&global_paths_strs)?;

        let mut builder = ConfigRs::builder();

        if let Some(global_path) = global_file_opt {
            builder = builder.add_source(File::new(&global_path, RvSettingsFormat).required(false));
        }

        if let Some(local_path) = local_file_opt {
            builder = builder.add_source(File::new(&local_path, RvSettingsFormat).required(false));
        }

        builder = builder.add_source(Environment::with_prefix("RV"));

        let s = match builder.build() {
            Ok(config) => config,
            Err(e) => {
                eprintln!("{}: {}", "Error".red(), e);

                std::process::exit(1);
            }
        };

        let settings: RvSettings = match s.try_deserialize() {
            Ok(settings) => settings,
            Err(e) => {
                eprintln!(
                    "{}: Failed to deserialize configuration: {}",
                    "Error".red(),
                    e
                );

                std::process::exit(1);
            }
        };

        Ok(settings)
    }

    pub fn validate(&self) -> Result<()> {
        if let Some(ref install_path_path_str) = self.install_path {
            let path = std::path::Path::new(&install_path_path_str);
            if !path.is_dir() {
                return Err(Error::SettingsValidationError {
                    value: install_path_path_str.clone(),
                    setting: "install_path".to_string(),
                });
            }
        }
        // Add other validations here if needed
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino_tempfile::Utf8TempDir;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_local_config() {
        let temp_dir = Utf8TempDir::new().expect("Failed to create temporary directory");

        let home_dir = temp_dir.path().join("home");
        let project_dir = temp_dir.path().join("project");

        let config_dir = project_dir.join(".config");
        std::fs::create_dir_all(&config_dir).unwrap();
        let config_file = config_dir.join("rv.kdl");

        let config_content = r#"
rv{
  install-path "/home/path"
}
"#;

        std::fs::write(&config_file, config_content).expect("Failed to write config");

        let rv_settings = RvSettings::new(&home_dir, &project_dir);

        assert_eq!(
            String::from("/home/path"),
            rv_settings.unwrap().install_path.unwrap()
        )
    }

    #[test]
    fn test_fallback_to_defaults_when_no_env_vars_and_no_files() {
        let temp_dir = Utf8TempDir::new().expect("Failed to create temporary directory");

        let home_dir = temp_dir.path().join("home");
        let project_dir = temp_dir.path().join("project");

        let rv_settings =
            RvSettings::new(&home_dir, &project_dir).expect("Failed to load settings");

        assert!(rv_settings.install_path.is_none());
    }

    #[test]
    fn test_validate_raises_error_for_invalid_install_path() {
        let invalid_path = "/non/existent/path/rv_test";

        let settings = RvSettings {
            install_path: Some(invalid_path.to_string()),
        };

        let result = settings.validate();

        assert!(result.is_err());

        if let Err(err) = result {
            match err {
                Error::SettingsValidationError { value, setting } => {
                    assert_eq!(value, invalid_path);
                    assert_eq!(setting, "install_path");
                }
                _ => panic!("Expected SettingsValidationError"),
            }
        }
    }

    #[test]
    fn test_collect_single_file_none_found() {
        let paths = vec!["nonexistent/path1", "nonexistent/path2"];
        let result = RvSettings::collect_single_file(&paths).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_collect_single_file_one_found() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("config.kdl");
        let file_str = file_path.to_str().unwrap();
        // create the file without .kdl, so collect_single_file adds it
        let base_path = &file_str[..file_str.len() - 4];
        let mut file = File::create(file_str).unwrap();
        writeln!(file, "rv {{ install-path \"/valid/path\" }}").unwrap();

        let paths = vec![base_path];
        let result = RvSettings::collect_single_file(&paths).unwrap();
        assert_eq!(result.unwrap(), format!("{}.kdl", base_path));
    }

    #[test]
    fn test_collect_single_file_multiple_found() {
        let dir = tempdir().unwrap();

        let file1_path = dir.path().join("config1.kdl");
        let file2_path = dir.path().join("config2.kdl");

        std::fs::write(&file1_path, "rv { install-path \"/path1\" }").unwrap();
        std::fs::write(&file2_path, "rv { install-path \"/path2\" }").unwrap();

        let base1 = file1_path.to_str().unwrap();
        let base2 = file2_path.to_str().unwrap();

        // Remove the trailing ".kdl" to simulate input without extension
        let base1_trimmed = &base1[..base1.len() - 4];
        let base2_trimmed = &base2[..base2.len() - 4];

        let paths = vec![base1_trimmed, base2_trimmed];
        let result = RvSettings::collect_single_file(&paths);
        assert!(result.is_err());

        if let Err(Error::MultipleConfigFiles(files)) = result {
            assert_eq!(files.len(), 2);
            assert!(files.contains(&format!("{}.kdl", base1_trimmed)));
            assert!(files.contains(&format!("{}.kdl", base2_trimmed)));
        } else {
            panic!("Expected MultipleConfigFiles error");
        }
    }
}
