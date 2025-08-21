use crate::common::RvTest;
use insta::assert_snapshot;

#[test]
fn test_shell_env_succeeds() {
    let test = RvTest::new();
    let output = test.rv(&["shell", "env"]);

    assert_snapshot!(output.normalized_stdout());
    assert!(output.success());
}
