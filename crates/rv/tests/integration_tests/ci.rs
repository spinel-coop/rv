use crate::common::RvTest;

#[test]
fn test_clean_install_download_test_gem() {
    let mut test = RvTest::new();
    test.use_gemfile("../rv-lockfile/tests/inputs/Gemfile.testsource");
    test.use_lockfile("../rv-lockfile/tests/inputs/Gemfile.lock.testsource");
    test.replace_source("http://gems.example.com", &test.server_url());

    let gemfile = fs_err::read_to_string(test.cwd.join("Gemfile")).unwrap();
    println!("{}", gemfile);

    let tarball_content =
        fs_err::read("../rv-gem-package/tests/fixtures/test-gem-1.0.0.gem").unwrap();
    let mock = test
        .mock_gem_download("test-gem-1.0.0.gem", &tarball_content)
        .create();

    let output = test.rv(&["ci"]);

    output.assert_success();
    mock.assert();
}

#[test]
fn test_clean_install_download_discourse() {
    let test = RvTest::new();
    test.use_gemfile("../rv-lockfile/tests/inputs/Gemfile.discourse");
    test.use_lockfile("../rv-lockfile/tests/inputs/Gemfile.lock.discourse");

    let output = test.rv(&["ci"]);
    output.assert_success();

    // Store a snapshot of all the files `rv ci` created.
    let test_dir_contents = std::process::Command::new("ls")
        .args(["-R".to_owned()])
        .current_dir(test.cwd)
        .output()
        .expect("ls should succeed")
        .stdout;
    let test_dir_contents =
        String::from_utf8(test_dir_contents).expect("ls -R should return UTF-8 bytes");
    insta::assert_snapshot!(test_dir_contents);
}
