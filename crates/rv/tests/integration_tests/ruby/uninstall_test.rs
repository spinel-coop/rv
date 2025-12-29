use crate::common::{RvOutput, RvTest};

impl RvTest {
    pub fn ruby_uninstall(&self, args: &[&str]) -> RvOutput {
        self.rv(&[&["ruby", "uninstall"], args].concat())
    }
}

#[test]
fn test_ruby_uninstall_no_rubies() {
    let test = RvTest::new();
    let uninstall = test.ruby_uninstall(&["3.4.7"]);
    uninstall.assert_failure();
    assert_eq!(
        uninstall.normalized_stderr(),
        "Error: UninstallError(NoMatchingRuby)\n"
    );
}

#[test]
fn test_ruby_uninstall_no_request() {
    let test = RvTest::new();
    let uninstall = test.ruby_uninstall(&[]);
    uninstall.assert_failure();
    assert_eq!(
        uninstall.normalized_stderr(),
        "error: the following required arguments were not provided:\n  <VERSION>\n\nUsage: rv ruby uninstall --no-cache <VERSION>\n\nFor more information, try '--help'.\n"
    );
}

#[test]
fn test_ruby_uninstall_no_matching_rubies() {
    let test = RvTest::new();
    test.create_ruby_dir("ruby-3.3.5");
    let uninstall = test.ruby_uninstall(&["3.4.5"]);
    uninstall.assert_failure();
    assert_eq!(
        uninstall.normalized_stderr(),
        "Error: UninstallError(NoMatchingRuby)\n"
    );
}

#[test]
fn test_ruby_uninstall_matching_request() {
    let test = RvTest::new();
    test.create_ruby_dir("ruby-3.3.5");
    let uninstall = test.ruby_uninstall(&["3.3.5"]);
    uninstall.assert_success();
    assert_eq!(
        uninstall.normalized_stdout(),
        "Deleting /tmp/opt/rubies/ruby-3.3.5\n"
    );
}
