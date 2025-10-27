use std::env::current_dir;

use crate::common::RvTest;

#[test]
fn test_clean_install_download() {
    let mut test = RvTest::new();

    println!("{:?}", current_dir());
    let tarball_content =
        fs_err::read("../rv-gem-package/tests/fixtures/test-gem-1.0.0.gem").unwrap();
    let mock = test
        .mock_gem_download("test-gem-1.0.0.gem", &tarball_content)
        .create();

    let rack_gemfile = "../rv-lockfile/tests/inputs/Gemfile.lock.testsource";
    let mut lockfile = fs_err::read_to_string(rack_gemfile).unwrap();
    lockfile = lockfile.replace("http://gems.example.com", &test.server_url());
    let _ = fs_err::write(test.cwd.join("Gemfile.lock"), &lockfile);
    let output = test.rv(&["ci"]);

    output.assert_success();
    mock.assert();
}
