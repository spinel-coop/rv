#[cfg(unix)]
use fs_err as fs;

use crate::common::{RvOutput, RvTest};
#[cfg(unix)]
use owo_colors::OwoColorize;
#[cfg(unix)]
use rv_cache::rm_rf;

// tool_install() removes RV_TEST_PLATFORM so the subprocess detects native
// platform and downloads real Ruby binaries from GitHub. On Windows, this
// triggers the RubyInstaller2 flow which has a completely different download
// path. These tests are gated to Unix until dedicated Windows CI setup exists.
#[cfg(unix)]
impl RvTest {
    pub fn tool_install(&mut self, args: &[&str]) -> RvOutput {
        self.env.remove("RV_INSTALL_URL");
        // Remove RV_TEST_PLATFORM so the subprocess uses its compile-time native
        // platform. This is necessary because tool install downloads real Ruby
        // binaries from GitHub, and those binaries must match the host architecture.
        self.env.remove("RV_TEST_PLATFORM");
        self.rv(&[
            &["tool", "install", "--gem-server", &self.server_url()],
            args,
        ]
        .concat())
    }
}

#[cfg(unix)]
#[test]
fn test_tool_install_twice() {
    let mut test = RvTest::new();

    let releases_mock = test.mock_releases_all_platforms(["4.0.0"].to_vec());

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

/// Tests users can explicitly install an older version of a gem.
#[cfg(unix)]
#[test]
fn test_tool_install_non_latest_version() {
    let mut test = RvTest::new();

    let releases_mock = test.mock_releases_all_platforms(["4.0.0"].to_vec());

    let info_endpoint_content = fs_err::read("tests/fixtures/info-indirect-gem").unwrap();
    let info_endpoint_mock = test
        .mock_info_endpoint("indirect", &info_endpoint_content)
        .create();

    let tarball_content =
        fs_err::read("../rv-gem-package/tests/fixtures/indirect-1.1.0.gem").unwrap();
    let tarball_mock = test
        .mock_gem_download("indirect-1.1.0.gem", &tarball_content)
        .create();

    // Install it, with an explicit version.
    let output = test.tool_install(&["indirect@1.1.0"]);
    output.assert_success();

    let tool_home = "/tmp/home/.local/share/rv/tools/indirect@1.1.0";
    let expected_info_message = format!(
        "Installed {} version 1.1.0 to {}",
        "indirect".cyan(),
        tool_home.cyan()
    );

    let stdout = output.normalized_stdout();
    assert!(stdout.contains(&expected_info_message), "{}", stdout);

    releases_mock.assert();
    info_endpoint_mock.assert();
    tarball_mock.assert();
}

#[cfg(unix)]
#[test]
fn test_tool_install_writes_ruby_version_file() {
    let mut test = RvTest::new();

    let releases_mock = test.mock_releases_all_platforms(["4.0.0"].to_vec());

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

    let tool_home = test
        .temp_home()
        .join(".local/share/rv/tools/indirect@1.2.0");
    let ruby_version_path = tool_home.join(".ruby-version");
    assert!(
        ruby_version_path.exists(),
        "Expected .ruby-version to exist at {}",
        ruby_version_path
    );
    let ruby_version = fs::read_to_string(ruby_version_path).unwrap();
    assert_eq!(ruby_version, "ruby-4.0.0\n");

    releases_mock.assert();
    info_endpoint_mock.assert();
    tarball_mock.assert();
}
