#[cfg(unix)]
use crate::common::{RvOutput, RvTest};

#[cfg(unix)]
use rv_cache::rm_rf;

#[cfg(unix)]
impl RvTest {
    pub fn tool_list(&mut self, args: &[&str]) -> RvOutput {
        self.rv(&[&["tool", "list"], args].concat())
    }
}

// This test calls tool_install() which downloads real Ruby binaries and is
// gated to Unix. See install_test.rs for details.
#[cfg(unix)]
#[test]
fn test_tool_list() {
    let mut test = RvTest::new();

    let _releases_mock = test.mock_releases_all_platforms(["4.0.0"].to_vec());

    let info_endpoint_content = fs_err::read("tests/fixtures/info-indirect-gem").unwrap();
    let _info_endpoint_mock = test
        .mock_info_endpoint("indirect", &info_endpoint_content)
        .create();

    let tarball_content =
        fs_err::read("../rv-gem-package/tests/fixtures/indirect-1.2.0.gem").unwrap();
    let _tarball_mock = test
        .mock_gem_download("indirect-1.2.0.gem", &tarball_content)
        .create();

    let output = test.tool_install(&["indirect"]);
    output.assert_success();

    // Test the list has 1 row
    let list_output = test.tool_list(&["--format", "json"]);
    list_output.assert_success();
    let json_out = list_output.normalized_stdout();
    assert_eq!(
        json_out,
        "[{\"gem_name\":\"indirect\",\"version\":\"1.2.0\"}]\n"
    );

    // Manually remove tool
    rm_rf(
        test.temp_home()
            .join(".local/share/rv/tools/indirect@1.2.0"),
    )
    .unwrap();

    // Test list has 0 rows.
    let second_list_output = test.tool_list(&["--format", "json"]);
    second_list_output.assert_success();
    let json_out = second_list_output.normalized_stdout();
    assert_eq!(json_out, "[]\n",);
}
