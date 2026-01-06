mod env_test;
mod init_test;

use crate::common::RvTest;
use insta::assert_snapshot;

#[test]
fn test_zsh_succeeds() {
    let test = RvTest::new();
    let output = test.rv(&["shell", "zsh"]);
    output.assert_success();

    assert_snapshot!(output.normalized_stdout());
}

#[test]
fn test_bash_succeeds() {
    let test = RvTest::new();
    let output = test.rv(&["shell", "bash"]);
    output.assert_success();

    assert_snapshot!(output.normalized_stdout());
}

#[test]
fn test_fish_succeeds() {
    let test = RvTest::new();
    let output = test.rv(&["shell", "fish"]);
    output.assert_success();

    assert_snapshot!(output.normalized_stdout());
}

#[test]
fn test_nu_succeeds() {
    let test = RvTest::new();
    let output = test.rv(&["shell", "nu"]);
    output.assert_success();

    assert_snapshot!(output.normalized_stdout());
}

#[test]
fn test_fails_without_shell() {
    let test = RvTest::new();
    let output = test.rv(&["shell"]);
    output.assert_failure();

    let stderr = output.stderr();
    assert!(
        stderr.contains("the following required arguments were not provided:\n  <SHELL>"),
        "shell without arguments did not print a nice error",
    );
}
