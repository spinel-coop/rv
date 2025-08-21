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
    assert!(output.success());
    assert_snapshot!(output.normalized_stdout(), @r"
      ruby-3.1.4    [36m/opt/rubies/3.1.4/bin/ruby[39m
      ruby-3.1.4    [36m/opt/rubies/ruby-3.1.4/bin/ruby[39m
    * ruby-3.2.0    [36m/opt/rubies/ruby-3.2.0/bin/ruby[39m
    ");

    test.create_ruby_dir("3.2.0");
    let output = test.ruby_list(&[]);
    assert!(output.success());
    assert_snapshot!(output.normalized_stdout(), @r"
      ruby-3.1.4    [36m/opt/rubies/3.1.4/bin/ruby[39m
      ruby-3.1.4    [36m/opt/rubies/ruby-3.1.4/bin/ruby[39m
      ruby-3.2.0    [36m/opt/rubies/3.2.0/bin/ruby[39m
    * ruby-3.2.0    [36m/opt/rubies/ruby-3.2.0/bin/ruby[39m
    ");

    test.env
        .insert("PATH".into(), "/opt/rubies/3.1.4/bin".into());

    let output = test.ruby_list(&[]);
    assert!(output.success());
    assert_snapshot!(output.normalized_stdout(), @r"
      ruby-3.1.4    [36m/opt/rubies/3.1.4/bin/ruby[39m
      ruby-3.1.4    [36m/opt/rubies/ruby-3.1.4/bin/ruby[39m
      ruby-3.2.0    [36m/opt/rubies/3.2.0/bin/ruby[39m
    * ruby-3.2.0    [36m/opt/rubies/ruby-3.2.0/bin/ruby[39m
    ");
}
