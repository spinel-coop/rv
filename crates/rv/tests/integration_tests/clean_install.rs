use crate::common::{RvOutput, RvTest};

impl RvTest {
    pub fn ci(&mut self, args: &[&str]) -> RvOutput {
        self.env
            .insert("BUNDLE_PATH".into(), self.current_dir().join("app").into());
        self.rv(&[&["ci"], args].concat())
    }
}

#[test]
fn test_clean_install_download_test_gem() {
    let mut test = RvTest::new();

    test.create_ruby_dir("ruby-4.0.1");

    test.use_gemfile("../rv-lockfile/tests/inputs/Gemfile.testsource");
    test.use_lockfile("../rv-lockfile/tests/inputs/Gemfile.testsource.lock");
    test.replace_source("http://gems.example.com", &test.server_url());

    let mock = test.mock_gem_download("test-gem-1.0.0.gem").create();

    let output = test.ci(&["--verbose"]);

    output.assert_success();
    mock.assert();
}

#[test]
fn test_clean_install_input_validation() {
    let mut test = RvTest::new();

    test.create_ruby_dir("ruby-4.0.1");

    // Test missing a lockfile fails
    let output = test.ci(&[]);
    output.assert_failure();
    assert_eq!(
        output.normalized_stderr(),
        "Error: CiError(MissingImplicitLockfile)\n",
    );

    // Test using only a lockfile works
    test.use_lockfile("../rv-lockfile/tests/inputs/Gemfile.empty.lock");
    let output = test.ci(&[]);
    output.assert_success();

    let gemfile_path = test.current_dir().join("Gemfile.empty");
    let gemfile = fs_err::read_to_string("../rv-lockfile/tests/inputs/Gemfile.empty").unwrap();
    let _ = fs_err::write(
        gemfile_path,
        gemfile.replace("https://rubygems.org", &test.server_url()),
    );

    let lockfile_path = test.current_dir().join("Gemfile.empty.lock");
    let lockfile =
        fs_err::read_to_string("../rv-lockfile/tests/inputs/Gemfile.empty.lock").unwrap();
    let _ = fs_err::write(
        lockfile_path,
        lockfile.replace("https://rubygems.org", &test.server_url()),
    );

    // Test passing an explicit Gemfile works
    let output = test.ci(&["--gemfile", "Gemfile.empty"]);
    output.assert_success();

    let project_dir = test.temp_root().join("project");
    std::fs::create_dir_all(project_dir.as_path()).unwrap();

    // Test pasing a Gemfile in a subdirectory works
    let gemfile_path = project_dir.join("Gemfile");
    let gemfile = fs_err::read_to_string("../rv-lockfile/tests/inputs/Gemfile.empty").unwrap();
    let _ = fs_err::write(
        gemfile_path,
        gemfile.replace("https://rubygems.org", &test.server_url()),
    );

    let lockfile_path = project_dir.join("Gemfile.lock");
    let lockfile =
        fs_err::read_to_string("../rv-lockfile/tests/inputs/Gemfile.empty.lock").unwrap();
    let _ = fs_err::write(
        lockfile_path,
        lockfile.replace("https://rubygems.org", &test.server_url()),
    );

    let output = test.ci(&["--gemfile", "project/Gemfile"]);
    output.assert_success();

    // Test passing a missing gemfile gives a nice error
    let output = test.ci(&["--gemfile", "Gemfile.missing"]);
    output.assert_failure();
    assert_eq!(
        output.normalized_stderr(),
        "Error: CiError(MissingGemfile(\"Gemfile.missing\"))\n",
    );

    // Test passing an invalid gemfile gives a nice error
    let output = test.ci(&["--gemfile", "/"]);
    output.assert_failure();
    assert_eq!(
        output.normalized_stderr(),
        "Error: CiError(InvalidGemfilePath(\"/\"))\n",
    );
}

#[test]
fn test_clean_install_respects_ruby() {
    let mut test = RvTest::new();

    let project_dir = test.temp_root().join("project");
    std::fs::create_dir_all(project_dir.as_path()).unwrap();
    std::fs::write(project_dir.join(".ruby-version"), b"3.4.8").unwrap();
    test.cwd = project_dir;

    let mock = test.mock_ruby_download("3.4.8").create();

    test.use_gemfile("../rv-lockfile/tests/inputs/Gemfile.empty");
    test.use_lockfile("../rv-lockfile/tests/inputs/Gemfile.empty.lock");
    test.replace_source("https://rubygems.org", &test.server_url());

    let output = test.ci(&["--verbose"]);
    output.assert_success();
    mock.assert();
    output
        .assert_stdout_contains("Installed Ruby version 3.4.8 to /tmp/home/.local/share/rv/rubies");
}

#[test]
fn test_ci_respects_rv_setting_gem_home() {
    use camino_tempfile::Utf8TempDir;

    let mut test = RvTest::new();
    test.create_ruby_dir("ruby-4.0.1");

    let project_dir = test.temp_root().join("project");
    std::fs::create_dir_all(project_dir.as_path()).unwrap();
    let temp_dir = Utf8TempDir::new().expect("Failed to create temporary directory");

    let config_content = format!(
        r#"
rv{{
  install-path "{}"
}}
"#,
        temp_dir.path()
    );

    std::fs::write(project_dir.join("rv.kdl"), config_content).expect("Failed to write config");

    test.cwd = project_dir;

    test.use_gemfile("../rv-lockfile/tests/inputs/Gemfile.symlink-test");
    test.use_lockfile("../rv-lockfile/tests/inputs/Gemfile.symlink-test.lock");
    test.replace_source("http://gems.example.com", &test.server_url());

    let mock = test.mock_gem_download("symlink-test-1.0.0.gem").create();

    let output = test.ci(&[]);
    output.assert_success();
    output.assert_stdout_contains(&format!(
        "1 gems installed to {}/ruby/4.0.0",
        temp_dir.path()
    ));
    mock.assert();
}

#[test]
fn test_clean_install_ignores_ruby_requests_outside_of_the_current_project() {
    let mut test = RvTest::new();

    let project_parent = test.temp_root();
    let project_dir = project_parent.join("project");
    std::fs::create_dir_all(project_dir.as_path()).unwrap();
    std::fs::write(project_parent.join(".ruby-version"), b"3.4.8").unwrap();
    test.cwd = project_dir;

    test.use_gemfile("../rv-lockfile/tests/inputs/Gemfile.empty");
    test.use_lockfile("../rv-lockfile/tests/inputs/Gemfile.empty.lock");
    test.replace_source("https://rubygems.org", &test.server_url());

    let ruby_mock = test.mock_ruby_download("4.0.1").create();
    let mock = test.mock_releases(["3.4.8", "4.0.1"].to_vec());

    let output = test.ci(&["--verbose"]);
    output.assert_success();
    ruby_mock.assert();
    mock.assert();
    output
        .assert_stdout_contains("Installed Ruby version 4.0.1 to /tmp/home/.local/share/rv/rubies");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn test_clean_install_native_macos_aarch64() {
    let mut test = RvTest::new();
    test.create_ruby_dir("ruby-4.0.1");

    test.use_gemfile("../rv-lockfile/tests/inputs/Gemfile.testwithnative");
    test.use_lockfile("../rv-lockfile/tests/inputs/Gemfile.testwithnative.lock");
    test.replace_source("https://rubygems.org", &test.server_url());
    let mock = test
        .mock_gem_download("ffi-1.17.2-arm64-darwin.gem")
        .create();

    let output = test.ci(&[]);

    output.assert_success();
    mock.assert();

    // Store a snapshot of all the files `rv ci` created.
    let files_sorted = find_all_files_in_dir(test.current_dir().as_ref());
    insta::assert_snapshot!(files_sorted);
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn test_clean_install_native_linux_x86_64() {
    let mut test = RvTest::new();
    test.create_ruby_dir("ruby-4.0.1");

    test.use_gemfile("../rv-lockfile/tests/inputs/Gemfile.testwithnative");
    test.use_lockfile("../rv-lockfile/tests/inputs/Gemfile.testwithnative.lock");
    test.replace_source("https://rubygems.org", &test.server_url());
    let mock = test
        .mock_gem_download("ffi-1.17.2-x86_64-linux-gnu.gem")
        .create();

    let output = test.ci(&[]);

    output.assert_success();
    mock.assert();

    // Store a snapshot of all the files `rv ci` created.
    let files_sorted = find_all_files_in_dir(test.current_dir().as_ref());
    insta::assert_snapshot!(files_sorted);
}

#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "linux", target_arch = "x86_64")
))]
#[test]
fn test_clean_install_native_and_generic_reinstall() {
    let mut test = RvTest::new();
    test.create_ruby_dir("ruby-4.0.1");

    test.use_gemfile("../rv-lockfile/tests/inputs/Gemfile.testwithnative-and-generic");
    test.use_lockfile("../rv-lockfile/tests/inputs/Gemfile.testwithnative-and-generic.lock");
    test.replace_source("https://rubygems.org", &test.server_url());

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    let package = "ffi-1.17.2-arm64-darwin.gem";
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    let package = "ffi-1.17.2-x86_64-linux-gnu.gem";

    let mock = test.mock_gem_download(package).create();

    let output = test.ci(&[]);

    output.assert_success();
    mock.assert();

    // It should not download or install anything the second time
    let output = test.ci(&[]);

    output.assert_success();
    output.assert_stdout_contains(
        "1 gem already installed in /tmp/app/ruby/4.0.0, skipping installation",
    );
}

/// Test that `rv ci` can install a gem containing symlinks.
///
/// The `symlink-test` gem contains both a file symlink (`default.txt → v1/template.txt`)
/// and a directory symlink (`v2 → v1`). On Windows without Developer Mode, symlink
/// creation fails — our `tar_utils::unpack_tar` falls back to copying. This test
/// verifies the symlinked content is readable regardless of platform.
///
/// Regression test for https://github.com/spinel-coop/rv/issues/586
/// Mirrors the real-world problem from haml-rails 3.0.0.
#[test]
fn test_clean_install_gem_with_symlinks() {
    let mut test = RvTest::new();
    test.create_ruby_dir("ruby-4.0.1");

    test.use_gemfile("../rv-lockfile/tests/inputs/Gemfile.symlink-test");
    test.use_lockfile("../rv-lockfile/tests/inputs/Gemfile.symlink-test.lock");
    test.replace_source("http://gems.example.com", &test.server_url());

    let mock = test.mock_gem_download("symlink-test-1.0.0.gem").create();

    let output = test.ci(&["--verbose"]);
    output.assert_success();
    mock.assert();

    // Verify the gem was unpacked and symlinked content is readable.
    let gem_dir = find_gem_dir(test.current_dir().as_ref(), "symlink-test-1.0.0");

    // Real file
    let real_content = fs_err::read_to_string(gem_dir.join("lib/templates/v1/template.txt"))
        .expect("real file should exist");
    assert_eq!(real_content, "template content v1\n");

    // File symlink (or copy on unprivileged Windows): default.txt → v1/template.txt
    let symlink_content = fs_err::read_to_string(gem_dir.join("lib/templates/default.txt"))
        .expect("file symlink target should be readable");
    assert_eq!(symlink_content, "template content v1\n");

    // Directory symlink (or copy on unprivileged Windows): v2 → v1
    let dir_symlink_content = fs_err::read_to_string(gem_dir.join("lib/templates/v2/template.txt"))
        .expect("directory symlink target should be readable");
    assert_eq!(dir_symlink_content, "template content v1\n");

    let dir_symlink_helper = fs_err::read_to_string(gem_dir.join("lib/templates/v2/helper.txt"))
        .expect("directory symlink should include all files");
    assert_eq!(dir_symlink_helper, "helper content\n");
}

/// Find the unpacked gem directory under BUNDLE_PATH.
/// Gems are installed to `<cwd>/app/ruby/<version>/gems/<gem-full-name>/`.
fn find_gem_dir(cwd: &std::path::Path, gem_full_name: &str) -> camino::Utf8PathBuf {
    let app_dir = cwd.join("app");
    // Find the ruby version directory (e.g., "4.0.0")
    let ruby_dir = std::fs::read_dir(app_dir.join("ruby"))
        .expect("app/ruby should exist")
        .filter_map(|e| e.ok())
        .find(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .expect("should have a ruby version directory");
    let gem_path = ruby_dir.path().join("gems").join(gem_full_name);
    camino::Utf8PathBuf::try_from(gem_path).expect("path should be UTF-8")
}

#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "linux", target_arch = "x86_64")
))]
fn find_all_files_in_dir(cwd: &std::path::Path) -> String {
    let test_dir_contents = std::process::Command::new("find")
        .args([".", "-type", "f"])
        .current_dir(cwd)
        .output()
        .expect("ls should succeed")
        .stdout;
    let test_dir_contents =
        String::from_utf8(test_dir_contents).expect("'find' should return UTF-8 bytes");
    let mut lines: Vec<_> = test_dir_contents
        .lines()
        // This file is created when running with coverage, we don't want to include it.
        .filter(|line| !line.ends_with("profraw"))
        // We don't want to test how rv installs ruby, just the CI files.
        .filter(|line| !line.contains("rv/rubies/"))
        .collect();
    lines.sort();
    lines.join("\n")
}
