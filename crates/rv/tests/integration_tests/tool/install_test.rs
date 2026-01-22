use crate::common::{RvOutput, RvTest};
use owo_colors::OwoColorize;
use rv_cache::rm_rf;

impl RvTest {
    pub fn tool_install(&mut self, args: &[&str]) -> RvOutput {
        self.env.remove("RV_INSTALL_URL");
        self.rv(&[
            &["tool", "install", "--gem-server", &self.server_url()],
            args,
        ]
        .concat())
    }
}

#[test]
fn test_tool_install_twice() {
    let mut test = RvTest::new();

    let releases_mock = test.mock_releases(["4.0.0"].to_vec());

    let info_endpoint_content = fs_err::read("tests/fixtures/info-indirect-gem").unwrap();
    let info_endpoint_mock = test
        .mock_info_endpoint("indirect", &info_endpoint_content)
        .create();

    let tarball_content =
        fs_err::read("../rv-gem-package/tests/fixtures/indirect-1.2.0.gem").unwrap();
    let tarball_mock = test
        .mock_gem_download("indirect-1.2.0.gem", &tarball_content)
        .create();

    let output = test.tool_install(&["indirect"]);
    output.assert_success();

    let tool_home = "/tmp/home/.local/share/rv/tools/indirect@1.2.0";
    let expected_info_message = format!(
        "Installed {} version 1.2.0 to {}",
        "indirect".cyan(),
        tool_home.cyan()
    );

    let stdout = output.normalized_stdout();
    assert!(stdout.contains(&expected_info_message), "{}", stdout);

    releases_mock.assert();
    info_endpoint_mock.assert();
    tarball_mock.assert();

    // Manually remove tool
    rm_rf(
        test.temp_home()
            .join(".local/share/rv/tools/indirect@1.2.0"),
    )
    .unwrap();

    // Check it succeeds a second time
    let output = test.tool_install(&["indirect"]);
    output.assert_success();

    let stdout = output.normalized_stdout();
    assert!(stdout.contains(&expected_info_message), "{}", stdout);
}
