use camino::Utf8PathBuf;
use camino_tempfile_ext::camino_tempfile::Utf8TempDir;
use mockito::Mock;
use rexpect::{reader::Options, session::PtyReplSession};
use std::{collections::HashMap, process::Command};

pub struct Shell {
    pub name: &'static str,
    pub startup_flag: &'static str,
    pub prompt_setter: &'static str,
}

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
            .insert("RV_ROOT_DIR".into(), test.temp_root().as_str().into());
        // Set consistent arch/os for cross-platform testing
        test.env
            .insert("RV_TEST_PLATFORM".into(), "aarch64-apple-darwin".into()); // For mocking current_platform::CURRENT_PLATFORM

        test.env.insert("RV_TEST_EXE".into(), "/tmp/bin/rv".into());
        test.env.insert("HOME".into(), test.temp_home().into());
        test.env
            .insert("BUNDLE_PATH".into(), test.cwd.join("app").into());

        // Disable network requests by default
        test.env.insert(
            "RV_LIST_URL".into(),
            format!(
                "{}/{}",
                test.server.url(),
                "repos/spinel-coop/rv-ruby/releases/latest"
            ),
        );
        test.env.insert(
            "RV_INSTALL_URL".into(),
            format!("{}/{}", test.server.url(), "latest/download"),
        );

        // Disable caching for tests by default
        test.env.insert("RV_NO_CACHE".into(), "true".into());

        test
    }

    pub fn temp_root(&self) -> Utf8PathBuf {
        self.temp_dir.path().canonicalize_utf8().unwrap()
    }

    pub fn enable_cache(&mut self) -> Utf8PathBuf {
        self.env.remove("RV_NO_CACHE");

        let cache_dir = self.temp_root().join("cache");
        self.env
            .insert("RV_CACHE_DIR".into(), cache_dir.as_str().into());

        cache_dir
    }

    pub fn temp_home(&self) -> Utf8PathBuf {
        self.temp_root().join("home")
    }

    pub fn legacy_gem_path(&self, version: &str) -> Utf8PathBuf {
        self.temp_home().join(".gem").join("ruby").join(version)
    }

    pub fn rv(&self, args: &[&str]) -> RvOutput {
        let mut cmd = self.rv_command();
        cmd.args(args);

        let output = cmd.output().expect("Failed to execute rv command");
        RvOutput::new(self.temp_root().as_str(), output)
    }

    pub fn rv_command(&self) -> Command {
        self.command(env!("CARGO_BIN_EXE_rv"))
    }

    pub fn make_session(&self, shell: Shell) -> Result<PtyReplSession, Box<dyn std::error::Error>> {
        let mut cmd = self.command(shell.name);
        cmd.arg(shell.startup_flag);
        cmd.env("TERM", "xterm-256color").env_remove("RV_TEST_EXE");

        let pty_session = rexpect::spawn_with_options(
            cmd,
            Options {
                timeout_ms: Some(4000),
                strip_ansi_escape_codes: true,
            },
        )?;
        let mut session = PtyReplSession {
            prompt: "PEXPECT>".to_owned(),
            pty_session,
            quit_command: Some("builtin exit".to_owned()),
            echo_on: false,
        };

        session.send_line(shell.prompt_setter)?;
        session.wait_for_prompt()?;

        session.send_line(&format!(
            "eval \"$({} shell init {})\"",
            self.rv_command().get_program().display(),
            shell.name,
        ))?;
        session.wait_for_prompt()?;

        Ok(session)
    }

    pub fn command<S: AsRef<std::ffi::OsStr>>(&self, program: S) -> Command {
        let mut cmd = Command::new(program);
        cmd.current_dir(&self.cwd);
        cmd.env_clear().envs(&self.env);
        cmd
    }

    /// Mocks the /releases API endpoint. Returns the mock handle
    /// so that tests can optionally assert it was called.
    pub fn mock_releases(&mut self, versions: Vec<&str>) -> Mock {
        use indoc::formatdoc;

        let assets = versions
            .into_iter()
            .map(|v| {
                formatdoc!(
                    r#"
            {{
                "name": "ruby-{v}.arm64_sonoma.tar.gz",
                "browser_download_url": "http://..."
            }}"#
                )
            })
            .collect::<Vec<_>>()
            .join(",\n    ");

        let body = formatdoc!(
            r#"
            {{
                "name": "latest",
                "assets": [{assets}]
            }}"#
        );

        self.server
            .mock("GET", "/repos/spinel-coop/rv-ruby/releases/latest")
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

    pub fn mock_gem_download(&mut self, filename: &str, content: &[u8]) -> Mock {
        let path = format!("gems/{}", filename);
        self.mock_tarball_download(&path, content)
    }

    pub fn mock_info_endpoint(&mut self, name: &str, content: &[u8]) -> Mock {
        let path = format!("/info/{}", name);
        self.server
            .mock("GET", path.as_str())
            .with_status(200)
            .with_header("content-type", "text/plain; charset=utf-8")
            .with_body(content)
    }

    /// Mock a tarball on disk for testing
    pub fn mock_tarball_on_disk(&mut self, filename: &str, content: &[u8]) -> Utf8PathBuf {
        let temp_dir = self.temp_root().join("tmp");
        std::fs::create_dir_all(&temp_dir).expect("Failed to create TMP directory");
        let full_path = temp_dir.join(filename);
        std::fs::write(&full_path, content).expect("Failed to write path");

        full_path
    }

    /// Get the server URL for constructing download URLs
    pub fn server_url(&self) -> String {
        self.server.url()
    }

    pub fn create_ruby_dir(&self, name: &str) -> Utf8PathBuf {
        let ruby_dir = self.temp_home().join(".local/share/rv/rubies").join(name);
        std::fs::create_dir_all(&ruby_dir).expect("Failed to create ruby directory");

        let bin_dir = ruby_dir.join("bin");
        std::fs::create_dir_all(&bin_dir).expect("Failed to create bin directory");

        let man_dir = ruby_dir.join("share/man");
        std::fs::create_dir_all(&man_dir).expect("Failed to create man directory");

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

    pub fn use_gemfile(&self, path: &str) {
        let gemfile = fs_err::read_to_string(path).unwrap();
        let _ = fs_err::write(self.cwd.join("Gemfile"), &gemfile);
    }

    pub fn use_lockfile(&self, path: &str) {
        let lockfile = fs_err::read_to_string(path).unwrap();
        let _ = fs_err::write(self.cwd.join("Gemfile.lock"), &lockfile);
    }

    pub fn replace_source(&self, from: &str, to: &str) {
        let gemfile_path = self.cwd.join("Gemfile");
        let gemfile = fs_err::read_to_string(&gemfile_path).unwrap();
        let _ = fs_err::write(gemfile_path, gemfile.replace(from, to));

        let lockfile_path = self.cwd.join("Gemfile.lock");
        let lockfile = fs_err::read_to_string(&lockfile_path).unwrap();
        let _ = fs_err::write(lockfile_path, lockfile.replace(from, to));
    }
}

#[derive(Debug)]
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
        output.replace(&self.test_root, "/tmp")
    }

    /// Normalize stderr for cross-platform snapshot testing
    #[allow(dead_code)]
    pub fn normalized_stderr(&self) -> String {
        let mut output = self.stderr();

        // Replace Windows path separators with forward slashes
        if cfg!(windows) {
            output = output.replace('\\', "/");
        }

        output
    }
}
