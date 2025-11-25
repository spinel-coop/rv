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

#[test]
fn test_clean_install_download_discourse() {
    let test = RvTest::new();

    let discourse_gemfile = "../rv-lockfile/tests/inputs/Gemfile.lock.discourse";
    let lockfile = fs_err::read_to_string(discourse_gemfile).unwrap();
    let _ = fs_err::write(test.cwd.join("Gemfile.lock"), &lockfile);
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
