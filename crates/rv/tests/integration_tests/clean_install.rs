use crate::common::{RvOutput, RvTest};

impl RvTest {
    pub fn ci(&mut self, args: &[&str]) -> RvOutput {
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

    let gemfile_path = test.cwd.join("Gemfile.empty");
    let gemfile = fs_err::read_to_string("../rv-lockfile/tests/inputs/Gemfile.empty").unwrap();
    let _ = fs_err::write(
        gemfile_path,
        gemfile.replace("https://rubygems.org", &test.server_url()),
    );

    let lockfile_path = test.cwd.join("Gemfile.empty.lock");
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
    output.assert_stdout_contains(
        "Installed Ruby version ruby-3.4.8 to /tmp/home/.local/share/rv/rubies",
    );
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
    output.assert_stdout_contains(
        "Installed Ruby version ruby-4.0.1 to /tmp/home/.local/share/rv/rubies",
    );
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
    let files_sorted = find_all_files_in_dir(test.cwd.as_ref());
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
    let files_sorted = find_all_files_in_dir(test.cwd.as_ref());
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

#[cfg(unix)]
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
