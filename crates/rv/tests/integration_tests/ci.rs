use crate::common::RvTest;

#[test]
fn test_clean_install_download_test_gem() {
    let mut test = RvTest::new();
    // Install some Ruby first.
    // test.rv(&["ruby", "install", "4.0.0"]).assert_success();

    // Now we can use rv.
    test.use_gemfile("../rv-lockfile/tests/inputs/Gemfile.testsource");
    test.use_lockfile("../rv-lockfile/tests/inputs/Gemfile.testsource.lock");
    test.replace_source("http://gems.example.com", &test.server_url());

    let gemfile = fs_err::read_to_string(test.cwd.join("Gemfile")).unwrap();
    println!("{}", gemfile);

    let tarball_content =
        fs_err::read("../rv-gem-package/tests/fixtures/test-gem-1.0.0.gem").unwrap();
    let mock = test
        .mock_gem_download("test-gem-1.0.0.gem", &tarball_content)
        .create();

    let output = test.rv(&["ci", "--verbose"]);
    output.assert_success();
    mock.assert();
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn test_clean_install_native_macos_aarch64() {
    let test = RvTest::new();
    test.use_gemfile("../rv-lockfile/tests/inputs/Gemfile.testwithnative");
    test.use_lockfile("../rv-lockfile/tests/inputs/Gemfile.testwithnative.lock");
    let output = test.rv(&["ci", "--skip-compile-extensions"]);
    output.assert_success();

    // Store a snapshot of all the files `rv ci` created.
    let files_sorted = find_all_files_in_dir(test.cwd.as_ref());
    insta::assert_snapshot!(files_sorted);
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn test_clean_install_native_linux_x86_64() {
    let test = RvTest::new();
    test.use_gemfile("../rv-lockfile/tests/inputs/Gemfile.testwithnative");
    test.use_lockfile("../rv-lockfile/tests/inputs/Gemfile.testwithnative.lock");
    let output = test.rv(&["ci", "--skip-compile-extensions"]);
    output.assert_success();

    // Store a snapshot of all the files `rv ci` created.
    let files_sorted = find_all_files_in_dir(test.cwd.as_ref());
    insta::assert_snapshot!(files_sorted);
}

#[test]
fn test_clean_install_download_faker() {
    let test = RvTest::new();
    // https://github.com/faker-ruby/faker/blob/2f8b18b112fb3b7d2750321a8e574518cfac0d53/Gemfile
    test.use_gemfile("../rv-lockfile/tests/inputs/Gemfile.faker");
    // https://github.com/faker-ruby/faker/blob/2f8b18b112fb3b7d2750321a8e574518cfac0d53/Gemfile.lock
    test.use_lockfile("../rv-lockfile/tests/inputs/Gemfile.faker.lock");

    let output = test.rv(&["ci", "--skip-compile-extensions"]);
    output.assert_success();

    // Store a snapshot of all the files `rv ci` created.
    let files_sorted = find_all_files_in_dir(test.cwd.as_ref());
    insta::assert_snapshot!(files_sorted);
}

fn find_all_files_in_dir(cwd: &std::path::Path) -> String {
    let test_dir_contents = std::process::Command::new("find")
        .args([".", "-type", "f"])
        .current_dir(cwd)
        .output()
        .expect("ls should succeed")
        .stdout;
    let test_dir_contents =
        String::from_utf8(test_dir_contents).expect("'find' should return UTF-8 bytes");
    let mut lines: Vec<_> = test_dir_contents
        .lines()
        // This file is created when running with coverage, we don't want to include it.
        .filter(|line| !line.ends_with("profraw"))
        .collect();
    lines.sort();
    lines.join("\n")
}
