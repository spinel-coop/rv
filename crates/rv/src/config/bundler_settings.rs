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

    /// HTTP URL userinfo (username and optional password) from Bundler `BUNDLE_<HOST>` keys
    /// (same as `bundle config`).
    ///
    /// Returns `(username, password)`. For a bare token, the whole config value is the username
    /// and password is `None`. If the value contains `:`, splits on the first `:` only (password
    /// may contain more colons), matching Bundler.
    pub fn userinfo_for_host(&self, host: &str) -> Option<(String, Option<String>)> {
        let key = format!("BUNDLE_{}", host.to_uppercase().replace('.', "__"));
        let raw = self.get_string(&key)?;
        Some(match raw.split_once(':') {
            None => (raw, None),
            Some((user, password)) => (user.to_string(), Some(password.to_string())),
        })
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
    fn test_userinfo_for_host_from_config_file_only() {
        let temp_dir = Utf8TempDir::new().expect("Failed to create temporary directory");
        let home_dir = temp_dir.path().join("home");
        let project_dir = temp_dir.path().join("project");

        // write a local config with host userinfo (bare token)
        let config_dir = project_dir.join(".bundle");
        std::fs::create_dir_all(&config_dir).unwrap();
        let config_file = config_dir.join("config");

        let config_content = r#"---

BUNDLE_GITHUB__COM: config-token
"#;
        std::fs::write(&config_file, config_content).expect("Failed to write config");

        let settings = BundlerSettings::new(&home_dir, &project_dir).unwrap();
        assert_eq!(
            Some(("config-token".to_string(), None)),
            settings.userinfo_for_host("github.com")
        );

        assert_eq!(None, settings.userinfo_for_host("rubygems.com"));
    }

    #[test]
    fn test_userinfo_for_host_user_password_from_config() {
        let temp_dir = Utf8TempDir::new().expect("Failed to create temporary directory");
        let home_dir = temp_dir.path().join("home");
        let project_dir = temp_dir.path().join("project");

        let config_dir = project_dir.join(".bundle");
        std::fs::create_dir_all(&config_dir).unwrap();
        let config_file = config_dir.join("config");

        let config_content = r#"---

BUNDLE_GITHUB__COM: "user:secret:with:colons"
BUNDLE_GITLAB__COM: "__token__:ghp_abc"
"#;
        std::fs::write(&config_file, config_content).expect("Failed to write config");

        let settings = BundlerSettings::new(&home_dir, &project_dir).unwrap();
        assert_eq!(
            Some(("user".to_string(), Some("secret:with:colons".to_string()))),
            settings.userinfo_for_host("github.com")
        );
        assert_eq!(
            Some(("__token__".to_string(), Some("ghp_abc".to_string()))),
            settings.userinfo_for_host("gitlab.com")
        );
    }
}
