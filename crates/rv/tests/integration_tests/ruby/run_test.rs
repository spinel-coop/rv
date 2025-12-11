use std::str::FromStr;

use crate::common::{RvOutput, RvTest};
use rv_ruby::request::RubyRequest;

#[derive(Debug, Default)]
pub struct RunOptions {
    pub set_no_install: bool,
}

impl RvTest {
    pub fn ruby_run(&self, version: RubyRequest, options: RunOptions, args: &[&str]) -> RvOutput {
        let RunOptions { set_no_install } = options;
        let mut cmd = self.rv_command();
        cmd.args(["ruby", "run"]);
        if set_no_install {
            cmd.arg("--no-install");
        }
        cmd.args([&version.to_string(), "--"]);
        cmd.args(args);

        let output = cmd.output().expect("Failed to execute rv run");
        RvOutput::new(self.temp_dir.path().as_str(), output)
    }
}

#[test]
fn test_ruby_run_simple() {
    let test = RvTest::new();
    test.create_ruby_dir("ruby-3.3.5");
    let output = test.ruby_run(
        RubyRequest::from_str("3.3.5").expect("this is a valid Ruby version"),
        Default::default(),
        &["-e", "'puts \"Hello, World\"'"],
    );

    output.assert_success();
    assert!(output.stderr().is_empty());
    assert_eq!(
        output.normalized_stdout(),
        "ruby\n3.3.5\naarch64-darwin23\naarch64\ndarwin23\n\n"
    );
}

#[test]
fn test_ruby_run_simple_no_install() {
    let test = RvTest::new();
    test.create_ruby_dir("ruby-3.3.5");

    // This should pass because we already installed 3.3.5
    let output = test.ruby_run(
        RubyRequest::from_str("3.3.5").expect("this is a valid Ruby version"),
        RunOptions {
            set_no_install: true,
        },
        &["-e", "'puts \"Hello, World\"'"],
    );

    output.assert_success();
    assert!(output.stderr().is_empty());
    assert_eq!(
        output.normalized_stdout(),
        "ruby\n3.3.5\naarch64-darwin23\naarch64\ndarwin23\n\n"
    );
}
