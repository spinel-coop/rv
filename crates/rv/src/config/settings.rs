use camino::Utf8PathBuf;
use saphyr::{LoadableYamlNode, Yaml};
use std::str::FromStr;

pub struct Settings {
    // Location to look for user config
    pub home_dir: Utf8PathBuf,

    // Location to look for project config
    pub project_dir: Utf8PathBuf,

    // Global gem home
    pub gem_home: Utf8PathBuf,

    pub gem_scope: String,
}

impl Settings {
    pub fn path(&self) -> Utf8PathBuf {
        let local = self.local_config();
        let env = self.env_config();
        let global = self.global_config();
        let mut use_deployment = None;

        for (path, path_system, deployment) in [local, env, global] {
            if use_deployment.is_none() {
                use_deployment = deployment;
            };

            if path.is_none() && path_system.is_none() {
                continue;
            };

            let install_path = InstallPath {
                explicit_path: path.clone().map(Utf8PathBuf::from),
                use_system_gems: path_system.unwrap_or(path.is_none()),
                system_path: self.gem_home.clone(),
                gem_scope: self.gem_scope.clone(),
            };

            return install_path.path();
        }

        let install_path = InstallPath {
            explicit_path: use_deployment.and(Some(Utf8PathBuf::from("vendor/bundle"))),
            use_system_gems: true,
            system_path: self.gem_home.clone(),
            gem_scope: self.gem_scope.clone(),
        };

        install_path.path()
    }

    pub fn local_config(&self) -> (Option<String>, Option<bool>, Option<bool>) {
        self.config_for_dir(self.project_dir.clone())
    }

    pub fn env_config(&self) -> (Option<String>, Option<bool>, Option<bool>) {
        (
            std::env::var("BUNDLE_PATH").ok(),
            std::env::var("BUNDLE_PATH__SYSTEM")
                .ok()
                .as_deref()
                .map(|v| bool::from_str(v).unwrap_or_default()),
            std::env::var("BUNDLE_DEPLOYMENT")
                .ok()
                .as_deref()
                .map(|v| bool::from_str(v).unwrap_or_default()),
        )
    }

    pub fn global_config(&self) -> (Option<String>, Option<bool>, Option<bool>) {
        self.config_for_dir(self.home_dir.clone())
    }

    pub fn config_for_dir(&self, dir: Utf8PathBuf) -> (Option<String>, Option<bool>, Option<bool>) {
        let config_file = dir.join(".bundle/config");

        if !config_file.is_file() {
            return (None, None, None);
        }

        let Some(config_content) = std::fs::read_to_string(&config_file).ok() else {
            return (None, None, None);
        };

        let doc = Yaml::load_from_str(&config_content).unwrap();
        let settings = &doc[0];

        if !settings.is_mapping() {
            return (None, None, None);
        }

        (
            settings
                .contains_mapping_key("BUNDLE_PATH")
                .then(|| settings["BUNDLE_PATH"].as_str().map(|v| v.to_string()))
                .flatten(),
            settings
                .contains_mapping_key("BUNDLE_PATH__SYSTEM")
                .then(|| settings["BUNDLE_PATH__SYSTEM"].as_bool())
                .flatten(),
            settings
                .contains_mapping_key("BUNDLE_DEPLOYMENT")
                .then(|| settings["BUNDLE_DEPLOYMENT"].as_bool())
                .flatten(),
        )
    }
}

struct InstallPath {
    explicit_path: Option<Utf8PathBuf>,

    system_path: Utf8PathBuf,

    use_system_gems: bool,

    gem_scope: String,
}

impl InstallPath {
    pub fn path(&self) -> Utf8PathBuf {
        if self.use_system_gems {
            self.base_path()
        } else {
            self.base_path().join(self.gem_scope.clone())
        }
    }

    fn base_path(&self) -> Utf8PathBuf {
        if let Some(path) = &self.explicit_path {
            return path.clone();
        }

        if self.use_system_gems {
            self.system_path.clone()
        } else {
            self.local_path()
        }
    }

    fn local_path(&self) -> Utf8PathBuf {
        Utf8PathBuf::from(".bundle")
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

        let gem_home = temp_dir.path().join("gems");
        let gem_scope = "ruby/4.0.1".to_string();

        let settings = Settings {
            home_dir,
            project_dir,
            gem_home,
            gem_scope,
        };

        let config_content = r#"---

BUNDLE_PATH: foo
"#;

        std::fs::write(&config_file, config_content).expect("Failed to write config");

        assert_eq!("foo/ruby/4.0.1", settings.path().to_string())
    }

    #[test]
    fn test_global_config() {
        let temp_dir = Utf8TempDir::new().expect("Failed to create temporary directory");

        let home_dir = temp_dir.path().join("home");
        let project_dir = temp_dir.path().join("project");

        let config_dir = home_dir.join(".bundle");
        std::fs::create_dir_all(&config_dir).unwrap();
        let config_file = config_dir.join("config");

        let gem_home = temp_dir.path().join("gems");
        let gem_scope = "ruby/4.0.1".to_string();

        let settings = Settings {
            home_dir,
            project_dir,
            gem_home,
            gem_scope,
        };

        let config_content = r#"---

BUNDLE_PATH: foo
"#;

        std::fs::write(&config_file, config_content).expect("Failed to write config");

        assert_eq!("foo/ruby/4.0.1", settings.path().to_string())
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

        let gem_home = temp_dir.path().join("gems");
        let gem_scope = "ruby/4.0.1".to_string();

        let settings = Settings {
            home_dir,
            project_dir,
            gem_home,
            gem_scope,
        };

        let global_config_content = r#"---

BUNDLE_PATH: foo
"#;

        std::fs::write(&global_config_file, global_config_content).expect("Failed to write config");

        let local_config_content = r#"---

BUNDLE_PATH: bar
"#;

        std::fs::write(&local_config_file, local_config_content).expect("Failed to write config");
        assert_eq!("bar/ruby/4.0.1", settings.path().to_string())
    }

    #[test]
    fn test_local_with_path_system() {
        let temp_dir = Utf8TempDir::new().expect("Failed to create temporary directory");

        let home_dir = temp_dir.path().join("home");
        let project_dir = temp_dir.path().join("project");

        let local_config_dir = project_dir.join(".bundle");
        std::fs::create_dir_all(&local_config_dir).unwrap();
        let local_config_file = local_config_dir.join("config");

        let gem_home = temp_dir.path().join("gems");
        let gem_scope = "ruby/4.0.1".to_string();

        let settings = Settings {
            home_dir,
            project_dir,
            gem_home: gem_home.clone(),
            gem_scope,
        };

        let local_config_content = r#"---

BUNDLE_PATH__SYSTEM: true
"#;

        std::fs::write(&local_config_file, local_config_content).expect("Failed to write config");
        assert_eq!(gem_home, settings.path())
    }
}
