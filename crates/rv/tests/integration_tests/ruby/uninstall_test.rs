use crate::common::{RvOutput, RvTest};

impl RvTest {
    pub fn ruby_uninstall(&self, args: &[&str]) -> RvOutput {
        let mut cmd = self.rv_command();
        cmd.args(["ruby", "uninstall"]);
        cmd.args(args);

        let output = cmd.output().expect("Failed to execute rv command");
        RvOutput::new(self.temp_dir.path().as_str(), output)
    }
}

#[test]
fn test_ruby_uninstall_no_rubies() {
    let test = RvTest::new();
    let uninstall = test.ruby_uninstall(&["3.4.7"]);
    assert!(!uninstall.success());
    assert_eq!(
        uninstall.normalized_stderr(),
        "Error: UninstallError(NoMatchingRuby)\n"
    );
}

#[test]
fn test_ruby_uninstall_no_request() {
    let test = RvTest::new();
    let uninstall = test.ruby_uninstall(&[]);
    assert!(!uninstall.success());
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
    assert!(!uninstall.success());
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
        "Deleting /opt/rubies/ruby-3.3.5\n"
    );
}
