use crate::common::{RvOutput, RvTest};

impl RvTest {
    pub fn ruby_pin(&self, args: &[&str]) -> RvOutput {
        self.rv(&[&["ruby", "pin"], args].concat())
    }
}

#[test]
fn test_ruby_pin_basic_test() {
    let test = RvTest::new();

    let set_pin = test.ruby_pin(&["3.4.7"]);
    set_pin.assert_success();
    assert_eq!(
        set_pin.normalized_stdout(),
        "/tmp/.ruby-version pinned to 3.4.7\n"
    );

    let version_file = test.temp_root().join(".ruby-version");
    assert!(version_file.exists());
    let content = fs_err::read_to_string(&version_file).unwrap();
    assert_eq!(content, format!("3.4.7\n"));

    let show_pin = test.ruby_pin(&[]);
    show_pin.assert_success();
    assert_eq!(
        show_pin.normalized_stdout(),
        "/tmp/.ruby-version is pinned to 3.4.7\n"
    );

    // Overwrite existing pin
    let set_pin = test.ruby_pin(&["3.2.0"]);
    set_pin.assert_success();
    assert_eq!(
        set_pin.normalized_stdout(),
        "/tmp/.ruby-version pinned to 3.2.0\n"
    );

    assert!(version_file.exists());
    let content = fs_err::read_to_string(&version_file).unwrap();
    assert_eq!(content, format!("3.2.0\n"));

    let show_pin = test.ruby_pin(&[]);
    show_pin.assert_success();
    assert_eq!(
        show_pin.normalized_stdout(),
        "/tmp/.ruby-version is pinned to 3.2.0\n"
    );

    // Pin a prerelease version
    let set_pin = test.ruby_pin(&["3.3.0-preview1"]);

    set_pin.assert_success();
    assert_eq!(
        set_pin.normalized_stdout(),
        "/tmp/.ruby-version pinned to 3.3.0-preview1\n"
    );

    let content = fs_err::read_to_string(&version_file).unwrap();
    assert_eq!(content, "3.3.0-preview1\n");

    // Pin a patch version
    let set_pin = test.ruby_pin(&["1.9.2-p0"]);

    set_pin.assert_success();
    assert_eq!(
        set_pin.normalized_stdout(),
        "/tmp/.ruby-version pinned to 1.9.2-p0\n"
    );

    let content = fs_err::read_to_string(&version_file).unwrap();
    assert_eq!(content, "1.9.2-p0\n");
}

#[test]
fn test_pin_runs_with_no_version() {
    let test = RvTest::new();

    let show_pin = test.ruby_pin(&[]);
    show_pin.assert_failure();
    assert!(
        show_pin
            .stderr()
            .contains("Error: RubyError(PinError(NoRubyRequest"),
        "pin without arguments did not print a nice error",
    );
}

#[test]
fn test_pin_runs_with_tool_versions() {
    let test = RvTest::new();

    let tool_versions_file = test.temp_root().join(".tool-versions");
    std::fs::write(&tool_versions_file, "ruby 3.2.0").unwrap();

    let show_pin = test.ruby_pin(&[]);
    show_pin.assert_success();
    assert_eq!(
        show_pin.normalized_stdout(),
        "/tmp/.tool-versions is pinned to 3.2.0\n"
    );

    // Overwrite existing pin
    let set_pin = test.ruby_pin(&["3.4.0"]);
    set_pin.assert_success();
    assert_eq!(
        set_pin.normalized_stdout(),
        "/tmp/.tool-versions pinned to 3.4.0\n"
    );

    // Verify the file contains the second version
    assert!(tool_versions_file.exists());
    let content = fs_err::read_to_string(&tool_versions_file).unwrap();
    assert_eq!(content, format!("ruby 3.4.0\n"));

    // try with leading whitespace
    fs_err::write(&tool_versions_file, " ruby 3.0.0").unwrap();

    let set_pin = test.ruby_pin(&["3.4.0"]);
    set_pin.assert_success();
    assert_eq!(
        set_pin.normalized_stdout(),
        "/tmp/.tool-versions pinned to 3.4.0\n"
    );

    // Verify the file contains the second version, but kept whitespace
    let content = fs_err::read_to_string(&tool_versions_file).unwrap();
    assert_eq!(content, " ruby 3.4.0\n");

    // Pin a fully qualified CRuby version
    let set_pin = test.ruby_pin(&["ruby-3.3.0"]);

    set_pin.assert_success();
    assert_eq!(
        set_pin.normalized_stdout(),
        "/tmp/.tool-versions pinned to 3.3.0\n"
    );

    // Verify the file contains the normalized version
    let content = fs_err::read_to_string(&tool_versions_file).unwrap();
    assert_eq!(content, " ruby 3.3.0\n");
}
