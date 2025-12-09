use crate::common::{RvOutput, RvTest};

impl RvTest {
    pub fn ruby_pin(&self, args: &[&str]) -> RvOutput {
        let mut cmd = self.rv_command();
        cmd.args(["ruby", "pin"]);
        cmd.args(args);

        let output = cmd.output().expect("Failed to execute rv command");
        RvOutput::new(self.temp_dir.path().as_str(), output)
    }
}

#[test]
fn test_ruby_pin_ruby_output_format_consistency() {
    let test = RvTest::new();

    let set_pin = test.ruby_pin(&["3.4.7"]);
    assert!(set_pin.success());
    assert_eq!(
        set_pin.normalized_stdout(),
        "/.ruby-version pinned to ruby-3.4.7\n"
    );

    let show_pin = test.ruby_pin(&[]);
    assert!(show_pin.success());
    assert_eq!(
        show_pin.normalized_stdout(),
        "/.ruby-version is pinned to ruby-3.4.7\n"
    );
}
