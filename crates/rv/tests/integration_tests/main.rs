mod clean_install;
mod common;
mod ruby;
mod run;
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
    let msg = error_re.unwrap().replace(&stderr, "");
    // On Windows, clap reports the exe name as 'rv.exe' instead of 'rv'
    let msg = msg.replace("'rv.exe'", "'rv'");
    assert_eq!(
        msg,
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
