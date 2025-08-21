use crate::common::RvTest;
use insta::assert_snapshot;

#[test]
fn test_shell_init_succeeds() {
    let test = RvTest::new();
    let output = test.rv(&["shell", "init"]);

    assert_snapshot!(output.normalized_stdout());
    assert!(output.success());
}
