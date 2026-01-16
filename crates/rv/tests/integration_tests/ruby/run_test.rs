use crate::common::{RvOutput, RvTest};
use std::fs;

#[derive(Debug, Default)]
pub struct RunOptions {
    pub set_no_install: bool,
}

impl RvTest {
    pub fn ruby_run(&self, version: Option<&str>, options: RunOptions, args: &[&str]) -> RvOutput {
        let RunOptions { set_no_install } = options;
        let mut run_args = vec!["ruby", "run"];

        if set_no_install {
            run_args.push("--no-install");
        }
        if let Some(version) = version {
            run_args.push(version);
        }
        run_args.push("--");

        self.rv(&[&run_args, args].concat())
    }
}

#[test]
fn test_ruby_run_simple() {
    let test = RvTest::new();
    test.create_ruby_dir("ruby-3.3.5");
    let output = test.ruby_run(
        Some("3.3.5"),
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
fn test_ruby_run_interpreter_cache() {
    let mut test = RvTest::new();

    test.create_ruby_dir("ruby-3.3.5");
    test.create_ruby_dir("ruby-3.4.1");

    let cache_dir = test.enable_cache();

    let output = test.ruby_run(
        Some("3.4.1"),
        Default::default(),
        &["-e", "'puts \"Hello, World\"'"],
    );

    output.assert_success();

    let interpreters_dir = cache_dir.join("ruby-v0").join("interpreters");
    assert!(interpreters_dir.exists());

    // it should cache a single version, not both versions
    assert_eq!(
        fs::read_dir(&interpreters_dir)
            .unwrap()
            .collect::<Vec<_>>()
            .len(),
        1
    )
}

#[test]
fn test_ruby_run_default() {
    let test = RvTest::new();
    test.create_ruby_dir("ruby-3.3.5");
    let output = test.ruby_run(None, Default::default(), &["-e", "'puts \"Hello, World\"'"]);

    output.assert_success();
    assert!(output.stderr().is_empty());
    assert_eq!(
        output.normalized_stdout(),
        "ruby\n3.3.5\naarch64-darwin23\naarch64\ndarwin23\n\n"
    );
}

#[test]
fn test_ruby_run_default_skips_prereleases() {
    let test = RvTest::new();
    test.create_ruby_dir("ruby-3.4.8");
    test.create_ruby_dir("ruby-4.0.0-preview3");
    let output = test.ruby_run(None, Default::default(), &["-e", "'puts \"Hello, World\"'"]);

    output.assert_success();
    assert!(output.stderr().is_empty());
    assert_eq!(
        output.normalized_stdout(),
        "ruby\n3.4.8\naarch64-darwin23\naarch64\ndarwin23\n\n"
    );
}

#[test]
fn test_ruby_run_simple_no_install() {
    let test = RvTest::new();
    test.create_ruby_dir("ruby-3.3.5");

    // This should pass because we already installed 3.3.5
    let output = test.ruby_run(
        Some("3.3.5"),
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

#[test]
fn test_ruby_run_invalid_version() {
    let test = RvTest::new();
    let output = test.ruby_run(
        Some("3.4.5.6.7"),
        Default::default(),
        &["-e", "'puts \"Hello, World\"'"],
    );

    output.assert_failure();
    assert_eq!(
        output.normalized_stderr(),
        "error: invalid value '3.4.5.6.7' for '[VERSION]': Could not parse version 3.4.5.6.7, no more than 4 numbers are allowed\n\nFor more information, try '--help'.\n",
    );
}
