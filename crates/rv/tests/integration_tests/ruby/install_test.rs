use crate::common::RvTest;
use std::fs;

#[test]
fn test_ruby_install_no_specific_version() {
    let mut test = RvTest::new();

    let ruby_mock = test.mock_ruby_download("3.4.5").create();

    let cache_dir = test.enable_cache();

    let mock = test.mock_releases(["3.4.5"].to_vec());

    let output = test.rv(&["ruby", "install"]);

    ruby_mock.assert();
    mock.assert();
    output.assert_success();
    output
        .assert_stdout_contains("Installed Ruby version 3.4.5 to /tmp/home/.local/share/rv/rubies");

    let cache_key = rv_cache::cache_digest(test.ruby_tarball_url("3.4.5"));
    let tarball_path = cache_dir
        .join("ruby-v0")
        .join("tarballs")
        .join(format!("{}.tar.gz", cache_key));
    assert!(tarball_path.exists(), "Tarball should be cached");
}

#[test]
fn test_ruby_install_incomplete_request() {
    let mut test = RvTest::new();

    let ruby_mock = test.mock_ruby_download("4.0.0").create();

    let cache_dir = test.enable_cache();

    let mock = test.mock_releases(["4.0.0"].to_vec());

    let output = test.rv(&["ruby", "install", "4"]);

    ruby_mock.assert();
    mock.assert();
    output.assert_success();

    output
        .assert_stdout_contains("Installed Ruby version 4.0.0 to /tmp/home/.local/share/rv/rubies");

    let cache_key = rv_cache::cache_digest(test.ruby_tarball_url("4.0.0"));
    let tarball_path = cache_dir
        .join("ruby-v0")
        .join("tarballs")
        .join(format!("{}.tar.gz", cache_key));
    assert!(tarball_path.exists(), "Tarball should be cached");
}

#[test]
fn test_ruby_install_successful_download() {
    let mut test = RvTest::new();

    let tarball_content = test.create_mock_tarball("3.4.5");
    let download_path = test.ruby_tarball_download_path("3.4.5");
    let ruby_mock = test
        .mock_tarball_download(&download_path, &tarball_content)
        .create();

    let cache_dir = test.enable_cache();

    let output = test.rv(&["ruby", "install", "3.4.5"]);

    ruby_mock.assert();
    output.assert_success();

    let cache_key = rv_cache::cache_digest(test.ruby_tarball_url("3.4.5"));
    let tarball_path = cache_dir
        .join("ruby-v0")
        .join("tarballs")
        .join(format!("{}.tar.gz", cache_key));
    assert!(tarball_path.exists(), "Tarball should be cached");

    let temp_path = cache_dir
        .join("ruby-v0")
        .join("tarballs")
        .join(format!("{}.tar.gz.tmp", cache_key));
    assert!(
        !temp_path.exists(),
        "Temp file should not exist after successful download"
    );

    let cached_content = fs::read(&tarball_path).expect("Should be able to read cached tarball");
    assert_eq!(
        cached_content, tarball_content,
        "Cached content should match downloaded content"
    );
}

// The mock tarball contains a bash script (bin/ruby) that can't execute on Windows.
#[test]
fn test_ruby_install_from_tarball() {
    let mut test = RvTest::new();

    let tarball_file = test.mock_tarball_on_disk("3.4.5");

    let tarball_path = tarball_file.as_str();
    let output = test.rv(&["ruby", "install", "--tarball-path", tarball_path, "3.4.5"]);

    output.assert_success();

    // Check mocked ruby from the tarball was actually installed by running it
    let output = test.rv(&["run", "ruby"]);
    output.assert_stdout_contains("ruby\n3.4.5");
}

#[test]
fn test_ruby_install_http_failure_no_empty_file() {
    let mut test = RvTest::new();

    let download_path = test.ruby_tarball_download_path("3.4.5");
    let ruby_mock = test
        .mock_request("GET", download_path.as_str())
        .with_status(404)
        .create();

    let cache_dir = test.enable_cache();

    let output = test.rv(&["ruby", "install", "3.4.5"]);

    ruby_mock.assert();
    output.assert_failure();

    let cache_key = rv_cache::cache_digest(test.ruby_tarball_url("3.4.5"));
    let tarball_path = cache_dir
        .join("ruby-v0")
        .join("tarballs")
        .join(format!("{}.tar.gz", cache_key));
    let temp_path = cache_dir
        .join("ruby-v0")
        .join("tarballs")
        .join(format!("{}.tar.gz.tmp", cache_key));

    assert!(
        !tarball_path.exists(),
        "No tarball should be created on HTTP failure"
    );
    assert!(
        !temp_path.exists(),
        "No temp file should remain on HTTP failure"
    );
}

#[test]
fn test_ruby_install_interrupted_download_cleanup() {
    let mut test = RvTest::new();

    let download_path = test.ruby_tarball_download_path("3.4.5");
    let ruby_mock = test
        .mock_request("GET", download_path.as_str())
        .with_status(200)
        .with_header("content-type", "application/gzip")
        .with_body("partial")
        .create();

    let cache_dir = test.enable_cache();

    let output = test.rv(&["ruby", "install", "3.4.5"]);

    ruby_mock.assert();
    output.assert_failure();

    let cache_key = rv_cache::cache_digest(test.ruby_tarball_url("3.4.5"));
    let tarball_path = cache_dir
        .join("ruby-v0")
        .join("tarballs")
        .join(format!("{}.tar.gz", cache_key));
    let temp_path = cache_dir
        .join("ruby-v0")
        .join("tarballs")
        .join(format!("{}.tar.gz.tmp", cache_key));

    assert!(
        tarball_path.exists(),
        "Tarball should exist at {} after successful download",
        tarball_path,
    );
    assert!(
        !temp_path.exists(),
        "No temp file should remain at {} after failure",
        temp_path,
    );
}

#[test]
fn test_ruby_install_cached_file_reused() {
    let mut test = RvTest::new();

    let mock = test.mock_ruby_download("3.4.5").expect(1).create();

    let _cache_dir = test.enable_cache();

    // This one should actually download tarballs, from the mocked server.
    let output1 = test.rv(&["ruby", "install", "3.4.5"]);
    output1.assert_success();

    // This one should just reuse the cached tarball without downloading.
    let output2 = test.rv(&["ruby", "install", "3.4.5", "--force"]);
    output2.assert_success();

    output2.assert_stdout_contains("already exists, skipping download");

    mock.assert();
}

#[test]
fn test_ruby_install_skips_existing_version_and_suggests_force_flag() {
    let mut test = RvTest::new();

    let mock = test.mock_ruby_download("3.4.5").create();

    let _cache_dir = test.enable_cache();

    // First installation – should succeed
    let output1 = test.rv(&["ruby", "install", "3.4.5"]);
    output1.assert_success();

    // Second installation – should report that it’s already present
    let output2 = test.rv(&["ruby", "install", "3.4.5"]);
    output2.assert_success();

    output2.assert_stdout_contains("If you want to overwrite it, use '--force'.");

    mock.assert();
}

#[test]
fn test_ruby_install_invalid_url() {
    let mut test = RvTest::new();

    test.env.insert(
        "RV_INSTALL_URL".into(),
        "http://invalid-url-that-does-not-exist.com".into(),
    );

    let cache_dir = test.enable_cache();

    let output = test.rv(&["ruby", "install", "3.4.5"]);

    output.assert_failure();

    let tarballs_dir = cache_dir.join("ruby-v0").join("tarballs");
    if tarballs_dir.exists() {
        let entries: Vec<_> = fs::read_dir(&tarballs_dir).unwrap().collect();
        assert!(
            entries.is_empty(),
            "No files should be created in tarballs directory"
        );
    }
}

#[test]
fn test_ruby_install_atomic_rename_behavior() {
    let mut test = RvTest::new();

    let tarball_content = test.create_mock_tarball("3.4.5");
    let download_path = test.ruby_tarball_download_path("3.4.5");
    let ruby_mock = test
        .mock_tarball_download(&download_path, &tarball_content)
        .create();

    let cache_dir = test.enable_cache();

    let output = test.rv(&["ruby", "install", "3.4.5"]);
    ruby_mock.assert();
    output.assert_success();

    let cache_key = rv_cache::cache_digest(test.ruby_tarball_url("3.4.5"));
    let tarball_path = cache_dir
        .join("ruby-v0")
        .join("tarballs")
        .join(format!("{}.tar.gz", cache_key));

    assert!(tarball_path.exists(), "Final tarball should exist");

    let metadata = fs::metadata(&tarball_path).expect("Should be able to get file metadata");
    assert!(metadata.len() > 0, "Tarball should not be empty");

    let content = fs::read(&tarball_path).expect("Should be able to read tarball");
    assert_eq!(content, tarball_content, "Content should match exactly");
}

#[test]
fn test_ruby_install_temp_file_cleanup_on_extraction_failure() {
    let mut test = RvTest::new();

    let download_path = test.ruby_tarball_download_path("3.4.5");
    let ruby_mock = test
        .mock_request("GET", download_path.as_str())
        .with_status(200)
        .with_header("content-type", "application/gzip")
        .with_body("invalid-tarball-content")
        .create();

    let cache_dir = test.enable_cache();

    let output = test.rv(&["ruby", "install", "3.4.5"]);

    ruby_mock.assert();
    output.assert_failure();

    let cache_key = rv_cache::cache_digest(test.ruby_tarball_url("3.4.5"));
    let temp_path = cache_dir
        .join("ruby-v0")
        .join("tarballs")
        .join(format!("{}.tar.gz.tmp", cache_key));

    assert!(!temp_path.exists(), "Temp file should be cleaned up");
}

#[test]
fn test_ruby_install_with_latest() {
    let mut test = RvTest::new();

    let ruby_mock = test.mock_ruby_download("4.0.1").create();

    let cache_dir = test.enable_cache();

    let mock = test.mock_releases(["3.4.5", "4.0.1"].to_vec());

    let output = test.rv(&["ruby", "install", "latest"]);

    ruby_mock.assert();
    mock.assert();
    output.assert_success();
    output
        .assert_stdout_contains("Installed Ruby version 4.0.1 to /tmp/home/.local/share/rv/rubies");

    let cache_key = rv_cache::cache_digest(test.ruby_tarball_url("4.0.1"));
    let tarball_path = cache_dir
        .join("ruby-v0")
        .join("tarballs")
        .join(format!("{}.tar.gz", cache_key));
    assert!(tarball_path.exists(), "Tarball should be cached");
}

#[test]
fn test_ruby_install_with_dev() {
    let mut test = RvTest::new();

    let (redirect_mock, download_mock) = test.mock_ruby_dev_download();

    let cache_dir = test.enable_cache();

    let output = test.rv(&["ruby", "install", "dev"]);

    redirect_mock.assert();
    download_mock.assert();
    output.assert_success();
    output.assert_stdout_contains("Installed ruby-dev to /tmp/home/.local/share/rv/rubies");

    let cache_key = rv_cache::cache_digest(test.ruby_dev_tarball_redirect_url());
    let tarball_path = cache_dir
        .join("ruby-v0")
        .join("tarballs")
        .join(format!("{}.tar.gz", cache_key));
    assert!(tarball_path.exists(), "Tarball should be cached");
}

#[test]
fn test_ruby_install_offline_with_no_cache_fails() {
    let mut test = RvTest::new();

    let _cache_dir = test.enable_cache();

    let output = test.rv(&["--offline", "ruby", "install", "3.4.5"]);

    output.assert_failure();
    output.assert_stderr_contains("OfflineArchiveMissing");
}

#[test]
fn test_ruby_install_offline_underspecified_version_fails_without_cache() {
    let mut test = RvTest::new();

    let _cache_dir = test.enable_cache();

    let output = test.rv(&["--offline", "ruby", "install", "3.4"]);

    output.assert_failure();
    output.assert_stderr_contains("OfflineRemoteRubyListUnavailable");
}

#[test]
fn test_ruby_install_offline_uses_cached_archive() {
    let mut test = RvTest::new();

    let cache_dir = test.enable_cache();
    let tarball_content = test.create_mock_tarball("3.4.5");
    let download_path = test.ruby_tarball_download_path("3.4.5");
    let cache_key = rv_cache::cache_digest(test.ruby_tarball_url("3.4.5"));
    let tarballs_dir = cache_dir.join("ruby-v0").join("tarballs");
    fs::create_dir_all(&tarballs_dir).unwrap();
    let tarball_path = tarballs_dir.join(format!("{}.tar.gz", cache_key));
    fs::write(&tarball_path, &tarball_content).unwrap();

    let no_fetch_guard = test
        .mock_request("GET", download_path.as_str())
        .with_status(200)
        .expect(0)
        .create();

    let output = test.rv(&["--offline", "ruby", "install", "3.4.5"]);

    output.assert_success();
    output
        .assert_stdout_contains("Installed Ruby version 3.4.5 to /tmp/home/.local/share/rv/rubies");
    no_fetch_guard.assert();
}

// The offline short-circuit must be exercised by a request that the
// non-offline guards (line ~100 `is_requested_ruby_installed_in_dir`) do not
// catch. We use a fuzzy request `3.4` while `ruby-3.4.5` is installed:
//
//   * with --offline: short-circuits on `current_ruby()` match, success.
//   * without --offline: falls through to `find_matching_remote_ruby`, which
//     attempts a release-list fetch, hits the mock server with no matching
//     mock, and fails.
//
// If the offline short-circuit ever regresses to ignoring `config.offline`,
// the without-offline case starts succeeding and the test catches it.
#[test]
fn test_ruby_install_offline_uses_already_installed_for_fuzzy_request() {
    let mut test = RvTest::new();
    let _cache_dir = test.enable_cache();
    test.create_ruby_dir("ruby-3.4.5");

    let output = test.rv(&["--offline", "ruby", "install", "3.4"]);
    output.assert_success();
    output.assert_stdout_contains("already installed");
    output.assert_stdout_contains("ruby-3.4.5");
}

#[test]
fn test_ruby_install_fuzzy_request_without_offline_does_not_short_circuit() {
    let mut test = RvTest::new();
    let _cache_dir = test.enable_cache();
    test.create_ruby_dir("ruby-3.4.5");

    // No --offline, no mocks: the fuzzy resolution must reach the network
    // (and fail). If the offline short-circuit fires unconditionally, this
    // would incorrectly succeed.
    let output = test.rv(&["ruby", "install", "3.4"]);
    output.assert_failure();
}
