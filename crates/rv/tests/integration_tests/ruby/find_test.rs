use crate::common::{RvOutput, RvTest};

impl RvTest {
    pub fn ruby_find(&self, args: &[&str]) -> RvOutput {
        let mut cmd = self.rv_command();
        cmd.args(["ruby", "find"]);
        cmd.args(args);

        let output = cmd.output().expect("Failed to execute rv command");
        RvOutput::new(self.temp_dir.path().as_str(), output)
    }
}

#[test]
fn test_ruby_find_no_rubies() {
    let test = RvTest::new();
    let find = test.ruby_find(&[]);
    find.assert_failure();
    assert_eq!(
        find.normalized_stderr(),
        "Error: FindError(NoMatchingRuby)\n"
    );
}

#[test]
fn test_ruby_find_no_matching_rubies() {
    let test = RvTest::new();
    test.create_ruby_dir("ruby-3.3.5");
    let find = test.ruby_find(&["3.4.5"]);
    find.assert_failure();
    assert_eq!(
        find.normalized_stderr(),
        "Error: FindError(NoMatchingRuby)\n"
    );
}

#[test]
fn test_ruby_find_invalid_version() {
    let test = RvTest::new();
    let find = test.ruby_find(&["3.4.5.6.7"]);
    find.assert_failure();
    assert_eq!(
        find.normalized_stderr(),
        "Error: FindError(InvalidVersion(TooManySegments(\"3.4.5.6.7\")))\n"
    );
}

#[test]
fn test_ruby_find_matching_request() {
    let test = RvTest::new();
    test.create_ruby_dir("ruby-3.3.5");
    let find = test.ruby_find(&["3.3.5"]);
    find.assert_success();
    assert_eq!(
        find.normalized_stdout(),
        "/opt/rubies/ruby-3.3.5/bin/ruby\n"
    );
}

#[test]
fn test_ruby_find_default() {
    let test = RvTest::new();
    test.create_ruby_dir("ruby-3.3.5");
    let find = test.ruby_find(&[]);
    find.assert_success();
    assert_eq!(
        find.normalized_stdout(),
        "/opt/rubies/ruby-3.3.5/bin/ruby\n"
    );
}

#[test]
fn test_ruby_find_dot_ruby_version_empty() {
    let test = RvTest::new();
    std::fs::write(test.temp_dir.path().join(".ruby-version"), "").unwrap();
    test.create_ruby_dir("ruby-3.3.5");
    test.create_ruby_dir("ruby-3.4.5");
    let find = test.ruby_find(&[]);
    find.assert_failure();
    assert_eq!(
        find.normalized_stderr(),
        "Error: ConfigError(RequestError(EmptyInput))\n"
    );
}

#[test]
fn test_ruby_find_dot_ruby_version_matching() {
    let test = RvTest::new();
    std::fs::write(test.temp_dir.path().join(".ruby-version"), "3.3.5\n").unwrap();
    test.create_ruby_dir("ruby-3.3.5");
    test.create_ruby_dir("ruby-3.4.5");
    let find = test.ruby_find(&[]);
    find.assert_success();
    assert_eq!(
        find.normalized_stdout(),
        "/opt/rubies/ruby-3.3.5/bin/ruby\n"
    );
}

#[test]
fn test_ruby_find_multiple_matching() {
    let test = RvTest::new();
    test.create_ruby_dir("ruby-3.3.5");
    test.create_ruby_dir("3.3.5");
    let find = test.ruby_find(&["3.3.5"]);
    find.assert_success();
    assert_eq!(
        find.normalized_stdout(),
        "/opt/rubies/ruby-3.3.5/bin/ruby\n"
    );
    let find = test.ruby_find(&["ruby-3.3.5"]);
    find.assert_success();
    assert_eq!(
        find.normalized_stdout(),
        "/opt/rubies/ruby-3.3.5/bin/ruby\n"
    );
}

#[test]
fn test_ruby_find_matching_jruby() {
    let test = RvTest::new();
    test.create_ruby_dir("ruby-11");
    test.create_ruby_dir("jruby-9.4.8.0");
    test.create_ruby_dir("jruby-10.0.1.0");
    let find = test.ruby_find(&["jruby"]);
    find.assert_success();
    assert_eq!(
        find.normalized_stdout(),
        "/opt/rubies/jruby-10.0.1.0/bin/ruby\n"
    );
    let find = test.ruby_find(&["jruby-9"]);
    find.assert_success();
    assert_eq!(
        find.normalized_stdout(),
        "/opt/rubies/jruby-9.4.8.0/bin/ruby\n"
    );
}
