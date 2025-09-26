use camino::Utf8PathBuf;
use camino_tempfile_ext::camino_tempfile::Utf8TempDir;
use mockito::Mock;
use std::{collections::HashMap, process::Command};

pub struct RvTest {
    pub temp_dir: Utf8TempDir,
    pub cwd: Utf8PathBuf,
    pub env: HashMap<String, String>,
    // For mocking the releases json from Github API
    pub server: mockito::ServerGuard,
}

impl RvTest {
    pub fn new() -> Self {
        let temp_dir = Utf8TempDir::new().expect("Failed to create temporary directory");
        let cwd = temp_dir.path().into();

        let mut test = Self {
            temp_dir,
            cwd,
            env: HashMap::new(),
            server: mockito::Server::new(),
        };

        test.env
            .insert("RV_ROOT_DIR".into(), test.temp_dir.path().as_str().into());
        // Set consistent arch/os for cross-platform testing
        test.env
            .insert("RV_TEST_PLATFORM".into(), "aarch64-apple-darwin".into()); // For mocking current_platform::CURRENT_PLATFORM
        test.env.insert("RV_TEST_ARCH".into(), "aarch64".into());
        test.env.insert("RV_TEST_OS".into(), "macos".into());

        test.env.insert("RV_TEST_EXE".into(), "/tmp/bin/rv".into());
        test.env.insert("HOME".into(), "/tmp/home".into());
        test.env.insert("RV_DISABLE_INDICATIF".into(), "1".into()); // Disable indicatif progress bars in tests due to a bug in tracing-indicatif

        // Disable network requests by default
        test.env.insert("RV_RELEASES_URL".into(), test.server.url());

        // Disable caching for tests by default
        test.env.insert("RV_NO_CACHE".into(), "true".into());

        test
    }

    pub fn rv(&self, args: &[&str]) -> RvOutput {
        let mut cmd = self.rv_command();
        cmd.args(args);

        let output = cmd.output().expect("Failed to execute rv command");
        RvOutput::new(self.temp_dir.path().as_str(), output)
    }

    pub fn rv_command(&self) -> Command {
        self.command(env!("CARGO_BIN_EXE_rv"))
    }

    pub fn command<S: AsRef<std::ffi::OsStr>>(&self, program: S) -> Command {
        let mut cmd = Command::new(program);
        cmd.current_dir(&self.cwd);
        cmd.env_clear().envs(&self.env);
        cmd
    }

    /// Mocks the /releases API endpoint. Returns the mock handle
    /// so that tests can optionally assert it was called.
    pub fn mock_releases(&mut self, body: &str) -> Mock {
        self.server
            .mock("GET", "/repos/spinel-coop/rv-ruby/releases")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body)
            .create()
    }

    /// Mock a tarball download for testing
    pub fn mock_tarball_download(&mut self, filename: &str, content: &[u8]) -> Mock {
        let path = format!("/{}", filename);
        self.server
            .mock("GET", path.as_str())
            .with_status(200)
            .with_header("content-type", "application/gzip")
            .with_body(content)
    }

    /// Get the server URL for constructing download URLs
    pub fn server_url(&self) -> String {
        self.server.url()
    }

    pub fn create_ruby_dir(&self, name: &str) -> Utf8PathBuf {
        let ruby_dir = self.temp_dir.path().join("opt").join("rubies").join(name);
        std::fs::create_dir_all(&ruby_dir).expect("Failed to create ruby directory");

        let bin_dir = ruby_dir.join("bin");
        std::fs::create_dir_all(&bin_dir).expect("Failed to create bin directory");

        // Extract Ruby information from directory name
        // Extract version from directory name: ruby-3.1.4 -> 3.1.4
        let version = if let Some(dash_pos) = name.find('-') {
            &name[dash_pos + 1..]
        } else {
            name
        };

        // Extract engine from directory name: ruby-3.1.4 -> ruby, jruby-9.4.0.0 -> jruby
        let engine = if let Some(dash_pos) = name.find('-') {
            &name[..dash_pos]
        } else {
            "ruby"
        };

        // Create a mock ruby executable that outputs the expected format for rv-ruby
        let ruby_exe = bin_dir.join("ruby");
        let mock_script = format!(
            r#"#!/bin/bash

echo "{engine}"
echo "{version}"
echo "aarch64-darwin23"
echo "aarch64"
echo "darwin23"
echo ""
"#
        );
        std::fs::write(&ruby_exe, mock_script).expect("Failed to create ruby executable");

        // Make it executable on Unix systems
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&ruby_exe).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&ruby_exe, perms).unwrap();
        }

        ruby_dir
    }
}

pub struct RvOutput {
    pub output: std::process::Output,
    pub test_root: String,
}

impl RvOutput {
    pub fn new(test_root: &str, output: std::process::Output) -> Self {
        Self {
            output,
            test_root: test_root.into(),
        }
    }

    pub fn success(&self) -> bool {
        self.output.status.success()
    }

    #[track_caller]
    pub fn assert_success(&self) -> &Self {
        assert!(
            self.success(),
            "Expected command to succeed, got:\n\n# STDERR\n{}\n# STDOUT\n{}\n# STATUS {:?}",
            str::from_utf8(&self.output.stderr).unwrap(),
            str::from_utf8(&self.output.stdout).unwrap(),
            self.output.status
        );
        self
    }

    #[track_caller]
    pub fn assert_failure(&self) -> &Self {
        assert!(
            !self.success(),
            "Expected command to fail, got:\n\n# STDERR\n{}\n# STDOUT\n{}",
            str::from_utf8(&self.output.stderr).unwrap(),
            str::from_utf8(&self.output.stdout).unwrap(),
        );
        self
    }

    pub fn stdout(&self) -> String {
        String::from_utf8_lossy(&self.output.stdout).to_string()
    }

    #[allow(dead_code)]
    pub fn stderr(&self) -> String {
        String::from_utf8_lossy(&self.output.stderr).to_string()
    }

    /// Normalize output for cross-platform snapshot testing
    pub fn normalized_stdout(&self) -> String {
        let mut output = self.stdout();

        // Replace Windows path separators with forward slashes
        if cfg!(windows) {
            output = output.replace('\\', "/");
        }

        // Remove test root from paths
        let mut full_test_root = self.test_root.clone();
        // On macOS, the test root might be prefixed with "/private"
        if cfg!(target_os = "macos") {
            full_test_root.insert_str(0, "/private");
        }
        output.replace(&full_test_root, "")
    }

    /// Normalize stderr for cross-platform snapshot testing
    #[allow(dead_code)]
    pub fn normalized_stderr(&self) -> String {
        let mut output = self.stderr();

        // Replace Windows path separators with forward slashes
        if cfg!(windows) {
            output = output.replace('\\', "/");
        }

        output.to_string()
    }
}
