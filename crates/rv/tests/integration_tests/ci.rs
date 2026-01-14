use crate::common::{RvOutput, RvTest};

impl RvTest {
    pub fn ci(&mut self, args: &[&str]) -> RvOutput {
        self.env.remove("RV_INSTALL_URL");
        self.rv(&[&["ci"], args].concat())
    }
}

#[test]
fn test_clean_install_download_test_gem() {
    let mut test = RvTest::new();

    let releases_mock = test.mock_releases(["4.0.0"].to_vec());

    test.use_gemfile("../rv-lockfile/tests/inputs/Gemfile.testsource");
    test.use_lockfile("../rv-lockfile/tests/inputs/Gemfile.testsource.lock");
    test.replace_source("http://gems.example.com", &test.server_url());

    let tarball_content =
        fs_err::read("../rv-gem-package/tests/fixtures/test-gem-1.0.0.gem").unwrap();
    let mock = test
        .mock_gem_download("test-gem-1.0.0.gem", &tarball_content)
        .create();

    let output = test.ci(&["--verbose"]);

    output.assert_success();
    releases_mock.assert();
    mock.assert();
}

#[test]
fn test_clean_install_respects_ruby() {
    let mut test = RvTest::new();

    let project_dir = test.temp_root().join("project");
    std::fs::create_dir_all(project_dir.as_path()).unwrap();
    std::fs::write(project_dir.join(".ruby-version"), b"3.4.8").unwrap();
    test.cwd = project_dir;

    test.use_gemfile("../rv-lockfile/tests/inputs/Gemfile.empty");
    test.use_lockfile("../rv-lockfile/tests/inputs/Gemfile.empty.lock");
    test.replace_source("https://rubygems.org", &test.server_url());

    let output = test.ci(&["--verbose"]);
    output.assert_success();
    let stdout = output.normalized_stdout();
    assert!(
        stdout.contains("Installed Ruby version ruby-3.4.8 to /tmp/home/.local/share/rv/rubies"),
        "{}",
        stdout,
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn test_clean_install_native_macos_aarch64() {
    let mut test = RvTest::new();
    let mock = test.mock_releases(["4.0.0"].to_vec());

    test.use_gemfile("../rv-lockfile/tests/inputs/Gemfile.testwithnative");
    test.use_lockfile("../rv-lockfile/tests/inputs/Gemfile.testwithnative.lock");

    let output = test.ci(&["--skip-compile-extensions"]);

    mock.assert();
    output.assert_success();

    // Store a snapshot of all the files `rv ci` created.
    let files_sorted = find_all_files_in_dir(test.cwd.as_ref());
    insta::assert_snapshot!(files_sorted);
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn test_clean_install_native_linux_x86_64() {
    let mut test = RvTest::new();
    let mock = test.mock_releases(["4.0.0"].to_vec());

    test.use_gemfile("../rv-lockfile/tests/inputs/Gemfile.testwithnative");
    test.use_lockfile("../rv-lockfile/tests/inputs/Gemfile.testwithnative.lock");

    let output = test.ci(&["--skip-compile-extensions"]);

    mock.assert();
    output.assert_success();

    // Store a snapshot of all the files `rv ci` created.
    let files_sorted = find_all_files_in_dir(test.cwd.as_ref());
    insta::assert_snapshot!(files_sorted);
}

#[test]
fn test_clean_install_download_faker() {
    let mut test = RvTest::new();
    let mock = test.mock_releases(["4.0.0"].to_vec());

    // https://github.com/faker-ruby/faker/blob/2f8b18b112fb3b7d2750321a8e574518cfac0d53/Gemfile
    test.use_gemfile("../rv-lockfile/tests/inputs/Gemfile.faker");
    // https://github.com/faker-ruby/faker/blob/2f8b18b112fb3b7d2750321a8e574518cfac0d53/Gemfile.lock
    test.use_lockfile("../rv-lockfile/tests/inputs/Gemfile.faker.lock");
    test.replace_source("http://gems.example.com", &test.server_url());

    let output = test.ci(&["--skip-compile-extensions"]);

    mock.assert();
    output.assert_success();

    // Store a snapshot of all the files `rv ci` created.
    let files_sorted = find_all_files_in_dir(test.cwd.as_ref());
    insta::assert_snapshot!(files_sorted);
}

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
