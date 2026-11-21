use crate::common::{MOCK_BUILD, RvOutput, RvTest};
use insta::assert_snapshot;
use rv_platform::HostPlatform;

impl RvTest {
    pub fn ruby_list(&self, args: &[&str]) -> RvOutput {
        self.rv(&[&["ruby", "list"], args].concat())
    }
}

#[test]
fn test_ruby_list_text_output_empty() {
    let mut test = RvTest::new();
    let mock = test.mock_releases([].to_vec());
    let output = test.ruby_list(&["--format", "json"]);

    mock.assert();
    output.assert_success();
    assert!(output.stderr().is_empty());
    assert_snapshot!(output.normalized_stdout());
}

#[test]
fn test_ruby_list_json_output_empty() {
    let mut test = RvTest::new();
    let mock = test.mock_releases([].to_vec());
    let output = test.ruby_list(&["--format", "json"]);

    mock.assert();
    output.assert_success();
    assert!(output.stderr().is_empty());
    assert_snapshot!(output.normalized_stdout());
}

#[test]
fn test_ruby_list_text_output_with_rubies() {
    let mut test = RvTest::new();
    let mock = test.mock_releases([].to_vec());

    // Create some mock Ruby installations
    test.create_ruby_dir("ruby-3.1.4");
    test.create_ruby_dir("ruby-3.2.0");

    let output = test.ruby_list(&["--no-color", "--format", "json"]);

    mock.assert();
    output.assert_success();
    assert!(output.stderr().is_empty());
    assert_snapshot!(output.normalized_stdout());
}

#[test]
fn test_ruby_list_json_output_with_rubies() {
    let mut test = RvTest::new();
    let mock = test.mock_releases([].to_vec());

    // Create some mock Ruby installations
    test.create_ruby_dir("ruby-3.1.4");
    test.create_ruby_dir("ruby-3.2.0");

    let output = test.ruby_list(&["--format", "json"]);

    mock.assert();
    output.assert_success();
    assert!(output.stderr().is_empty());

    // Verify it's valid JSON
    let stdout = output.stdout();
    let _: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|_| panic!("Output should be valid JSON, was: {stdout}"));

    assert_snapshot!(output.normalized_stdout());
}

#[test]
fn test_ruby_list_multiple_matching_rubies() {
    let mut test = RvTest::new();
    let mock = test.mock_releases([].to_vec());

    let project_dir = test.temp_root().join("project");
    std::fs::create_dir_all(project_dir.as_path()).unwrap();
    std::fs::write(project_dir.join(".ruby-version"), b"3").unwrap();
    test.cwd = project_dir;

    // Create some mock Ruby installations
    test.create_ruby_dir("ruby-3.1.4");
    test.create_ruby_dir("ruby-3.2.0");
    test.create_ruby_dir("3.1.4");

    let output = test.ruby_list(&["--no-color", "--format", "json"]);
    mock.assert();
    output.assert_success();
    assert!(output.stderr().is_empty());
    // editorconfig-checker-disable
    assert_snapshot!(output.normalized_stdout(), @r#"
    [
      {
        "Installed": {
          "key": "ruby-3.1.4-macos-aarch64",
          "version": "ruby-3.1.4",
          "path": "/tmp/home/.local/share/rv/rubies/3.1.4",
          "managed": true,
          "arch": "aarch64",
          "os": "macos",
          "gem_root": null,
          "enable_shared": false,
          "rubygems_platform": "aarch64-darwin23"
        },
        "active": false
      },
      {
        "Installed": {
          "key": "ruby-3.1.4-macos-aarch64",
          "version": "ruby-3.1.4",
          "path": "/tmp/home/.local/share/rv/rubies/ruby-3.1.4",
          "managed": true,
          "arch": "aarch64",
          "os": "macos",
          "gem_root": null,
          "enable_shared": false,
          "rubygems_platform": "aarch64-darwin23"
        },
        "active": false
      },
      {
        "Installed": {
          "key": "ruby-3.2.0-macos-aarch64",
          "version": "ruby-3.2.0",
          "path": "/tmp/home/.local/share/rv/rubies/ruby-3.2.0",
          "managed": true,
          "arch": "aarch64",
          "os": "macos",
          "gem_root": null,
          "enable_shared": false,
          "rubygems_platform": "aarch64-darwin23"
        },
        "active": true
      }
    ]
    "#);
    // editorconfig-checker-enable

    test.create_ruby_dir("3.2.0");
    let output = test.ruby_list(&["--no-color", "--format", "json"]);
    output.assert_success();
    // editorconfig-checker-disable
    assert_snapshot!(output.normalized_stdout(), @r#"
    [
      {
        "Installed": {
          "key": "ruby-3.1.4-macos-aarch64",
          "version": "ruby-3.1.4",
          "path": "/tmp/home/.local/share/rv/rubies/3.1.4",
          "managed": true,
          "arch": "aarch64",
          "os": "macos",
          "gem_root": null,
          "enable_shared": false,
          "rubygems_platform": "aarch64-darwin23"
        },
        "active": false
      },
      {
        "Installed": {
          "key": "ruby-3.1.4-macos-aarch64",
          "version": "ruby-3.1.4",
          "path": "/tmp/home/.local/share/rv/rubies/ruby-3.1.4",
          "managed": true,
          "arch": "aarch64",
          "os": "macos",
          "gem_root": null,
          "enable_shared": false,
          "rubygems_platform": "aarch64-darwin23"
        },
        "active": false
      },
      {
        "Installed": {
          "key": "ruby-3.2.0-macos-aarch64",
          "version": "ruby-3.2.0",
          "path": "/tmp/home/.local/share/rv/rubies/3.2.0",
          "managed": true,
          "arch": "aarch64",
          "os": "macos",
          "gem_root": null,
          "enable_shared": false,
          "rubygems_platform": "aarch64-darwin23"
        },
        "active": false
      },
      {
        "Installed": {
          "key": "ruby-3.2.0-macos-aarch64",
          "version": "ruby-3.2.0",
          "path": "/tmp/home/.local/share/rv/rubies/ruby-3.2.0",
          "managed": true,
          "arch": "aarch64",
          "os": "macos",
          "gem_root": null,
          "enable_shared": false,
          "rubygems_platform": "aarch64-darwin23"
        },
        "active": true
      }
    ]
    "#);
    // editorconfig-checker-enable

    test.env.insert(
        "PATH".into(),
        "/tmp/home/.local/share/rv/rubies/3.1.4/bin".into(),
    );

    let output = test.ruby_list(&["--no-color", "--format", "json"]);
    output.assert_success();
    // editorconfig-checker-disable
    assert_snapshot!(output.normalized_stdout(), @r#"
    [
      {
        "Installed": {
          "key": "ruby-3.1.4-macos-aarch64",
          "version": "ruby-3.1.4",
          "path": "/tmp/home/.local/share/rv/rubies/3.1.4",
          "managed": true,
          "arch": "aarch64",
          "os": "macos",
          "gem_root": null,
          "enable_shared": false,
          "rubygems_platform": "aarch64-darwin23"
        },
        "active": false
      },
      {
        "Installed": {
          "key": "ruby-3.1.4-macos-aarch64",
          "version": "ruby-3.1.4",
          "path": "/tmp/home/.local/share/rv/rubies/ruby-3.1.4",
          "managed": true,
          "arch": "aarch64",
          "os": "macos",
          "gem_root": null,
          "enable_shared": false,
          "rubygems_platform": "aarch64-darwin23"
        },
        "active": false
      },
      {
        "Installed": {
          "key": "ruby-3.2.0-macos-aarch64",
          "version": "ruby-3.2.0",
          "path": "/tmp/home/.local/share/rv/rubies/3.2.0",
          "managed": true,
          "arch": "aarch64",
          "os": "macos",
          "gem_root": null,
          "enable_shared": false,
          "rubygems_platform": "aarch64-darwin23"
        },
        "active": false
      },
      {
        "Installed": {
          "key": "ruby-3.2.0-macos-aarch64",
          "version": "ruby-3.2.0",
          "path": "/tmp/home/.local/share/rv/rubies/ruby-3.2.0",
          "managed": true,
          "arch": "aarch64",
          "os": "macos",
          "gem_root": null,
          "enable_shared": false,
          "rubygems_platform": "aarch64-darwin23"
        },
        "active": true
      }
    ]
    "#);
    // editorconfig-checker-enable
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
fn test_ruby_list_with_available_and_installed_sorts_properly() {
    let mut test = RvTest::new();
    test.create_ruby_dir("ruby-3.4.1");

    let mock = test.mock_releases(["3.4.10"].to_vec());
    let output = test.ruby_list(&["--format", "json"]);

    mock.assert();
    output.assert_success();

    // 3.4.1 and 3.4.10 should be listed, with 3.4.1 marked as installed and sorted first
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
    let mut test = RvTest::new();
    let mock = test.mock_releases([].to_vec());
    let output = test.ruby_list(&["--format", "json"]);
    mock.assert();
    output.assert_success();
    assert!(output.stderr().is_empty());

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

#[test]
fn test_ruby_list_shows_the_build_of_an_available_ruby() {
    let mut test = RvTest::new();

    let mock = test.mock_releases(["3.4.5"].to_vec());
    let output = test.ruby_list(&["--no-color"]);

    mock.assert();
    output.assert_success();
    output.assert_stdout_contains(&format!("3.4.5 (build {MOCK_BUILD})"));
}

#[test]
fn test_ruby_list_points_out_an_installed_ruby_with_a_newer_build() {
    let mut test = RvTest::new();
    let ruby_dir = test.create_ruby_dir("ruby-3.4.5");
    std::fs::write(
        ruby_dir.join(".rv-build"),
        r#"{"build":"20260101","url":"https://rv.dev/ruby/3.4.5/macos-aarch64","installed_at":"2026-01-02T03:04:05Z"}"#,
    )
    .unwrap();

    let mock = test.mock_releases(["3.4.5"].to_vec());
    let output = test.ruby_list(&["--no-color"]);

    mock.assert();
    output.assert_success();
    output.assert_stdout_contains(&format!("(build {MOCK_BUILD} available)"));

    let output = test.ruby_list(&["--format", "json"]);
    let rubies: serde_json::Value = serde_json::from_str(&output.stdout()).unwrap();

    assert_eq!(rubies[0]["build"], "20260101");
    assert_eq!(rubies[0]["newer_build"], MOCK_BUILD);
}

#[test]
fn test_ruby_list_says_nothing_about_an_installed_ruby_on_the_newest_build() {
    let mut test = RvTest::new();
    let ruby_dir = test.create_ruby_dir("ruby-3.4.5");
    std::fs::write(
        ruby_dir.join(".rv-build"),
        format!(
            r#"{{"build":"{MOCK_BUILD}","url":"https://rv.dev/ruby/3.4.5/macos-aarch64","installed_at":"2026-10-15T03:04:05Z"}}"#
        ),
    )
    .unwrap();

    let _mock = test.mock_releases(["3.4.5"].to_vec());
    let output = test.ruby_list(&["--no-color"]);

    output.assert_success();
    assert!(!output.stdout().contains("available)"));

    let output = test.ruby_list(&["--format", "json"]);
    let rubies: serde_json::Value = serde_json::from_str(&output.stdout()).unwrap();

    assert_eq!(rubies[0]["build"], MOCK_BUILD);
    assert!(rubies[0].get("newer_build").is_none());
}

#[test]
fn test_ruby_list_says_nothing_about_the_build_of_a_ruby_without_a_marker() {
    let mut test = RvTest::new();
    test.create_ruby_dir("ruby-3.4.5");

    let _mock = test.mock_releases(["3.4.5"].to_vec());
    let output = test.ruby_list(&["--format", "json"]);
    let rubies: serde_json::Value = serde_json::from_str(&output.stdout()).unwrap();

    output.assert_success();
    assert!(rubies[0].get("build").is_none());
    assert!(rubies[0].get("newer_build").is_none());
}
