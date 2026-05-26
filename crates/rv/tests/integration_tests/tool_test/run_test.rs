use crate::{RvOutput, RvTest};

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
}
