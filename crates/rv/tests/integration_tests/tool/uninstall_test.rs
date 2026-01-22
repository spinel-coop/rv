use crate::common::{RvOutput, RvTest};

impl RvTest {
    pub fn tool_uninstall(&mut self, args: &[&str]) -> RvOutput {
        self.rv(&[&["tool", "uninstall"], args].concat())
    }
}

#[test]
fn test_tool_uninstall() {
    let mut test = RvTest::new();

    // Tool directory should not exist,
    // because it hasn't been installed yet.
    let tool_home = "/tmp/home/.local/share/rv/tools/test-gem-uninstalling@1.0.0";
    let exists = std::fs::exists(tool_home).unwrap();
    assert!(!exists);

    let _releases_mock = test.mock_releases(["4.0.0"].to_vec());

    let info_endpoint_content = fs_err::read("tests/fixtures/info-test-gem").unwrap();
    let _info_endpoint_mock = test
        .mock_info_endpoint("test-gem-uninstalling", &info_endpoint_content)
        .create();

    let tarball_content =
        fs_err::read("../rv-gem-package/tests/fixtures/test-gem-1.0.0.gem").unwrap();
    let _tarball_mock = test
        .mock_gem_download("test-gem-uninstalling-1.0.0.gem", &tarball_content)
        .create();

    let output = test.tool_install(&["test-gem-uninstalling"]);
    output.assert_success();

    // Test the dir exists now.
    let exists = std::fs::exists(tool_home).unwrap();
    assert!(!exists);

    // Manually remove tool
    test.tool_uninstall(&["test-gem"]).assert_success();

    // Tool directory should not exist.
    let exists = std::fs::exists(tool_home).unwrap();
    assert!(!exists);
}
