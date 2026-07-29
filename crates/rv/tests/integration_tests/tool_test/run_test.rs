use crate::common::{RvOutput, RvTest};

impl RvTest {
    pub fn tool_run(&mut self, args: &[&str]) -> RvOutput {
        self.rv(&[
            &["tool", "run", "--gem-server", &self.gemserver_url()],
            args,
        ]
        .concat())
    }
}

mod test {
    use owo_colors::OwoColorize as _;
    use rv_cache::rm_rf;

    use crate::RvTest;

    #[test]
    fn test_tool_run_works() {
        let mut test = RvTest::new();

        let releases_mock = test.mock_releases_all_platforms(["4.0.0"].to_vec());
        let ruby_mock = test.mock_ruby_download("4.0.0").create();
        let info_endpoint_mock = test.mock_info_endpoint("indirect").create();
        let tarball_mock = test.mock_gem_download("indirect-1.2.0.gem").create();

        let output = test.tool_run(&["indirect"]);

        let tool_home = "/tmp/home/.local/share/rv/tools/indirect@1.2.0";
        let expected_info_message = format!(
            "Installed {} version 1.2.0 to {}",
            "indirect".cyan(),
            tool_home.cyan()
        );
        output.assert_success();
        output.assert_stdout_contains(&expected_info_message);

        releases_mock.assert();
        ruby_mock.assert();
        info_endpoint_mock.assert();
        tarball_mock.assert();

        // Manually remove tool
        rm_rf(test.data_dir().join("rv/tools/indirect@1.2.0")).unwrap();
    }

    #[test]
    fn test_tool_run_works_with_namespaced_gem() {
        let mut test = RvTest::new();
        test.namespace = Some("@indirect".to_string());

        let releases_mock = test.mock_releases_all_platforms(["4.0.0"].to_vec());
        let ruby_mock = test.mock_ruby_download("4.0.0").create();
        let info_endpoint_mock = test.mock_info_endpoint("indirect").create();
        let tarball_mock = test.mock_gem_download("indirect-1.2.0.gem").create();

        // skip using `tool_run()` specifically so we do not include the namespace
        // in the gem server URL, and test that it gets added from the gem name.
        let output = test.rv(&[
            "tool",
            "run",
            "--gem-server",
            &test.server_url(),
            "@indirect/indirect",
        ]);

        let tool_home = "/tmp/home/.local/share/rv/tools/indirect@1.2.0";
        let expected_info_message = format!(
            "Installed {} version 1.2.0 to {}",
            "indirect".cyan(),
            tool_home.cyan()
        );
        output.assert_success();
        output.assert_stdout_contains(&expected_info_message);

        releases_mock.assert();
        ruby_mock.assert();
        info_endpoint_mock.assert();
        tarball_mock.assert();

        // Manually remove tool
        rm_rf(test.data_dir().join("rv/tools/indirect@1.2.0")).unwrap();
    }

    #[test]
    fn test_tool_run_with_extra_gem() {
        let mut test = RvTest::new();

        let releases_mock = test.mock_releases_all_platforms(["4.0.0"].to_vec());
        let ruby_mock = test.mock_ruby_download("4.0.0").create();
        let indirect_info_mock = test.mock_info_endpoint("indirect").create();
        let indirect_tarball_mock = test.mock_gem_download("indirect-1.2.0.gem").create();
        let racc_info_mock = test.mock_info_endpoint("racc").create();
        let racc_tarball_mock = test.mock_gem_download("racc-1.8.1.gem").create();

        let output = test.tool_run(&["-w", "racc", "indirect"]);

        let tool_home = "/tmp/home/.local/share/rv/tools/indirect@1.2.0";
        let expected_info_message = format!(
            "Installed {} version 1.2.0 to {}",
            "indirect".cyan(),
            tool_home.cyan()
        );
        output.assert_success();
        output.assert_stdout_contains(&expected_info_message);

        releases_mock.assert();
        ruby_mock.assert();
        indirect_info_mock.assert();
        indirect_tarball_mock.assert();
        racc_info_mock.assert();
        racc_tarball_mock.assert();

        // Manually remove tool
        rm_rf(test.data_dir().join("rv/tools/indirect@1.2.0")).unwrap();
    }

    #[test]
    fn test_tool_run_with_multiple_extra_gems() {
        let mut test = RvTest::new();

        let releases_mock = test.mock_releases_all_platforms(["4.0.0"].to_vec());
        let ruby_mock = test.mock_ruby_download("4.0.0").create();
        let indirect_info_mock = test.mock_info_endpoint("indirect").create();
        let indirect_tarball_mock = test.mock_gem_download("indirect-1.2.0.gem").create();
        let racc_info_mock = test.mock_info_endpoint("racc").create();
        let racc_tarball_mock = test.mock_gem_download("racc-1.8.1.gem").create();
        let second_with_info_mock = test.mock_info_endpoint("second-with").create();
        let second_with_tarball_mock = test.mock_gem_download("second-with-1.0.0.gem").create();

        let output = test.tool_run(&["--with", "racc", "--with", "second-with", "indirect"]);

        let tool_home = "/tmp/home/.local/share/rv/tools/indirect@1.2.0";
        let expected_info_message = format!(
            "Installed {} version 1.2.0 to {}",
            "indirect".cyan(),
            tool_home.cyan()
        );
        output.assert_success();
        output.assert_stdout_contains(&expected_info_message);

        releases_mock.assert();
        ruby_mock.assert();
        indirect_info_mock.assert();
        indirect_tarball_mock.assert();
        racc_info_mock.assert();
        racc_tarball_mock.assert();
        second_with_info_mock.assert();
        second_with_tarball_mock.assert();

        // Manually remove tool
        rm_rf(test.data_dir().join("rv/tools/indirect@1.2.0")).unwrap();
    }

    #[test]
    fn test_tool_run_with_pinned_version() {
        let mut test = RvTest::new();

        let releases_mock = test.mock_releases_all_platforms(["4.0.0"].to_vec());
        let ruby_mock = test.mock_ruby_download("4.0.0").create();
        let indirect_info_mock = test.mock_info_endpoint("indirect").create();
        let indirect_tarball_mock = test.mock_gem_download("indirect-1.2.0.gem").create();
        let racc_info_mock = test.mock_info_endpoint("racc").create();
        let racc_tarball_mock = test.mock_gem_download("racc-1.8.1.gem").create();

        let output = test.tool_run(&["--with", "racc@1.8.1", "indirect"]);

        let tool_home = "/tmp/home/.local/share/rv/tools/indirect@1.2.0";
        let expected_info_message = format!(
            "Installed {} version 1.2.0 to {}",
            "indirect".cyan(),
            tool_home.cyan()
        );
        output.assert_success();
        output.assert_stdout_contains(&expected_info_message);

        releases_mock.assert();
        ruby_mock.assert();
        indirect_info_mock.assert();
        indirect_tarball_mock.assert();
        racc_info_mock.assert();
        racc_tarball_mock.assert();

        // Manually remove tool
        rm_rf(test.data_dir().join("rv/tools/indirect@1.2.0")).unwrap();
    }

    #[test]
    fn test_tool_run_with_already_installed_tool() {
        let mut test = RvTest::new();

        // First, install the tool normally
        let releases_mock = test.mock_releases_all_platforms(["4.0.0"].to_vec());
        let ruby_mock = test.mock_ruby_download("4.0.0").create();
        let indirect_info_mock = test.mock_info_endpoint("indirect").create();
        let indirect_tarball_mock = test.mock_gem_download("indirect-1.2.0.gem").create();

        let output = test.tool_run(&["indirect"]);
        output.assert_success();

        releases_mock.assert();
        ruby_mock.assert();
        indirect_info_mock.assert();
        indirect_tarball_mock.assert();

        // Now run again with --with. The primary gem should NOT be re-fetched,
        // but the --with gem should be fetched and installed.
        let indirect_info_mock_2 = test.mock_info_endpoint("indirect").expect(0).create();
        let indirect_tarball_mock_2 = test
            .mock_gem_download("indirect-1.2.0.gem")
            .expect(0)
            .create();
        let racc_info_mock = test.mock_info_endpoint("racc").create();
        let racc_tarball_mock = test.mock_gem_download("racc-1.8.1.gem").create();

        let output = test.tool_run(&["--with", "racc", "indirect"]);
        output.assert_success();

        indirect_info_mock_2.assert();
        indirect_tarball_mock_2.assert();
        racc_info_mock.assert();
        racc_tarball_mock.assert();

        // Manually remove tool
        rm_rf(test.data_dir().join("rv/tools/indirect@1.2.0")).unwrap();
    }

    #[test]
    fn test_tool_run_with_nonexistent_gem() {
        let mut test = RvTest::new();

        let releases_mock = test.mock_releases_all_platforms(["4.0.0"].to_vec());
        let ruby_mock = test.mock_ruby_download("4.0.0").create();
        let indirect_info_mock = test.mock_info_endpoint("indirect").create();
        let indirect_tarball_mock = test.mock_gem_download("indirect-1.2.0.gem").create();
        let nonexistent_mock = test
            .mock_request("GET", "info/nonexistent")
            .with_status(404)
            .create();

        let output = test.tool_run(&["--with", "nonexistent", "indirect"]);
        output.assert_failure();

        releases_mock.assert();
        ruby_mock.assert();
        indirect_info_mock.assert();
        indirect_tarball_mock.assert();
        nonexistent_mock.assert();

        // Manually remove tool
        rv_cache::rm_rf(test.data_dir().join("rv/tools/indirect@1.2.0")).unwrap();
    }

    #[test]
    fn test_tool_run_with_no_command() {
        let mut test = RvTest::new();

        let output = test.tool_run(&["--with", "racc"]);
        output.assert_failure();
    }
}
