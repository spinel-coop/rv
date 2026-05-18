use crate::common::{RvOutput, RvTest};

impl RvTest {
    pub fn vendor(&mut self, args: &[&str]) -> RvOutput {
        self.rv(&[&["vendor"], args].concat())
    }
}

#[test]
fn test_vendor_downloads_gems_with_bundler_filenames() {
    let mut test = RvTest::new();

    test.use_gemfile("../rv-lockfile/tests/inputs/Gemfile.testsource");
    test.use_lockfile("../rv-lockfile/tests/inputs/Gemfile.testsource.lock");
    test.replace_source("http://gems.example.com", &test.server_url());

    let mock = test.mock_gem_download("test-gem-1.0.0.gem").create();

    let output = test.vendor(&[]);

    mock.assert();
    output.assert_success();
    output.assert_stdout_contains("Vendored");

    let vendored = test.current_dir().join("vendor/cache/test-gem-1.0.0.gem");
    assert!(
        vendored.exists(),
        "Expected gem at {} after vendor",
        vendored,
    );
}

#[test]
fn test_vendor_is_idempotent() {
    let mut test = RvTest::new();

    test.use_gemfile("../rv-lockfile/tests/inputs/Gemfile.testsource");
    test.use_lockfile("../rv-lockfile/tests/inputs/Gemfile.testsource.lock");
    test.replace_source("http://gems.example.com", &test.server_url());

    // First run downloads exactly once.
    let mock = test.mock_gem_download("test-gem-1.0.0.gem").expect(1).create();

    test.vendor(&[]).assert_success();

    // Second run must not hit the network.
    let second = test.vendor(&[]);
    second.assert_success();
    second.assert_stdout_contains("1 already present");

    mock.assert();
}

#[test]
fn test_vendor_offline_is_rejected() {
    let test = RvTest::new();

    test.use_gemfile("../rv-lockfile/tests/inputs/Gemfile.testsource");
    test.use_lockfile("../rv-lockfile/tests/inputs/Gemfile.testsource.lock");
    test.replace_source("http://gems.example.com", &test.server_url());

    let output = test.rv(&["--offline", "vendor"]);
    output.assert_failure();
    output.assert_stderr_contains("OfflineNotSupported");
}

#[test]
fn test_vendor_honors_bundle_cache_path() {
    let mut test = RvTest::new();

    test.use_gemfile("../rv-lockfile/tests/inputs/Gemfile.testsource");
    test.use_lockfile("../rv-lockfile/tests/inputs/Gemfile.testsource.lock");
    test.replace_source("http://gems.example.com", &test.server_url());

    let bundle_dir = test.current_dir().join(".bundle");
    std::fs::create_dir_all(&bundle_dir).unwrap();
    std::fs::write(
        bundle_dir.join("config"),
        "---\nBUNDLE_CACHE_PATH: my/cache\n",
    )
    .unwrap();

    let mock = test.mock_gem_download("test-gem-1.0.0.gem").create();
    test.vendor(&[]).assert_success();
    mock.assert();

    let vendored = test.current_dir().join("my/cache/test-gem-1.0.0.gem");
    assert!(vendored.exists(), "Expected gem at custom cache path");
    assert!(
        !test
            .current_dir()
            .join("vendor/cache/test-gem-1.0.0.gem")
            .exists(),
        "Default vendor/cache should be untouched when BUNDLE_CACHE_PATH is set",
    );
}
