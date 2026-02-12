#[cfg(unix)]
use crate::common::{RvOutput, RvTest};

// ci() removes RV_TEST_PLATFORM so the subprocess detects native platform and
// downloads real Ruby binaries from GitHub. On Windows, this triggers the
// RubyInstaller2 flow which has a completely different download/install path.
// These tests are gated to Unix until dedicated Windows CI test setup exists.
#[cfg(unix)]
impl RvTest {
    pub fn ci(&mut self, args: &[&str]) -> RvOutput {
        self.env.remove("RV_INSTALL_URL");
        // Remove RV_TEST_PLATFORM so the subprocess uses its compile-time native
        // platform. This is necessary because ci tests download real Ruby binaries
        // from GitHub, and those binaries must match the host architecture.
        self.env.remove("RV_TEST_PLATFORM");
        self.rv(&[&["ci"], args].concat())
    }
}

#[cfg(unix)]
#[test]
fn test_clean_install_download_test_gem() {
    let mut test = RvTest::new();

    let releases_mock = test.mock_releases_all_platforms(["4.0.0"].to_vec());

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

#[cfg(unix)]
#[test]
fn test_clean_install_rakefile_extension() {
    let mut test = RvTest::new();

    let releases_mock = test.mock_releases_all_platforms(["4.0.0"].to_vec());

    test.use_gemfile("../rv-lockfile/tests/inputs/Gemfile.rakeext");
    test.use_lockfile("../rv-lockfile/tests/inputs/Gemfile.rakeext.lock");
    test.replace_source("http://gems.example.com", &test.server_url());

    let tarball_content =
        fs_err::read("../rv-gem-package/tests/fixtures/rake-ext-test-1.0.0.gem").unwrap();
    let mock = test
        .mock_gem_download("rake-ext-test-1.0.0.gem", &tarball_content)
        .create();

    let output = test.ci(&["--verbose"]);

    output.assert_success();
    releases_mock.assert();
    mock.assert();
}

#[cfg(unix)]
#[test]
fn test_clean_install_input_validation() {
    let mut test = RvTest::new();

    let releases_mock = test.mock_releases_all_platforms(["4.0.0"].to_vec());

    // Test missing a lockfile fails
    let output = test.ci(&[]);
    output.assert_failure();
    assert_eq!(
        output.normalized_stderr(),
        "Error: CiError(MissingImplicitLockfile)\n",
    );
    releases_mock.assert();

    // Test using only a lockfile works
    test.use_lockfile("../rv-lockfile/tests/inputs/Gemfile.empty.lock");
    let output = test.ci(&[]);
    output.assert_success();
    releases_mock.assert();

    // Let rv infer installation path from Gemfile argument. This test would install real gems to
    // real rv installation directory, so we use an empty Gemfile to avoid side effects.
    test.env.remove("BUNDLE_PATH");

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
    releases_mock.assert();

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

    // Let rv infer installation path from Gemfile argument. This test would install real gems to
    // real rv installation directory, so we use an empty Gemfile to avoid side effects.
    test.env.remove("BUNDLE_PATH");

    let output = test.ci(&["--gemfile", "project/Gemfile"]);
    output.assert_success();
    releases_mock.assert();

    // Test passing a missing gemfile gives a nice error
    let output = test.ci(&["--gemfile", "Gemfile.missing"]);
    output.assert_failure();
    assert_eq!(
        output.normalized_stderr(),
        "Error: CiError(MissingGemfile(\"Gemfile.missing\"))\n",
    );
    releases_mock.assert();

    // Test passing an invalid gemfile gives a nice error
    let output = test.ci(&["--gemfile", "/"]);
    output.assert_failure();
    assert_eq!(
        output.normalized_stderr(),
        "Error: CiError(InvalidGemfilePath(\"/\"))\n",
    );
    releases_mock.assert();
}

#[cfg(unix)]
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
    let mock = test.mock_releases_all_platforms(["4.0.0"].to_vec());

    test.use_gemfile("../rv-lockfile/tests/inputs/Gemfile.testwithnative");
    test.use_lockfile("../rv-lockfile/tests/inputs/Gemfile.testwithnative.lock");

    let output = test.ci(&[]);

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
    let mock = test.mock_releases_all_platforms(["4.0.0"].to_vec());

    test.use_gemfile("../rv-lockfile/tests/inputs/Gemfile.testwithnative");
    test.use_lockfile("../rv-lockfile/tests/inputs/Gemfile.testwithnative.lock");

    let output = test.ci(&[]);

    mock.assert();
    output.assert_success();

    // Store a snapshot of all the files `rv ci` created.
    let files_sorted = find_all_files_in_dir(test.cwd.as_ref());
    insta::assert_snapshot!(files_sorted);
}

#[cfg(unix)]
#[test]
fn test_clean_install_evaluates_local_gemspecs_in_the_right_cwd() {
    use indoc::formatdoc;

    let mut test = RvTest::new();
    let mock = test.mock_releases_all_platforms(["4.0.0"].to_vec());

    let project_dir = test.temp_root().join("project");
    std::fs::create_dir_all(project_dir.as_path()).unwrap();
    std::fs::create_dir_all(project_dir.join("foo").as_path()).unwrap();

    let foo_gemspec = formatdoc! {"
        require './version.rb'

        Gem::Specification.new do |s|
            s.name = 'foo'
            s.version = Foo::VERSION
            s.summary = 'The foo gem'
            s.author = 'Bandre Barco'
        end
    "};

    std::fs::write(project_dir.join("foo/foo.gemspec"), foo_gemspec).unwrap();

    let foo_version = formatdoc! {"
        module Foo
            VERSION = '1.0.0'
        end
    "};

    std::fs::write(project_dir.join("foo/version.rb"), foo_version).unwrap();

    test.cwd = project_dir;

    test.use_gemfile("../rv-lockfile/tests/inputs/Gemfile.relative-gemspec-paths");
    test.use_lockfile("../rv-lockfile/tests/inputs/Gemfile.relative-gemspec-paths.lock");

    let output = test.ci(&[]);

    mock.assert();
    output.assert_success();

    assert_eq!(output.normalized_stderr(), "");
}

#[cfg(unix)]
#[test]
fn test_clean_install_fails_if_evaluating_a_path_gemspec_fails() {
    use indoc::formatdoc;

    let mut test = RvTest::new();
    let mock = test.mock_releases_all_platforms(["4.0.0"].to_vec());

    let project_dir = test.temp_root().join("project");
    std::fs::create_dir_all(project_dir.as_path()).unwrap();
    std::fs::create_dir_all(project_dir.join("foo").as_path()).unwrap();

    let foo_gemspec = formatdoc! {"
        require './missing.rb'

        Gem::Specification.new do |s|
            s.name = 'foo'
            s.version = Foo::VERSION
            s.summary = 'The foo gem'
            s.author = 'Bandre Barco'
        end
    "};

    std::fs::write(project_dir.join("foo/foo.gemspec"), foo_gemspec).unwrap();

    test.cwd = project_dir;

    test.use_gemfile("../rv-lockfile/tests/inputs/Gemfile.relative-gemspec-paths");
    test.use_lockfile("../rv-lockfile/tests/inputs/Gemfile.relative-gemspec-paths.lock");

    let output = test.ci(&[]);

    mock.assert();
    output.assert_failure();

    assert!(
        output
            .normalized_stderr()
            .contains("cannot load such file -- ./missing.rb"),
        "should show an error about the file that failed to load"
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
