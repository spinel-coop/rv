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
    output.assert_stdout_contains(
        "Installed Ruby version ruby-3.4.5 to /tmp/home/.local/share/rv/rubies",
    );

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

    output.assert_stdout_contains(
        "Installed Ruby version ruby-4.0.0 to /tmp/home/.local/share/rv/rubies",
    );

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
        .mock_tarball_download(download_path, &tarball_content)
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
    let output = test.rv(&["ruby", "run"]);
    output.assert_stdout_contains("ruby\n3.4.5");
}

#[test]
fn test_ruby_install_http_failure_no_empty_file() {
    let mut test = RvTest::new();

    let download_path = test.ruby_tarball_download_path("3.4.5");
    let ruby_mock = test
        .server
        .mock("GET", download_path.as_str())
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
        .server
        .mock("GET", download_path.as_str())
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
    let output2 = test.rv(&["ruby", "install", "3.4.5"]);
    output2.assert_success();

    output2.assert_stdout_contains("already exists, skipping download");

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
        .mock_tarball_download(download_path, &tarball_content)
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
        .server
        .mock("GET", download_path.as_str())
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
