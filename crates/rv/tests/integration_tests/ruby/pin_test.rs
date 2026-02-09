use crate::common::{RvOutput, RvTest};

impl RvTest {
    pub fn ruby_pin(&self, args: &[&str]) -> RvOutput {
        self.rv(&[&["ruby", "pin"], args].concat())
    }
}

#[test]
fn test_ruby_pin_ruby_output_format_consistency() {
    let test = RvTest::new();

    let set_pin = test.ruby_pin(&["3.4.7"]);
    set_pin.assert_success();
    assert_eq!(
        set_pin.normalized_stdout(),
        "/tmp/.ruby-version pinned to 3.4.7\n"
    );

    let show_pin = test.ruby_pin(&[]);
    show_pin.assert_success();
    assert_eq!(
        show_pin.normalized_stdout(),
        "/tmp/.ruby-version is pinned to 3.4.7\n"
    );
}
