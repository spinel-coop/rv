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

#[test]
fn test_ruby_install_from_tarball() {
    let mut test = RvTest::new();

    let tarball_content = test.create_mock_tarball("3.4.5");
    let tarball_file = test.mock_tarball_on_disk("3.4.5", tarball_content);

    let tarball_path = tarball_file.as_str();
    let output = test.rv(&["ruby", "install", "--tarball-path", tarball_path, "3.4.5"]);

    output.assert_success();

    // Check mocked ruby from the tarball was actually installed by running it
    let output = test.rv(&["run", "ruby"]);
    output.assert_stdout_contains("ruby\n3.4.5");
}

#[test]
fn test_ruby_install_from_tarball_with_files_falling_outside_root() {
    let test = RvTest::new();

    let tarball_path = test.temp_root().join("evil.tar.gz");
    let tarball_content = fs_err::read("tests/fixtures/evil.tar.gz").unwrap();
    fs_err::write(&tarball_path, &tarball_content).unwrap();

    let ruby_dir = test.rubies_dir().join("ruby-3.4.5");
    std::fs::create_dir_all(&ruby_dir).expect("Failed to create ruby directory");

    let output = test.rv(&[
        "ruby",
        "install",
        "--tarball-path",
        tarball_path.as_str(),
        "3.4.5",
    ]);

    output.assert_failure();
    output.assert_stderr_contains("DirectoryTraversalError");

    let owned_path = test.rubies_dir().join("owned");
    assert!(
        !owned_path.exists(),
        "No malicious file should be created, but found {owned_path}",
    );
}

#[test]
fn test_ruby_install_from_zip_with_files_falling_outside_root() {
    let test = RvTest::new();

    let tarball_path = test.temp_root().join("evil.zip");
    let tarball_content = fs_err::read("tests/fixtures/evil.zip").unwrap();
    fs_err::write(&tarball_path, &tarball_content).unwrap();

    let ruby_dir = test.rubies_dir().join("ruby-3.4.5");
    std::fs::create_dir_all(&ruby_dir).expect("Failed to create ruby directory");

    let output = test.rv(&[
        "ruby",
        "install",
        "--tarball-path",
        tarball_path.as_str(),
        "3.4.5",
    ]);

    output.assert_failure();
    output.assert_stderr_contains("DirectoryTraversalError");

    let owned_path = test.rubies_dir().join("owned");
    assert!(
        !owned_path.exists(),
        "No malicious file should be created, but found {owned_path}",
    );
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

    test.env
        .insert("RV_INSTALL_URL".into(), "http://127.0.0.1:0".into());

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
fn test_jruby_install() {
    let mut test = RvTest::new();

    let jruby_mock = test.mock_jruby_download("10.1.1.0").create();
    let cache_dir = test.enable_cache();
    let mock = test.mock_jruby_releases(["10.1.1.0"].to_vec());

    let output = test.rv(&["ruby", "install", "jruby-10.1.1.0"]);

    jruby_mock.assert();
    output.assert_success();
    output.assert_stdout_contains(
        "Installed jruby version 10.1.1.0 to /tmp/home/.local/share/rv/rubies",
    );

    // The directory has to be engine-qualified, or rv won't find it again.
    let install_dir = test.rubies_dir().join("jruby-10.1.1.0");
    let ruby_bin = install_dir.join("bin").join(test.ruby_executable_name());
    assert!(ruby_bin.exists(), "{ruby_bin} should exist");

    // Needs the long-name record applied, or it lands truncated to 100 bytes.
    let long_path = install_dir
        .join("lib")
        .join("ruby")
        .join("stdlib")
        .join("did_you_mean")
        .join("spell_checkers")
        .join("name_error_checkers")
        .join("variable_name_checker.rb");
    assert!(
        long_path.exists(),
        "long path should be restored in full, got: {:?}",
        fs::read_dir(
            install_dir.join("lib/ruby/stdlib/did_you_mean/spell_checkers/name_error_checkers")
        )
        .map(|d| d
            .filter_map(|e| e.ok().map(|e| e.file_name()))
            .collect::<Vec<_>>())
    );

    let cache_key = rv_cache::cache_digest(test.jruby_tarball_url("10.1.1.0"));
    let tarball_path = cache_dir
        .join("ruby-v0")
        .join("tarballs")
        .join(format!("{}.tar.gz", cache_key));
    assert!(tarball_path.exists(), "Tarball should be cached");

    drop(mock);
}

#[test]
fn test_jruby_install_resolves_incomplete_request() {
    let mut test = RvTest::new();

    let jruby_mock = test.mock_jruby_download("9.4.15.0").create();
    test.enable_cache();
    let mock = test.mock_jruby_releases(["9.4.12.1", "9.4.15.0", "10.1.1.0"].to_vec());

    // Must pick the newest 9.4.x, not the newest JRuby overall.
    let output = test.rv(&["ruby", "install", "jruby-9.4"]);

    jruby_mock.assert();
    mock.assert();
    output.assert_success();
    output.assert_stdout_contains("Installed jruby version 9.4.15.0");

    assert!(
        test.rubies_dir()
            .join("jruby-9.4.15.0")
            .join("bin")
            .join(test.ruby_executable_name())
            .exists()
    );
}

#[test]
fn test_ruby_install_is_unaffected_by_jruby_support() {
    let mut test = RvTest::new();

    let ruby_mock = test.mock_ruby_download("3.4.5").create();
    test.enable_cache();
    // Fully specified: resolves without the releases list, so this mock isn't asserted.
    let _mock = test.mock_releases(["3.4.5"].to_vec());

    let output = test.rv(&["ruby", "install", "3.4.5"]);

    ruby_mock.assert();
    output.assert_success();
    output.assert_stdout_contains("Installed Ruby version 3.4.5");

    assert!(test.rubies_dir().join("ruby-3.4.5").exists());
}
