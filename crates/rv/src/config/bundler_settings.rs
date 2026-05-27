use camino::Utf8PathBuf;
use config::{Config as ConfigRs, Environment, File, FileFormat};
use miette::Diagnostic;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::path::absolute;

#[derive(Debug, thiserror::Error, Diagnostic)]
pub enum Error {
    #[error("Error parsing Bundler configuration: {0}")]
    #[diagnostic(code(rv::bundler_build_error))]
    BuildError(String),

    #[error("Failed to deserialize configuration: {0}")]
    #[diagnostic(code(rv::bundler_deserialization_error))]
    DeserializationError(String),
}

type Result<T> = miette::Result<T, Error>;

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, Default)]
pub struct BundlerSettings {
    #[serde(flatten)]
    settings: HashMap<String, JsonValue>,
}

impl BundlerSettings {
    pub fn new(home_dir: &Utf8PathBuf, project_dir: &Utf8PathBuf) -> Result<Self> {
        let mut builder = ConfigRs::builder();

        builder = builder.add_source(
            File::new(home_dir.join(".bundle/config").as_str(), FileFormat::Yaml).required(false),
        );

        builder = builder.add_source(
            Environment::with_prefix("BUNDLE")
                .keep_prefix(true)
                .convert_case(config::Case::UpperSnake),
        );

        builder = builder.add_source(
            File::new(
                project_dir.join(".bundle/config").as_str(),
                FileFormat::Yaml,
            )
            .required(false),
        );

        let config = builder
            .build()
            .map_err(|e| Error::BuildError(e.to_string()))?;

        let parsed: BundlerSettings = config
            .try_deserialize()
            .map_err(|e| Error::DeserializationError(e.to_string()))?;

        Ok(Self {
            settings: parsed.settings,
        })
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.settings.get(key).and_then(|v| {
            if let Some(b) = v.as_bool() {
                return Some(b);
            }
            if let Some(s) = v.as_str() {
                return s.parse::<bool>().ok();
            }
            None
        })
    }

    pub fn get_string(&self, key: &str) -> Option<String> {
        self.settings.get(key).map(|v| match v {
            JsonValue::String(s) => s.clone(),
            other => other.to_string(),
        })
    }

    pub fn path(&self) -> Option<Utf8PathBuf> {
        let path_opt = self.get_string("BUNDLE_PATH");
        let path_system_opt = self.get_bool("BUNDLE_PATH__SYSTEM");
        let deployment_opt = self.get_bool("BUNDLE_DEPLOYMENT");

        if let Some(ref p) = path_opt {
            let result = absolute(p)
                .ok()
                .and_then(|pb| Utf8PathBuf::from_path_buf(pb).ok());

            return result;
        }

        if path_system_opt.unwrap_or(false) {
            return None;
        }

        if deployment_opt.unwrap_or(false) {
            return absolute("vendor/bundle")
                .ok()
                .and_then(|pb| Utf8PathBuf::from_path_buf(pb).ok());
        }

        None
    }

    pub fn token_for(&self, host: &str) -> Option<String> {
        let key = format!("BUNDLE_{}", host.to_uppercase().replace('.', "__"));

        self.get_string(&key)
    }

    /// Whether Bundler's `cache_all` setting is enabled. When true, `rv ci`
    /// also writes downloaded gems to `vendor/cache/` as a side effect.
    pub fn cache_all(&self) -> bool {
        self.get_bool("BUNDLE_CACHE_ALL").unwrap_or(false)
    }

    /// Directory where vendored `.gem` files live. Defaults to `vendor/cache`
    /// in the current working directory; can be overridden by
    /// `BUNDLE_CACHE_PATH` in `.bundle/config`. Relative paths are resolved
    /// against the cwd to match Bundler's behavior.
    pub fn cache_path(&self) -> Option<Utf8PathBuf> {
        let raw = self
            .get_string("BUNDLE_CACHE_PATH")
            .unwrap_or_else(|| "vendor/cache".to_string());
        absolute(&raw)
            .ok()
            .and_then(|pb| Utf8PathBuf::from_path_buf(pb).ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino_tempfile::Utf8TempDir;

    #[test]
    fn test_local_config() {
        let temp_dir = Utf8TempDir::new().expect("Failed to create temporary directory");

        let home_dir = temp_dir.path().join("home");
        let project_dir = temp_dir.path().join("project");

        let config_dir = project_dir.join(".bundle");
        std::fs::create_dir_all(&config_dir).unwrap();
        let config_file = config_dir.join("config");

        let cwd = std::env::current_dir().unwrap();

        let config_content = r#"---

BUNDLE_PATH: foo
"#;

        std::fs::write(&config_file, config_content).expect("Failed to write config");

        let bundler_settings = BundlerSettings::new(&home_dir, &project_dir).unwrap();

        assert_eq!(
            cwd.join("foo"),
            bundler_settings.path().unwrap().to_string()
        )
    }

    #[test]
    fn test_global_config() {
        let temp_dir = Utf8TempDir::new().expect("Failed to create temporary directory");

        let home_dir = temp_dir.path().join("home");
        let project_dir = temp_dir.path().join("project");

        let config_dir = home_dir.join(".bundle");
        std::fs::create_dir_all(&config_dir).unwrap();
        let config_file = config_dir.join("config");

        let cwd = std::env::current_dir().unwrap();

        let config_content = r#"---

BUNDLE_PATH: foo
"#;

        std::fs::write(&config_file, config_content).expect("Failed to write config");

        let bundler_settings = BundlerSettings::new(&home_dir, &project_dir).unwrap();

        assert_eq!(
            cwd.join("foo"),
            bundler_settings.path().unwrap().to_string()
        )
    }

    #[test]
    fn test_local_and_global_config() {
        let temp_dir = Utf8TempDir::new().expect("Failed to create temporary directory");

        let home_dir = temp_dir.path().join("home");
        let project_dir = temp_dir.path().join("project");

        let global_config_dir = home_dir.join(".bundle");
        std::fs::create_dir_all(&global_config_dir).unwrap();
        let global_config_file = global_config_dir.join("config");

        let local_config_dir = project_dir.join(".bundle");
        std::fs::create_dir_all(&local_config_dir).unwrap();
        let local_config_file = local_config_dir.join("config");

        let global_config_content = r#"---

BUNDLE_PATH: foo
"#;

        std::fs::write(&global_config_file, global_config_content).expect("Failed to write config");

        let cwd = std::env::current_dir().unwrap();

        let local_config_content = r#"---

BUNDLE_PATH: bar
"#;

        std::fs::write(&local_config_file, local_config_content).expect("Failed to write config");

        let bundler_settings = BundlerSettings::new(&home_dir, &project_dir).unwrap();

        assert_eq!(
            cwd.join("bar"),
            bundler_settings.path().unwrap().to_string()
        )
    }

    #[test]
    fn test_local_with_path_system() {
        let temp_dir = Utf8TempDir::new().expect("Failed to create temporary directory");

        let home_dir = temp_dir.path().join("home");
        let project_dir = temp_dir.path().join("project");

        let local_config_dir = project_dir.join(".bundle");
        std::fs::create_dir_all(&local_config_dir).unwrap();
        let local_config_file = local_config_dir.join("config");

        let local_config_content = r#"---

BUNDLE_PATH__SYSTEM: true
"#;

        std::fs::write(&local_config_file, local_config_content).expect("Failed to write config");

        let bundler_settings = BundlerSettings::new(&home_dir, &project_dir).unwrap();

        assert_eq!(None, bundler_settings.path())
    }

    #[test]
    fn test_local_with_deployment() {
        let temp_dir = Utf8TempDir::new().expect("Failed to create temporary directory");

        let home_dir = temp_dir.path().join("home");
        let project_dir = temp_dir.path().join("project");

        let local_config_dir = project_dir.join(".bundle");
        std::fs::create_dir_all(&local_config_dir).unwrap();
        let local_config_file = local_config_dir.join("config");

        let cwd = std::env::current_dir().unwrap();

        let local_config_content = r#"---

BUNDLE_DEPLOYMENT: true
"#;

        std::fs::write(&local_config_file, local_config_content).expect("Failed to write config");

        let bundler_settings = BundlerSettings::new(&home_dir, &project_dir).unwrap();

        assert_eq!(
            cwd.join("vendor/bundle"),
            bundler_settings.path().unwrap().to_string()
        )
    }

    #[test]
    fn test_empty_config() {
        let temp_dir = Utf8TempDir::new().expect("Failed to create temporary directory");

        let home_dir = temp_dir.path().join("home");
        let project_dir = temp_dir.path().join("project");

        let local_config_dir = project_dir.join(".bundle");
        std::fs::create_dir_all(&local_config_dir).unwrap();
        let local_config_file = local_config_dir.join("config");

        let local_config_content = "";

        std::fs::write(&local_config_file, local_config_content).expect("Failed to write config");

        let bundler_settings = BundlerSettings::new(&home_dir, &project_dir).unwrap();

        assert_eq!(None, bundler_settings.path())
    }

    #[test]
    fn test_cache_all_defaults_to_false() {
        let temp_dir = Utf8TempDir::new().unwrap();
        let home_dir = temp_dir.path().join("home");
        let project_dir = temp_dir.path().join("project");

        let settings = BundlerSettings::new(&home_dir, &project_dir).unwrap();
        assert!(!settings.cache_all());
    }

    #[test]
    fn test_cache_all_from_config() {
        let temp_dir = Utf8TempDir::new().unwrap();
        let home_dir = temp_dir.path().join("home");
        let project_dir = temp_dir.path().join("project");

        let config_dir = project_dir.join(".bundle");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join("config"), "---\nBUNDLE_CACHE_ALL: true\n").unwrap();

        let settings = BundlerSettings::new(&home_dir, &project_dir).unwrap();
        assert!(settings.cache_all());
    }

    #[test]
    fn test_cache_path_defaults_to_vendor_cache() {
        let temp_dir = Utf8TempDir::new().unwrap();
        let home_dir = temp_dir.path().join("home");
        let project_dir = temp_dir.path().join("project");

        let settings = BundlerSettings::new(&home_dir, &project_dir).unwrap();
        let cwd = std::env::current_dir().unwrap();
        assert_eq!(
            cwd.join("vendor/cache"),
            settings.cache_path().unwrap().to_string()
        );
    }

    #[test]
    fn test_cache_path_override_from_config() {
        let temp_dir = Utf8TempDir::new().unwrap();
        let home_dir = temp_dir.path().join("home");
        let project_dir = temp_dir.path().join("project");

        let config_dir = project_dir.join(".bundle");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("config"),
            "---\nBUNDLE_CACHE_PATH: my/cache\n",
        )
        .unwrap();

        let settings = BundlerSettings::new(&home_dir, &project_dir).unwrap();
        let cwd = std::env::current_dir().unwrap();
        assert_eq!(
            cwd.join("my/cache"),
            settings.cache_path().unwrap().to_string()
        );
    }

    #[test]
    fn test_token_for_from_config_file_only() {
        let temp_dir = Utf8TempDir::new().expect("Failed to create temporary directory");
        let home_dir = temp_dir.path().join("home");
        let project_dir = temp_dir.path().join("project");

        // write a local config with the token
        let config_dir = project_dir.join(".bundle");
        std::fs::create_dir_all(&config_dir).unwrap();
        let config_file = config_dir.join("config");

        let config_content = r#"---

BUNDLE_GITHUB__COM: config-token
"#;
        std::fs::write(&config_file, config_content).expect("Failed to write config");

        let settings = BundlerSettings::new(&home_dir, &project_dir).unwrap();
        assert_eq!(
            Some("config-token".to_string()),
            settings.token_for("github.com")
        );

        assert_eq!(None, settings.token_for("rubygems.com"));
    }
}
