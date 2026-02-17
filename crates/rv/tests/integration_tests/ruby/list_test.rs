use crate::common::{RvOutput, RvTest};
use insta::assert_snapshot;
use rv_platform::HostPlatform;

impl RvTest {
    pub fn ruby_list(&self, args: &[&str]) -> RvOutput {
        self.rv(&[&["ruby", "list"], args].concat())
    }
}

#[test]
fn test_ruby_list_text_output_empty() {
    let test = RvTest::new();
    let output = test.ruby_list(&["--format", "json"]);

    output.assert_success();
    assert_snapshot!(output.normalized_stdout());
}

#[test]
fn test_ruby_list_json_output_empty() {
    let test = RvTest::new();
    let output = test.ruby_list(&["--format", "json"]);

    output.assert_success();
    assert_snapshot!(output.normalized_stdout());
}

#[test]
fn test_ruby_list_text_output_with_rubies() {
    let test = RvTest::new();

    // Create some mock Ruby installations
    test.create_ruby_dir("ruby-3.1.4");
    test.create_ruby_dir("ruby-3.2.0");

    let output = test.ruby_list(&["--no-color", "--format", "json"]);

    output.assert_success();
    assert_snapshot!(output.normalized_stdout());
}

#[test]
fn test_ruby_list_json_output_with_rubies() {
    let test = RvTest::new();

    // Create some mock Ruby installations
    test.create_ruby_dir("ruby-3.1.4");
    test.create_ruby_dir("ruby-3.2.0");

    let output = test.ruby_list(&["--format", "json"]);

    output.assert_success();

    // Verify it's valid JSON
    let stdout = output.stdout();
    let _: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|_| panic!("Output should be valid JSON, was: {stdout}"));

    assert_snapshot!(output.normalized_stdout());
}

#[test]
fn test_ruby_list_multiple_matching_rubies() {
    let mut test = RvTest::new();

    let project_dir = test.temp_root().join("project");
    std::fs::create_dir_all(project_dir.as_path()).unwrap();
    std::fs::write(project_dir.join(".ruby-version"), b"3").unwrap();
    test.cwd = project_dir;

    // Create some mock Ruby installations
    test.create_ruby_dir("ruby-3.1.4");
    test.create_ruby_dir("ruby-3.2.0");
    test.create_ruby_dir("3.1.4");

    let output = test.ruby_list(&["--no-color", "--format", "json"]);
    output.assert_success();
    assert_snapshot!(output.normalized_stdout(), @r#"
    [
      {
        "key": "ruby-3.1.4-macos-aarch64",
        "version": "ruby-3.1.4",
        "path": "/tmp/home/.local/share/rv/rubies/3.1.4",
        "managed": true,
        "arch": "aarch64",
        "os": "macos",
        "gem_root": null,
        "installed": true,
        "active": false
      },
      {
        "key": "ruby-3.1.4-macos-aarch64",
        "version": "ruby-3.1.4",
        "path": "/tmp/home/.local/share/rv/rubies/ruby-3.1.4",
        "managed": true,
        "arch": "aarch64",
        "os": "macos",
        "gem_root": null,
        "installed": true,
        "active": false
      },
      {
        "key": "ruby-3.2.0-macos-aarch64",
        "version": "ruby-3.2.0",
        "path": "/tmp/home/.local/share/rv/rubies/ruby-3.2.0",
        "managed": true,
        "arch": "aarch64",
        "os": "macos",
        "gem_root": null,
        "installed": true,
        "active": true
      }
    ]
    "#);

    test.create_ruby_dir("3.2.0");
    let output = test.ruby_list(&["--no-color", "--format", "json"]);
    output.assert_success();
    assert_snapshot!(output.normalized_stdout(), @r#"
    [
      {
        "key": "ruby-3.1.4-macos-aarch64",
        "version": "ruby-3.1.4",
        "path": "/tmp/home/.local/share/rv/rubies/3.1.4",
        "managed": true,
        "arch": "aarch64",
        "os": "macos",
        "gem_root": null,
        "installed": true,
        "active": false
      },
      {
        "key": "ruby-3.1.4-macos-aarch64",
        "version": "ruby-3.1.4",
        "path": "/tmp/home/.local/share/rv/rubies/ruby-3.1.4",
        "managed": true,
        "arch": "aarch64",
        "os": "macos",
        "gem_root": null,
        "installed": true,
        "active": false
      },
      {
        "key": "ruby-3.2.0-macos-aarch64",
        "version": "ruby-3.2.0",
        "path": "/tmp/home/.local/share/rv/rubies/3.2.0",
        "managed": true,
        "arch": "aarch64",
        "os": "macos",
        "gem_root": null,
        "installed": true,
        "active": false
      },
      {
        "key": "ruby-3.2.0-macos-aarch64",
        "version": "ruby-3.2.0",
        "path": "/tmp/home/.local/share/rv/rubies/ruby-3.2.0",
        "managed": true,
        "arch": "aarch64",
        "os": "macos",
        "gem_root": null,
        "installed": true,
        "active": true
      }
    ]
    "#);

    test.env.insert(
        "PATH".into(),
        "/tmp/home/.local/share/rv/rubies/3.1.4/bin".into(),
    );

    let output = test.ruby_list(&["--no-color", "--format", "json"]);
    output.assert_success();
    assert_snapshot!(output.normalized_stdout(), @r#"
    [
      {
        "key": "ruby-3.1.4-macos-aarch64",
        "version": "ruby-3.1.4",
        "path": "/tmp/home/.local/share/rv/rubies/3.1.4",
        "managed": true,
        "arch": "aarch64",
        "os": "macos",
        "gem_root": null,
        "installed": true,
        "active": false
      },
      {
        "key": "ruby-3.1.4-macos-aarch64",
        "version": "ruby-3.1.4",
        "path": "/tmp/home/.local/share/rv/rubies/ruby-3.1.4",
        "managed": true,
        "arch": "aarch64",
        "os": "macos",
        "gem_root": null,
        "installed": true,
        "active": false
      },
      {
        "key": "ruby-3.2.0-macos-aarch64",
        "version": "ruby-3.2.0",
        "path": "/tmp/home/.local/share/rv/rubies/3.2.0",
        "managed": true,
        "arch": "aarch64",
        "os": "macos",
        "gem_root": null,
        "installed": true,
        "active": false
      },
      {
        "key": "ruby-3.2.0-macos-aarch64",
        "version": "ruby-3.2.0",
        "path": "/tmp/home/.local/share/rv/rubies/ruby-3.2.0",
        "managed": true,
        "arch": "aarch64",
        "os": "macos",
        "gem_root": null,
        "installed": true,
        "active": true
      }
    ]
    "#);
}

#[test]
fn test_ruby_list_with_available_and_installed_merges_both_lists() {
    let mut test = RvTest::new();
    test.create_ruby_dir("ruby-3.1.4");

    let mock = test.mock_releases(["3.4.5"].to_vec());
    let output = test.ruby_list(&["--format", "json"]);

    mock.assert();
    output.assert_success();

    // 3.1.4 and 3.4.5 should be listed, with 3.1.4 marked as installed
    insta::assert_snapshot!(output.normalized_stdout());
}

#[test]
fn test_ruby_list_with_available_and_installed_with_same_minor_lists_all_versions() {
    let mut test = RvTest::new();
    test.create_ruby_dir("ruby-3.4.0");

    let mock = test.mock_releases(["3.4.1"].to_vec());
    let output = test.ruby_list(&["--format", "json"]);

    mock.assert();
    output.assert_success();

    // 3.4.0 and 3.4.1 should be listed, with 3.4.0 marked as installed
    insta::assert_snapshot!(output.normalized_stdout());
}

#[test]
fn test_ruby_list_with_available_and_installed_remotes_keep_only_latest_patch() {
    let mut test = RvTest::new();
    test.create_ruby_dir("ruby-3.3.1");

    let mock = test.mock_releases(["3.4.0", "3.4.1"].to_vec());
    let output = test.ruby_list(&["--format", "json"]);

    mock.assert();
    output.assert_success();

    // Only 3.3.1 and 3.4.1 should be listed, with 3.3.1 marked as installed
    insta::assert_snapshot!(output.normalized_stdout());
}

#[test]
fn test_ruby_list_with_no_installed_rubies_is_empty() {
    let test = RvTest::new();
    let output = test.ruby_list(&["--format", "json"]);
    output.assert_success();

    // The output will be completely empty because no rubies are installed
    // and the API is disabled.
    assert_eq!(output.normalized_stdout(), "[]");
}

#[test]
fn test_ruby_list_no_local_installs_still_lists_remote_rubies() {
    let mut test = RvTest::new();

    let mock = test.mock_releases(["3.0.0", "4.0.0"].to_vec());
    let output = test.ruby_list(&["--format", "json"]);

    mock.assert();
    output.assert_success();

    insta::assert_snapshot!(output.normalized_stdout());
}

#[test]
fn test_ruby_list_ruby_3_5_is_skipped() {
    let mut test = RvTest::new();

    let mock = test.mock_releases(["3.5.0-preview1", "4.0.0"].to_vec());
    let output = test.ruby_list(&["--format", "json"]);

    mock.assert();
    output.assert_success();

    insta::assert_snapshot!(output.normalized_stdout());
}

#[test]
fn test_ruby_list_shows_requested_ruby_even_if_not_installed_and_not_a_latest_patch() {
    let mut test = RvTest::new();

    let project_dir = test.temp_root().join("project");
    std::fs::create_dir_all(project_dir.as_path()).unwrap();
    std::fs::write(project_dir.join(".ruby-version"), b"3.4.7").unwrap();
    test.cwd = project_dir;

    let mock = test.mock_releases(["3.4.7", "3.4.8"].to_vec());
    let output = test.ruby_list(&["--format", "json"]);

    mock.assert();
    output.assert_success();

    // Both 3.4.7 and 3.4.8 should be listed, with 3.4.7 marked as active
    insta::assert_snapshot!(output.normalized_stdout());
}

#[test]
fn test_ruby_list_without_updating_versions() {
    let mut test = RvTest::new();
    test.env.insert("RV_LIST_URL".into(), "-".into());
    let output = test.ruby_list(&["--format", "json"]);
    output.assert_success();
    assert_eq!(output.normalized_stdout(), "[]");
}

/// Verifies that Windows sees rubies from the RubyInstaller2 endpoint.
/// This is the test that would have caught the original bug: `rv ruby list`
/// on Windows returned "No rubies found" because it only queried rv-ruby
/// (which has zero Windows assets).
#[test]
fn test_ruby_list_windows_platform_finds_rubies() {
    let mut test = RvTest::new();
    test.set_platform(HostPlatform::WindowsX86_64);

    let mock = test.mock_windows_releases(["3.4.4", "3.3.7"].to_vec());
    let output = test.ruby_list(&["--format", "json"]);

    mock.assert();
    output.assert_success();

    let stdout = output.normalized_stdout();
    assert!(
        stdout.contains("ruby-3.4.4"),
        "Windows should see ruby-3.4.4, got: {stdout}",
    );
    assert!(
        stdout.contains("ruby-3.3.7"),
        "Windows should see ruby-3.3.7, got: {stdout}",
    );
}

/// Verifies that each non-Windows platform sees only its own rubies when
/// the release contains assets for all platforms. Windows uses a different
/// fetch path (RubyInstaller2) and is tested separately.
#[test]
fn test_ruby_list_all_platforms_find_rubies() {
    let non_windows: Vec<HostPlatform> = HostPlatform::all()
        .iter()
        .copied()
        .filter(|hp| !hp.is_windows())
        .collect();

    for platform in non_windows {
        let mut test = RvTest::new();
        test.set_platform(platform);

        let mock = test.mock_releases_all_platforms(["3.4.1"].to_vec());
        let output = test.ruby_list(&["--format", "json"]);

        mock.assert();
        output.assert_success();

        let stdout = output.normalized_stdout();
        assert!(
            stdout.contains("ruby-3.4.1"),
            "Platform {:?} should see ruby-3.4.1, got: {stdout}",
            platform,
        );
    }
}
