#[cfg(unix)]
use crate::common::{RvOutput, RvTest};

#[cfg(unix)]
use fs_err as fs;

#[cfg(unix)]
impl RvTest {
    pub fn tool_uninstall(&mut self, args: &[&str]) -> RvOutput {
        self.rv(&[&["tool", "uninstall"], args].concat())
    }
}

// On Windows, the tool directory resolves to APPDATA/rv/tools/ (via etcetera)
// which differs from the XDG path .local/share/rv/tools/ this test creates.
#[cfg(unix)]
#[test]
fn test_tool_uninstall() {
    let mut test = RvTest::new();

    // Create a tools directory with a fake tool in it.
    // This fakes installing the gem 'test-gem'.
    let tool_root = &test.temp_home();
    let tool_home = tool_root.join(".local/share/rv/tools/test-gem@1.0.0");
    fs::create_dir_all(&tool_home).unwrap();

    // Test the dir exists now.
    assert!(fs::exists(&tool_home).unwrap());

    // Run `rv tool uninstall test-gem`
    test.tool_uninstall(&["test-gem"]).assert_success();

    // Tool directory should not exist.
    assert!(!fs::exists(&tool_home).unwrap());
}
