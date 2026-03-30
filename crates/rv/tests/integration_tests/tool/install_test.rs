use fs_err as fs;

use crate::common::{RvOutput, RvTest};
use owo_colors::OwoColorize;
use rv_cache::rm_rf;

impl RvTest {
    pub fn tool_install(&mut self, args: &[&str]) -> RvOutput {
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

    let releases_mock = test.mock_releases_all_platforms(["4.0.0"].to_vec());
    let ruby_mock = test.mock_ruby_download("4.0.0").create();

    let info_endpoint_mock = test.mock_info_endpoint("indirect").create();

    let tarball_mock = test.mock_gem_download("indirect-1.2.0.gem").create();

    let output = test.tool_install(&["indirect"]);
    output.assert_success();

    let tool_home = "/tmp/home/.local/share/rv/tools/indirect@1.2.0";
    let expected_info_message = format!(
        "Installed {} version 1.2.0 to {}",
        "indirect".cyan(),
        tool_home.cyan()
    );

    output.assert_stdout_contains(&expected_info_message);

    releases_mock.assert();
    ruby_mock.assert();
    info_endpoint_mock.assert();
    tarball_mock.assert();

    // Manually remove tool
    rm_rf(test.data_dir().join("rv/tools/indirect@1.2.0")).unwrap();

    // Check it succeeds a second time
    let output = test.tool_install(&["indirect"]);
    output.assert_success();

    output.assert_stdout_contains(&expected_info_message);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn test_tool_install_resolves_platform_specific_gems() {
    let mut test = RvTest::new();

    let releases_mock = test.mock_releases_all_platforms(["4.0.2"].to_vec());
    let ruby_mock = test.mock_ruby_download("4.0.2").create();

    let nokogiri_info_endpoint_mock = test.mock_info_endpoint("nokogiri").create();

    let racc_info_endpoint_mock = test.mock_info_endpoint("racc").create();

    let nokogiri_tarball_mock = test
        .mock_gem_download("nokogiri-1.19.0-arm64-darwin.gem")
        .create();
    let racc_tarball_mock = test.mock_gem_download("racc-1.8.1.gem").create();

    // Install it, with an explicit version.
    let output = test.tool_install(&["nokogiri@1.19.0"]);
    output.assert_success();

    let tool_home = "/tmp/home/.local/share/rv/tools/nokogiri@1.19.0-arm64-darwin";
    let expected_info_message = format!(
        "Installed {} version 1.19.0-arm64-darwin to {}",
        "nokogiri".cyan(),
        tool_home.cyan()
    );

    output.assert_stdout_contains(&expected_info_message);

    releases_mock.assert();
    ruby_mock.assert();
    nokogiri_info_endpoint_mock.assert();
    racc_info_endpoint_mock.assert();
    nokogiri_tarball_mock.assert();
    racc_tarball_mock.assert();
}

/// Tests users can explicitly install an older version of a gem.
#[test]
fn test_tool_install_non_latest_version() {
    let mut test = RvTest::new();

    let releases_mock = test.mock_releases_all_platforms(["4.0.0"].to_vec());
    let ruby_mock = test.mock_ruby_download("4.0.0").create();

    let info_endpoint_mock = test.mock_info_endpoint("indirect").create();

    let tarball_mock = test.mock_gem_download("indirect-1.1.0.gem").create();

    // Install it, with an explicit version.
    let output = test.tool_install(&["indirect@1.1.0"]);
    output.assert_success();

    let tool_home = "/tmp/home/.local/share/rv/tools/indirect@1.1.0";
    let expected_info_message = format!(
        "Installed {} version 1.1.0 to {}",
        "indirect".cyan(),
        tool_home.cyan()
    );

    output.assert_stdout_contains(&expected_info_message);

    releases_mock.assert();
    ruby_mock.assert();
    info_endpoint_mock.assert();
    tarball_mock.assert();
}

#[test]
fn test_tool_install_writes_ruby_version_file() {
    let mut test = RvTest::new();

    let releases_mock = test.mock_releases_all_platforms(["4.0.0"].to_vec());
    let ruby_mock = test.mock_ruby_download("4.0.0").create();

    let info_endpoint_mock = test.mock_info_endpoint("indirect").create();

    let tarball_mock = test.mock_gem_download("indirect-1.2.0.gem").create();

    let output = test.tool_install(&["indirect"]);
    output.assert_success();

    let tool_home = test.data_dir().join("rv/tools/indirect@1.2.0");
    let ruby_version_path = tool_home.join(".ruby-version");
    assert!(
        ruby_version_path.exists(),
        "Expected .ruby-version to exist at {}",
        ruby_version_path
    );
    let ruby_version = fs::read_to_string(ruby_version_path).unwrap();
    assert_eq!(ruby_version, "ruby-4.0.0\n");

    releases_mock.assert();
    ruby_mock.assert();
    info_endpoint_mock.assert();
    tarball_mock.assert();
}

#[test]
fn test_tool_install_package_data_tar_gz_with_trailing_garbage() {
    let mut test = RvTest::new();

    let releases_mock = test.mock_releases_all_platforms(["4.0.0"].to_vec());
    let ruby_mock = test.mock_ruby_download("4.0.0").create();

    let info_endpoint_mock = test.mock_info_endpoint("alba").create();

    let tarball_mock = test.mock_gem_download("alba-3.10.0.gem").create();

    let output = test.tool_install(&["alba"]);
    output.assert_failure();

    // Unpacks fine, but fails to install because it has no executables
    assert_eq!(
        output.normalized_stderr(),
        "Error: ToolError(ToolInstallError(NoExecutables))\n"
    );

    releases_mock.assert();
    ruby_mock.assert();
    info_endpoint_mock.assert();
    tarball_mock.assert();
}
