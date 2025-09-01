use crate::common::{RvOutput, RvTest};
use insta::assert_snapshot;

impl RvTest {
    pub fn ruby_list(&self, args: &[&str]) -> RvOutput {
        let mut cmd = self.rv_command();
        cmd.args(["ruby", "list"]);
        cmd.args(args);

        let output = cmd.output().expect("Failed to execute rv command");
        RvOutput::new(self.temp_dir.path().as_str(), output)
    }
}

#[test]
fn test_ruby_list_text_output_empty() {
    let test = RvTest::new();
    let output = test.ruby_list(&[]);

    assert!(output.success(), "rv ruby list should succeed");
    assert_snapshot!(output.normalized_stdout());
}

#[test]
fn test_ruby_list_json_output_empty() {
    let test = RvTest::new();
    let output = test.ruby_list(&["--format", "json"]);

    assert!(
        output.success(),
        "rv ruby list --format json should succeed"
    );
    assert_snapshot!(output.normalized_stdout());
}

#[test]
fn test_ruby_list_text_output_with_rubies() {
    let test = RvTest::new();

    // Create some mock Ruby installations
    test.create_ruby_dir("ruby-3.1.4");
    test.create_ruby_dir("ruby-3.2.0");

    let output = test.ruby_list(&[]);

    assert!(output.success(), "rv ruby list should succeed");
    assert_snapshot!(output.normalized_stdout());
}

#[test]
fn test_ruby_list_json_output_with_rubies() {
    let test = RvTest::new();

    // Create some mock Ruby installations
    test.create_ruby_dir("ruby-3.1.4");
    test.create_ruby_dir("ruby-3.2.0");

    let output = test.ruby_list(&["--format", "json"]);

    assert!(
        output.success(),
        "rv ruby list --format json should succeed"
    );

    // Verify it's valid JSON
    let stdout = output.stdout();
    let _: serde_json::Value = serde_json::from_str(&stdout).expect("Output should be valid JSON");

    assert_snapshot!(output.normalized_stdout());
}

#[test]
fn test_ruby_list_multiple_matching_rubies() {
    let mut test = RvTest::new();

    let project_dir = test.temp_dir.path().join("project");
    std::fs::create_dir_all(project_dir.as_path()).unwrap();
    std::fs::write(project_dir.join(".ruby-version"), b"3").unwrap();
    test.cwd = project_dir;

    // Create some mock Ruby installations
    test.create_ruby_dir("ruby-3.1.4");
    test.create_ruby_dir("ruby-3.2.0");
    test.create_ruby_dir("3.1.4");

    let output = test.ruby_list(&[]);
    output.assert_success();
    assert_snapshot!(output.normalized_stdout(), @r"
      ruby-3.1.4 [installed] /opt/rubies/3.1.4/bin/ruby
      ruby-3.1.4 [installed] /opt/rubies/ruby-3.1.4/bin/ruby
    * ruby-3.2.0 [installed] /opt/rubies/ruby-3.2.0/bin/ruby
    ");

    test.create_ruby_dir("3.2.0");
    let output = test.ruby_list(&[]);
    output.assert_success();
    assert_snapshot!(output.normalized_stdout(), @r"
      ruby-3.1.4 [installed] /opt/rubies/3.1.4/bin/ruby
      ruby-3.1.4 [installed] /opt/rubies/ruby-3.1.4/bin/ruby
      ruby-3.2.0 [installed] /opt/rubies/3.2.0/bin/ruby
    * ruby-3.2.0 [installed] /opt/rubies/ruby-3.2.0/bin/ruby
    ");

    test.env
        .insert("PATH".into(), "/opt/rubies/3.1.4/bin".into());

    let output = test.ruby_list(&[]);
    output.assert_success();
    assert_snapshot!(output.normalized_stdout(), @r"
      ruby-3.1.4 [installed] /opt/rubies/3.1.4/bin/ruby
      ruby-3.1.4 [installed] /opt/rubies/ruby-3.1.4/bin/ruby
      ruby-3.2.0 [installed] /opt/rubies/3.2.0/bin/ruby
    * ruby-3.2.0 [installed] /opt/rubies/ruby-3.2.0/bin/ruby
    ");
}

#[test]
fn test_ruby_list_with_available_and_installed() {
    let mut test = RvTest::new();
    test.create_ruby_dir("ruby-3.1.4");

    let releases_body = r#"[
        { "name": "3.4.5", "assets": [{"name": "portable-ruby-3.4.5.arm64_sonoma.bottle.tar.gz", "browser_download_url": "http://..."}]}
    ]"#;
    let mock = test.mock_releases(releases_body);
    let output = test.rv(&["ruby", "list"]);

    mock.assert();
    output.assert_success();

    // 3.1.4 and 3.4.5 should be listed, with 3.1.4 marked as installed
    insta::assert_snapshot!(output.normalized_stdout());
}

#[test]
fn test_ruby_list_with_no_installed_rubies_is_empty() {
    let test = RvTest::new();
    let output = test.rv(&["ruby", "list"]);
    output.assert_success();

    // The output will be completely empty because no rubies are installed
    // and the API is disabled.
    assert_eq!(output.normalized_stdout(), "");
}
