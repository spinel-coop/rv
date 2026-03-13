use camino::Utf8PathBuf;
use saphyr::{LoadableYamlNode, Yaml};
use std::{path::absolute, str::FromStr};

#[derive(Debug, Clone, Default)]
pub struct BundlerSettings<'a> {
    // Project specific Bundler configuration, if any
    pub local: Option<Yaml<'a>>,

    // Global Bundler configuration, if any
    pub global: Option<Yaml<'a>>,
}

impl BundlerSettings<'_> {
    pub fn new(home_dir: &Utf8PathBuf, project_dir: &Utf8PathBuf) -> Self {
        Self {
            local: Self::config_for_dir(project_dir),
            global: Self::config_for_dir(home_dir),
        }
    }

    pub fn token_for(&self, host: &str) -> Option<String> {
        let key = format!("BUNDLE_{}", host.to_uppercase().replace(".", "__"));

        self.local
            .as_ref()
            .and_then(|settings| Self::get_string_file_config(settings, &key))
            .or_else(|| Self::get_string_env_config(&key))
            .or_else(|| {
                self.global
                    .as_ref()
                    .and_then(|settings| Self::get_string_file_config(settings, &key))
            })
    }

    pub fn path(&self) -> Option<Utf8PathBuf> {
        let local = self.local_path_config();
        let env = Self::env_path_config();
        let global = self.global_path_config();
        let mut use_deployment = None;

        for (path, path_system, deployment) in [local, env, global] {
            if use_deployment.is_none() {
                use_deployment = deployment;
            };

            if path.is_none() && path_system.is_none() {
                continue;
            };

            let is_none = path.is_none();
            let install_path = InstallPath {
                explicit_path: path,
                use_system_gems: path_system.unwrap_or(is_none),
            };

            return install_path.path();
        }

        let use_deployment = use_deployment?;

        let install_path = InstallPath {
            explicit_path: use_deployment.then_some("vendor/bundle".to_string()),
            use_system_gems: !use_deployment,
        };

        install_path.path()
    }

    fn local_path_config(&self) -> (Option<String>, Option<bool>, Option<bool>) {
        Self::path_config(&self.local)
    }

    fn env_path_config() -> (Option<String>, Option<bool>, Option<bool>) {
        (
            Self::get_string_env_config("BUNDLE_PATH"),
            Self::get_bool_env_config("BUNDLE_PATH__SYSTEM"),
            Self::get_bool_env_config("BUNDLE_DEPLOYMENT"),
        )
    }

    fn global_path_config(&self) -> (Option<String>, Option<bool>, Option<bool>) {
        Self::path_config(&self.global)
    }

    fn path_config(settings: &Option<Yaml>) -> (Option<String>, Option<bool>, Option<bool>) {
        let Some(settings) = settings else {
            return (None, None, None);
        };

        (
            Self::get_string_file_config(settings, "BUNDLE_PATH"),
            Self::get_bool_file_config(settings, "BUNDLE_PATH__SYSTEM"),
            Self::get_bool_file_config(settings, "BUNDLE_DEPLOYMENT"),
        )
    }

    fn get_string_file_config(settings: &Yaml<'_>, key: &str) -> Option<String> {
        settings
            .contains_mapping_key(key)
            .then(|| settings[key].as_str().map(|v| v.to_string()))
            .flatten()
    }

    fn get_bool_file_config(settings: &Yaml<'_>, key: &str) -> Option<bool> {
        settings
            .contains_mapping_key(key)
            .then(|| settings[key].as_bool())
            .flatten()
    }

    fn get_string_env_config(key: &str) -> Option<String> {
        std::env::var(key).ok()
    }

    fn get_bool_env_config(key: &str) -> Option<bool> {
        Self::get_string_env_config(key)
            .as_deref()
            .map(|v| bool::from_str(v).unwrap_or_default())
    }

    fn config_for_dir(dir: &Utf8PathBuf) -> Option<Yaml<'static>> {
        let config_file = dir.join(".bundle/config");

        if !config_file.is_file() {
            return None;
        }

        let config_content = std::fs::read_to_string(&config_file).ok()?;

        let doc = Yaml::load_from_str(&config_content).unwrap();

        if doc.is_empty() {
            return None;
        }

        let settings = doc[0].clone();

        if !settings.is_mapping() {
            return None;
        }

        Some(settings)
    }
}

struct InstallPath {
    explicit_path: Option<String>,

    use_system_gems: bool,
}

impl InstallPath {
    pub fn path(&self) -> Option<Utf8PathBuf> {
        if self.use_system_gems {
            return None;
        }

        let base_path = match &self.explicit_path {
            Some(explicit_path) => absolute(explicit_path),
            None => absolute(".bundle"),
        };

        let Ok(base_path) = base_path else {
            return None;
        };

        Utf8PathBuf::from_path_buf(base_path).ok()
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

        let bundler_settings = BundlerSettings::new(&home_dir, &project_dir);

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

        let bundler_settings = BundlerSettings::new(&home_dir, &project_dir);

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

        let bundler_settings = BundlerSettings::new(&home_dir, &project_dir);

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

        let bundler_settings = BundlerSettings::new(&home_dir, &project_dir);

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

        let bundler_settings = BundlerSettings::new(&home_dir, &project_dir);

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

        let bundler_settings = BundlerSettings::new(&home_dir, &project_dir);

        assert_eq!(None, bundler_settings.path())
    }
}
