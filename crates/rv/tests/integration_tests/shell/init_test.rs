use crate::common::RvTest;
use insta::assert_snapshot;

#[test]
fn test_zsh_shell_init_succeeds() {
    let test = RvTest::new();
    let output = test.rv(&["shell", "init", "zsh"]);
    output.assert_success();

    assert_snapshot!(output.normalized_stdout());
}

#[test]
fn test_bash_shell_init_succeeds() {
    let test = RvTest::new();
    let output = test.rv(&["shell", "init", "bash"]);
    output.assert_success();

    assert_snapshot!(output.normalized_stdout());
}

#[test]
fn test_fish_shell_init_succeeds() {
    let test = RvTest::new();
    let output = test.rv(&["shell", "init", "fish"]);
    output.assert_success();

    assert_snapshot!(output.normalized_stdout());
}

#[test]
fn test_nu_shell_init_succeeds() {
    let test = RvTest::new();
    let output = test.rv(&["shell", "init", "nu"]);
    output.assert_success();

    assert_snapshot!(output.normalized_stdout());
}

#[test]
fn test_shell_init_fails_without_shell() {
    let test = RvTest::new();
    let output = test.rv(&["shell", "init"]);
    output.assert_failure();

    assert_eq!(output.normalized_stdout(), "");
}
