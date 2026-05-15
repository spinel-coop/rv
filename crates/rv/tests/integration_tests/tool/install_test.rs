use fs_err as fs;

use crate::common::{RvOutput, RvTest};
use owo_colors::OwoColorize;
use rv_cache::rm_rf;

impl RvTest {
    pub fn tool_install(&mut self, args: &[&str]) -> RvOutput {
        self.rv(&[
            &["tool", "install", "--gem-server", &self.gemserver_url()],
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

#[test]
fn test_tool_install_with_server_with_path_no_trailing_slash() {
    let mut test = RvTest::namespaced("@indirect".to_string());

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
fn test_tool_install_offline_with_no_cache_fails() {
    let test = RvTest::new();

    test.create_ruby_dir("ruby-4.0.0");

    let output = test.rv(&[
        "--offline",
        "tool",
        "install",
        "--gem-server",
        &test.gemserver_url(),
        "indirect",
    ]);

    output.assert_failure();
    output.assert_stderr_contains("OfflineGemInfoMissing");
}

#[test]
fn test_tool_install_offline_uses_cached_metadata_and_gem() {
    let mut test = RvTest::new();

    test.create_ruby_dir("ruby-4.0.0");

    // Pre-populate the compact-index blob for `indirect` and the gem archive
    // download cache, so offline tool-install can satisfy itself from disk.
    let cache_dir = test.enable_cache();

    let info_content = fs::read("tests/fixtures/info-indirect-gem").unwrap();
    let compact_index_dir = cache_dir
        .join("gemdeps-v0")
        .join("compact_index")
        .join("info");
    fs_err::create_dir_all(&compact_index_dir).unwrap();
    fs_err::write(compact_index_dir.join("indirect"), &info_content).unwrap();

    let gem_path = test.gem_package_download_path("indirect-1.2.0.gem");
    let gem_url = format!("{}/{}", test.server_url(), gem_path);
    let gem_content = fs_err::read("../rv-gem-package/tests/fixtures/indirect-1.2.0.gem").unwrap();
    let cache_key = rv_cache::cache_digest(gem_url.as_str());
    let gems_dir = cache_dir.join("gem-v0").join("gems");
    fs_err::create_dir_all(&gems_dir).unwrap();
    fs_err::write(gems_dir.join(format!("{}.gem", cache_key)), &gem_content).unwrap();

    // Trip-wires: any HTTP fetch would match one of these and fail expect(0).
    let no_info_fetch = test
        .mock_request("GET", "/info/indirect")
        .with_status(200)
        .expect(0)
        .create();
    let no_gem_fetch = test
        .mock_request("GET", gem_path.as_str())
        .with_status(200)
        .expect(0)
        .create();

    let output = test.rv(&[
        "--offline",
        "tool",
        "install",
        "--gem-server",
        &test.gemserver_url(),
        "indirect",
    ]);

    output.assert_success();
    no_info_fetch.assert();
    no_gem_fetch.assert();
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
        "Error: ToolError(ToolInstallError(NoMatchingExecutable(\"alba\")))\n"
    );

    releases_mock.assert();
    ruby_mock.assert();
    info_endpoint_mock.assert();
    tarball_mock.assert();
}
