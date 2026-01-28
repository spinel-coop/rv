mod ci;
mod common;
mod ruby;
mod run_test;
mod shell;
mod tool;

use crate::common::RvTest;
use regex::Regex;

#[test]
fn test_no_command() {
    let test = RvTest::new();
    let result = test.rv(&["--color=always"]);
    result.assert_failure();
    let stderr = result.stderr();
    let error_re = Regex::new(r"(?s)\n  \[subcommands:.*");
    assert_eq!(
        error_re.unwrap().replace(&stderr, ""),
        "error: 'rv' requires a subcommand but one was not provided",
    );
}

#[test]
fn test_global_flags() {
    let test = RvTest::new();
    let result = test.rv(&["--help"]);
    result.assert_success();
    let stdout = result.stdout();
    assert!(
        !stdout.contains("--gemfile"),
        "--gemfile should not be a global flag"
    );
}
