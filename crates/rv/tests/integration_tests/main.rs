mod ci;
mod common;
mod ruby;
mod shell;

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
