use camino::{Utf8Path, Utf8PathBuf};
use camino_tempfile_ext::camino_tempfile::Utf8TempDir;
use mockito::Mock;
use rv_platform::HostPlatform;
use std::{collections::HashMap, process::Command};

#[cfg(unix)]
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
    /// The platform the subprocess sees via `RV_TEST_PLATFORM`.
    pub platform: HostPlatform,
}

impl RvTest {
    pub fn new() -> Self {
        let temp_dir = Utf8TempDir::new().expect("Failed to create temporary directory");

        // Platform is fixed during our integration tests to macOS, so some code paths are not
        // exercised on Windows, like the different ruby download URLs. Eventually we'll probably
        // want to stop mocking this value if possible to get more coverage.
        let platform = HostPlatform::MacosAarch64;

        let mut test = Self {
            cwd: temp_dir.path().into(),
            temp_dir,
            env: HashMap::new(),
            server: mockito::Server::new(),
            platform,
        };

        test.env
            .insert("RV_ROOT_DIR".into(), test.temp_root().into());
        // Set consistent arch/os for cross-platform testing
        test.env
            .insert("RV_TEST_PLATFORM".into(), platform.target_triple().into());

        test.env.insert("RV_TEST_EXE".into(), "/tmp/bin/rv".into());
        test.env.insert("HOME".into(), test.temp_home().into());
        // On Windows, set APPDATA and USERPROFILE so that the Win32 SHGetKnownFolderPath API (used
        // by the `etcetera` crate for data_dir) resolves to the test locations that we expect
        // rather than to paths in the real user profile. This is the same approach used by uv
        // (Astral's Python package manager).
        test.env.insert("APPDATA".into(), test.data_dir().into());
        test.env
            .insert("USERPROFILE".into(), test.temp_home().into());

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
        test.env.insert(
            "RV_WINDOWS_LIST_URL".into(),
            format!(
                "{}/{}",
                test.server.url(),
                "repos/oneclick/rubyinstaller2/releases"
            ),
        );

        // Override the rubies directory so rv looks in the test temp dir.
        // On Windows, etcetera resolves data_dir via the Win32 SHGetKnownFolderPath
        // API which returns the real %APPDATA% path, ignoring our test HOME env var.
        // RUBIES_PATH forces rv to use our temp dir instead.
        let rubies_dir = test.temp_home().join(".local/share/rv/rubies");
        std::fs::create_dir_all(&rubies_dir).expect("Failed to create rubies directory");
        test.env.insert("RUBIES_PATH".into(), rubies_dir.into());

        // Disable caching for tests by default
        test.env.insert("RV_NO_CACHE".into(), "true".into());

        test
    }

    pub fn temp_root(&self) -> Utf8PathBuf {
        Self::canonicalize(self.temp_dir.path())
    }

    pub fn current_dir(&self) -> Utf8PathBuf {
        Self::canonicalize(&self.cwd)
    }

    // Use dunce::canonicalize to avoid Windows 8.3 short paths (e.g. RUNNER~1)
    // and \\?\ UNC prefix that std::fs::canonicalize produces on Windows.
    pub fn canonicalize(dir: &Utf8Path) -> Utf8PathBuf {
        Utf8PathBuf::try_from(dunce::canonicalize(dir).unwrap()).unwrap()
    }

    pub fn enable_cache(&mut self) -> Utf8PathBuf {
        self.env.remove("RV_NO_CACHE");

        let cache_dir = self.temp_root().join("cache");
        self.env
            .insert("RV_CACHE_DIR".into(), cache_dir.clone().into());

        cache_dir
    }

    pub fn set_platform(&mut self, platform: HostPlatform) {
        self.platform = platform;
        self.env
            .insert("RV_TEST_PLATFORM".into(), platform.target_triple().into());
    }

    pub fn temp_home(&self) -> Utf8PathBuf {
        self.temp_root().join("home")
    }

    pub fn data_dir(&self) -> Utf8PathBuf {
        self.temp_home().join(".local/share")
    }

    pub fn legacy_gem_path(&self, version: &str) -> Utf8PathBuf {
        self.temp_home()
            .join(".gem")
            .join("ruby")
            .join(format!("{version}.0"))
    }

    pub fn rv(&self, args: &[&str]) -> RvOutput {
        let mut cmd = self.rv_command();
        cmd.args(args);

        let output = cmd.output().expect("Failed to execute rv command");
        let test_root = self.temp_root().to_string();
        RvOutput { test_root, output }
    }

    pub fn rv_command(&self) -> Command {
        self.command(env!("CARGO_BIN_EXE_rv"))
    }

    #[cfg(unix)]
    pub fn make_session(
        &self,
        shell: Shell,
    ) -> Result<rexpect::session::PtyReplSession, Box<dyn std::error::Error>> {
        use rexpect::reader::Options;
        use rexpect::session::PtyReplSession;

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
        cmd.current_dir(self.current_dir());
        cmd.env_clear();

        // On Windows, preserve essential system env vars. Without SystemRoot,
        // Winsock can't initialize (error 10106) and all HTTP requests fail.
        // Without COMSPEC, .cmd batch scripts can't be executed.
        #[cfg(windows)]
        for var in ["SystemRoot", "SYSTEMDRIVE", "COMSPEC"] {
            if let Ok(val) = std::env::var(var) {
                cmd.env(var, val);
            }
        }

        cmd.envs(&self.env);
        cmd
    }

    /// Mocks the /releases API endpoint. Returns the mock handle
    /// so that tests can optionally assert it was called.
    ///
    /// Assets use the archive suffix for `self.platform`, so the mocked
    /// response matches whatever platform the subprocess is configured for.
    pub fn mock_releases(&mut self, versions: Vec<&str>) -> Mock {
        self.mock_releases_for_platform(versions, self.platform)
    }

    /// Mocks the /releases API endpoint with assets for a specific platform.
    pub fn mock_releases_for_platform(
        &mut self,
        versions: Vec<&str>,
        platform: HostPlatform,
    ) -> Mock {
        use indoc::formatdoc;

        let suffix = platform.archive_suffix();
        let assets = versions
            .into_iter()
            .map(|v| {
                formatdoc!(
                    r#"
            {{
                "name": "ruby-{v}{suffix}",
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

    /// Mocks the rv-ruby /releases endpoint with assets for all NON-WINDOWS platforms.
    ///
    /// Windows uses a separate endpoint (RubyInstaller2), so Windows assets
    /// should NOT appear in the rv-ruby release. Use `mock_windows_releases()`
    /// to mock the Windows endpoint.
    pub fn mock_releases_all_platforms(&mut self, versions: Vec<&str>) -> Mock {
        use indoc::formatdoc;

        let assets: Vec<String> = versions
            .into_iter()
            .flat_map(|v| {
                HostPlatform::all()
                    .iter()
                    .filter(|hp| !hp.is_windows())
                    .map(move |hp| {
                        let suffix = hp.archive_suffix();
                        formatdoc!(
                            r#"
            {{
                "name": "ruby-{v}{suffix}",
                "browser_download_url": "http://..."
            }}"#
                        )
                    })
            })
            .collect();

        let assets_str = assets.join(",\n    ");

        let body = formatdoc!(
            r#"
            {{
                "name": "latest",
                "assets": [{assets_str}]
            }}"#
        );

        self.server
            .mock("GET", "/repos/spinel-coop/rv-ruby/releases/latest")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body)
            .create()
    }

    /// Mocks the RubyInstaller2 /releases endpoint for Windows.
    ///
    /// Returns an array of releases (one per version), each with a
    /// `rubyinstaller-{v}-1-x64.7z` asset. This matches the real
    /// RubyInstaller2 release structure.
    pub fn mock_windows_releases(&mut self, versions: Vec<&str>) -> Mock {
        use indoc::formatdoc;

        let releases: Vec<String> = versions
            .iter()
            .map(|v| {
                formatdoc!(
                    r#"
            {{
                "name": "RubyInstaller-{v}-1",
                "assets": [
                    {{
                        "name": "rubyinstaller-{v}-1-x64.7z",
                        "browser_download_url": "http://..."
                    }}
                ]
            }}"#
                )
            })
            .collect();

        let body = format!("[{}]", releases.join(",\n"));

        self.server
            .mock("GET", "/repos/oneclick/rubyinstaller2/releases")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body)
            .create()
    }

    /// Mock a ruby tarball download for testing
    pub fn mock_ruby_download(&mut self, version: &str) -> Mock {
        let path = self.ruby_tarball_download_path(version);
        let content = self.create_mock_tarball(version);
        self.mock_tarball_download(path, &content)
    }

    pub fn mock_gem_download(&mut self, package: &str) -> Mock {
        let path = self.gem_package_download_path(package);
        let content = fs_err::read(format!("../rv-gem-package/tests/fixtures/{package}")).unwrap();
        self.mock_tarball_download(path, &content)
    }

    /// Mock a tarball download for testing
    pub fn mock_tarball_download(&mut self, path: String, content: &[u8]) -> Mock {
        self.server
            .mock("GET", path.as_str())
            .with_status(200)
            .with_header("content-type", "application/gzip")
            .with_body(content)
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
    pub fn mock_tarball_on_disk(&mut self, version: &str) -> Utf8PathBuf {
        let content = &self.create_mock_tarball(version);
        let filename = &self.make_tarball_file_name(version);
        let temp_dir = self.temp_root().join("tmp");
        std::fs::create_dir_all(&temp_dir).expect("Failed to create TMP directory");
        let full_path = temp_dir.join(filename);
        std::fs::write(&full_path, content).expect("Failed to write path");

        full_path
    }

    pub fn create_mock_tarball(&self, version: &str) -> Vec<u8> {
        use flate2::Compression;
        use flate2::write::GzEncoder;
        use std::io::Write;
        use tar::Builder;

        let mut archive_data = Vec::new();
        {
            let mut builder = Builder::new(&mut archive_data);

            let root = format!("ruby-{version}/");
            let mut dir_header = tar::Header::new_gnu();
            dir_header.set_path(&root).unwrap();
            dir_header.set_size(0);
            dir_header.set_mode(0o755);
            dir_header.set_entry_type(tar::EntryType::Directory);
            dir_header.set_cksum();
            builder.append(&dir_header, std::io::empty()).unwrap();

            let mut bin_dir_header = tar::Header::new_gnu();
            bin_dir_header.set_path(format!("{root}bin/")).unwrap();
            bin_dir_header.set_size(0);
            bin_dir_header.set_mode(0o755);
            bin_dir_header.set_entry_type(tar::EntryType::Directory);
            bin_dir_header.set_cksum();
            builder.append(&bin_dir_header, std::io::empty()).unwrap();

            let mut ruby_header = tar::Header::new_gnu();
            let ruby_executable_name = self.ruby_executable_name();
            let ruby_content = &self.ruby_mock_script("ruby", version);
            ruby_header
                .set_path(format!("{root}bin/{ruby_executable_name}"))
                .unwrap();
            ruby_header.set_size(ruby_content.len() as u64);
            ruby_header.set_mode(0o755);
            ruby_header.set_cksum();
            builder
                .append(&ruby_header, ruby_content.as_bytes())
                .unwrap();

            builder.finish().unwrap();
        }

        let mut gz_data = Vec::new();
        {
            let mut encoder = GzEncoder::new(&mut gz_data, Compression::default());
            encoder.write_all(&archive_data).unwrap();
            encoder.finish().unwrap();
        }

        gz_data
    }

    pub fn ruby_tarball_url(&self, version: &str) -> String {
        format!(
            "{}{}",
            self.server_url(),
            self.ruby_tarball_download_path(version)
        )
    }

    pub fn ruby_tarball_download_path(&self, version: &str) -> String {
        let filename = self.make_tarball_file_name(version);
        format!("/latest/download/{filename}")
    }

    pub fn gem_package_download_path(&self, package: &str) -> String {
        format!("/gems/{}", package)
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

        // Create a mock ruby executable that outputs the expected format for rv-ruby.
        let ruby_exe = bin_dir.join(self.ruby_executable_name());
        let mock_script = self.ruby_mock_script(engine, version);
        std::fs::write(&ruby_exe, mock_script).expect("Failed to create ruby executable");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&ruby_exe).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&ruby_exe, perms).unwrap();
        }

        ruby_dir
    }

    #[cfg(unix)]
    fn ruby_executable_name(&self) -> &str {
        "ruby"
    }

    #[cfg(windows)]
    fn ruby_executable_name(&self) -> &str {
        "ruby.cmd"
    }

    #[cfg(unix)]
    fn ruby_mock_script(&self, engine: &str, version: &str) -> String {
        format!(
            "#!/bin/bash\n\
             echo \"{engine}\"\n\
             echo \"{version}\"\n\
             echo \"aarch64-darwin23\"\n\
             echo \"aarch64\"\n\
             echo \"darwin23\"\n\
             echo \"\"\n"
        )
    }

    #[cfg(windows)]
    fn ruby_mock_script(&self, engine: &str, version: &str) -> String {
        format!(
            "@echo off\r\n\
             echo {engine}\r\n\
             echo {version}\r\n\
             echo aarch64-darwin23\r\n\
             echo aarch64\r\n\
             echo darwin23\r\n\
             echo.\r\n"
        )
    }

    pub fn use_gemfile(&self, path: &str) {
        let gemfile = fs_err::read_to_string(path).unwrap();
        let _ = fs_err::write(self.current_dir().join("Gemfile"), &gemfile);
    }

    pub fn use_lockfile(&self, path: &str) {
        let lockfile = fs_err::read_to_string(path).unwrap();
        let _ = fs_err::write(self.current_dir().join("Gemfile.lock"), &lockfile);
    }

    pub fn replace_source(&self, from: &str, to: &str) {
        let gemfile_path = self.current_dir().join("Gemfile");
        let gemfile = fs_err::read_to_string(&gemfile_path).unwrap();
        let _ = fs_err::write(gemfile_path, gemfile.replace(from, to));

        let lockfile_path = self.current_dir().join("Gemfile.lock");
        let lockfile = fs_err::read_to_string(&lockfile_path).unwrap();
        let _ = fs_err::write(lockfile_path, lockfile.replace(from, to));
    }

    pub fn write_ruby_version_file(&self, version: &str) {
        let path = self.temp_root().join(".ruby-version");
        fs_err::write(path, format!("{version}\n")).expect("Failed to write .ruby-version file");
    }

    fn make_tarball_file_name(&self, version: &str) -> String {
        let suffix = self.make_platform_suffix();
        format!("ruby-{version}.{suffix}.tar.gz")
    }

    /// Returns the ruby arch string matching the default test platform (`MacosAarch64`).
    ///
    /// Uses `HostPlatform::MacosAarch64` to match the hardcoded default in `RvTest::new()`,
    /// NOT `HostPlatform::current()`, because the test process doesn't have
    /// `RV_TEST_PLATFORM` set — only the subprocess does.
    fn make_platform_suffix(&self) -> String {
        HostPlatform::MacosAarch64.ruby_arch_str().to_string()
    }
}

pub fn is_shell_installed(shell_name: &str) -> bool {
    Command::new(shell_name).arg("--version").output().is_ok()
}

#[derive(Debug)]
pub struct RvOutput {
    pub output: std::process::Output,
    pub test_root: String,
}

impl RvOutput {
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

    #[track_caller]
    pub fn assert_stdout_contains(&self, expected: &str) -> &Self {
        let got = self.normalized_stdout();

        assert!(
            got.contains(expected),
            "# EXPECTED STDOUT TO INCLUDE\n{expected}\n\n# GOT\n{got}\n",
        );
        self
    }

    #[track_caller]
    pub fn assert_stderr_contains(&self, expected: &str) -> &Self {
        let got = self.normalized_stderr();

        assert!(
            got.contains(expected),
            "# EXPECTED STDERR TO INCLUDE\n{expected}\n\n# GOT\n{got}\n",
        );
        self
    }

    pub fn stdout(&self) -> String {
        String::from_utf8_lossy(&self.output.stdout).to_string()
    }

    pub fn stderr(&self) -> String {
        String::from_utf8_lossy(&self.output.stderr).to_string()
    }

    /// Normalize output for cross-platform snapshot testing
    pub fn normalized_stdout(&self) -> String {
        let mut output = self.stdout();

        // Normalize CRLF to LF before any other processing
        if cfg!(windows) {
            output = output.replace("\r\n", "\n");
        }

        // Replace Windows path separators with forward slashes.
        // First replace double-backslash (JSON-escaped `\` produces `\\` in output)
        // with a single forward slash, then replace remaining single backslashes.
        // Without this ordering, `\\` becomes `//` instead of `/`.
        // Then restore PowerShell provider paths (Env:\, Function:\) that use
        // backslash as a provider separator, not a file path separator.
        if cfg!(windows) {
            output = output.replace("\\\\", "/");
            output = output.replace('\\', "/");
            output = output.replace("Env:/", "Env:\\");
            output = output.replace("Function:/", "Function:\\");
        }

        // Remove test root from paths. On Windows, also match the forward-slash
        // version since we already converted backslashes above.
        output = output.replace(&self.test_root, "/tmp");
        if cfg!(windows) {
            output = output.replace(&self.test_root.replace('\\', "/"), "/tmp");
        }

        // Normalize Windows Ruby executable names so snapshots match across platforms.
        // On Windows, create_ruby_dir() creates ruby.cmd (since we can't create ELF/Mach-O
        // executables from tests), so list output shows bin/ruby.cmd instead of bin/ruby.
        if cfg!(windows) {
            output = output.replace("/ruby.cmd", "/ruby");
            output = output.replace("/ruby.exe", "/ruby");
        }

        output
    }

    /// Normalize stderr for cross-platform snapshot testing
    pub fn normalized_stderr(&self) -> String {
        let mut output = self.stderr();

        // Normalize CRLF to LF before any other processing
        if cfg!(windows) {
            output = output.replace("\r\n", "\n");
        }

        // Replace Windows path separators with forward slashes
        if cfg!(windows) {
            output = output.replace("\\\\", "/");
            output = output.replace('\\', "/");
        }

        // Normalize the binary name: clap uses argv[0] which is `rv.exe` on Windows
        if cfg!(windows) {
            output = output.replace("rv.exe", "rv");
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    use std::os::unix::process::ExitStatusExt;
    #[cfg(windows)]
    use std::os::windows::process::ExitStatusExt;

    fn fake_output(stdout: &str) -> RvOutput {
        RvOutput {
            output: std::process::Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: stdout.as_bytes().to_vec(),
                stderr: Vec::new(),
            },
            test_root: "/private/var/folders/abc123".into(),
        }
    }

    #[test]
    fn test_normalized_stdout_replaces_test_root() {
        let out = fake_output("ruby at /private/var/folders/abc123/home/.local/share/rv\n");
        assert_eq!(
            out.normalized_stdout(),
            "ruby at /tmp/home/.local/share/rv\n"
        );
    }

    #[test]
    fn test_normalized_stdout_preserves_other_content() {
        let out = fake_output("hello world\n");
        assert_eq!(out.normalized_stdout(), "hello world\n");
    }
}
