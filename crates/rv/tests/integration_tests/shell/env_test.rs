use std::fs::create_dir_all;

use crate::common::RvTest;
use camino::Utf8PathBuf;
use insta::assert_snapshot;
use rv_cache::rm_rf;
use tempfile::tempdir;

#[test]
fn test_shell_env_succeeds() {
    let test = RvTest::new();
    let output = test.rv(&["shell", "env", "zsh"]);

    assert_snapshot!(output.normalized_stdout());
    assert!(output.success());
}

#[test]
fn test_bash_env_succeeds() {
    let test = RvTest::new();
    let output = test.rv(&["shell", "env", "bash"]);

    assert_snapshot!(output.normalized_stdout());
    assert!(output.success());
}

#[test]
fn test_fish_env_succeeds() {
    let test = RvTest::new();
    let output = test.rv(&["shell", "env", "fish"]);

    assert_snapshot!(output.normalized_stdout());
    assert!(output.success());
}

#[test]
fn test_nushell_env_succeeds() {
    let test = RvTest::new();
    let output = test.rv(&["shell", "env", "nu"]);

    assert_snapshot!(output.normalized_stdout());
    assert!(output.success());
}

#[test]
fn test_shell_env_with_path() {
    let mut test = RvTest::new();
    test.env.insert("PATH".into(), "/tmp/bin".into());
    let output = test.rv(&["shell", "env", "zsh"]);

    assert_snapshot!(output.normalized_stdout());
    assert!(output.success());
}

#[test]
fn test_shell_env_clears_ruby_vars() {
    let mut test = RvTest::new();
    test.env.insert("PATH".into(), "/tmp/bin".into());
    test.env.insert("RUBY_ROOT".into(), "/tmp/ruby".into());
    test.env.insert("RUBY_ENGINE".into(), "ruby".into());
    test.env.insert("RUBY_VERSION".into(), "3.4.5".into());
    test.env.insert("RUBYOPT".into(), "--verbose".into());
    let output = test.rv(&["shell", "env", "zsh"]);

    assert_snapshot!(output.normalized_stdout());
    assert!(output.success());
}

#[test]
fn test_shell_env_clear_gem_vars() {
    let mut test = RvTest::new();
    test.env.insert("PATH".into(), "/tmp/bin".into());
    test.env.insert("GEM_ROOT".into(), "/tmp/ruby/gems".into());
    test.env.insert("GEM_HOME".into(), "/tmp/root/.gems".into());
    test.env.insert(
        "GEM_PATH".into(),
        "/tmp/root/.gems/bin:/tmp/ruby/gems".into(),
    );
    let output = test.rv(&["shell", "env", "zsh"]);

    assert_snapshot!(output.normalized_stdout());
    assert!(output.success());
}

#[test]
fn test_shell_env_with_ruby_and_xdg_compatible_gem_path() {
    let mut test = RvTest::new();
    test.create_ruby_dir("ruby-3.3.5");
    let temp_dir = tempdir().unwrap();
    let temp_dir_home = temp_dir.path().join("home");
    test.env
        .insert("HOME".into(), temp_dir_home.to_string_lossy().to_string());

    // Ensure the legacy path is not present.
    rm_rf(
        temp_dir_home
            .join(".gem")
            .join("ruby")
            .join("3.3.5")
            .to_str()
            .map(Utf8PathBuf::from)
            .unwrap(),
    )
    .unwrap();

    test.env.insert("PATH".into(), "/tmp/bin".into());
    test.env.insert("RUBY_ROOT".into(), "/tmp/ruby".into());
    test.env.insert("RUBY_ENGINE".into(), "ruby".into());
    test.env.insert("RUBY_VERSION".into(), "3.4.5".into());
    test.env.insert("RUBYOPT".into(), "--verbose".into());
    test.env.insert("GEM_ROOT".into(), "/tmp/ruby/gems".into());
    test.env.insert("GEM_HOME".into(), "/tmp/root/.gems".into());
    test.env.insert(
        "GEM_PATH".into(),
        "/tmp/root/.gems/bin:/tmp/ruby/gems".into(),
    );

    let output = test.rv(&["shell", "env", "zsh"]);
    output.assert_success();

    assert_snapshot!(
        output.normalized_stdout_with_temp_dir(temp_dir.path().to_string_lossy().to_string())
    );
}

#[test]
fn test_shell_env_with_ruby_and_legacy_gem_path() {
    let mut test = RvTest::new();
    test.create_ruby_dir("ruby-3.3.5");
    let temp_dir = tempdir().unwrap();
    let temp_dir_home = temp_dir.path().join("home");
    test.env
        .insert("HOME".into(), temp_dir_home.to_string_lossy().to_string());

    // Ensure the legacy path is present.
    create_dir_all(temp_dir_home.join(".gem").join("ruby").join("3.3.5")).unwrap();

    test.env.insert("PATH".into(), "/tmp/bin".into());
    test.env.insert("RUBY_ROOT".into(), "/tmp/ruby".into());
    test.env.insert("RUBY_ENGINE".into(), "ruby".into());
    test.env.insert("RUBY_VERSION".into(), "3.4.5".into());
    test.env.insert("RUBYOPT".into(), "--verbose".into());
    test.env.insert("GEM_ROOT".into(), "/tmp/ruby/gems".into());
    test.env.insert("GEM_HOME".into(), "/tmp/root/.gems".into());
    test.env.insert(
        "GEM_PATH".into(),
        "/tmp/root/.gems/bin:/tmp/ruby/gems".into(),
    );

    let output = test.rv(&["shell", "env", "zsh"]);
    output.assert_success();

    assert_snapshot!(
        output.normalized_stdout_with_temp_dir(temp_dir.path().to_string_lossy().to_string())
    );
}

#[test]
fn test_shell_env_with_existing_manpath() {
    let mut test = RvTest::new();
    test.create_ruby_dir("ruby-3.3.5");
    let temp_dir = tempdir().unwrap();
    let temp_dir_home = temp_dir.path().join("home");
    test.env
        .insert("HOME".into(), temp_dir_home.to_string_lossy().to_string());

    // Set existing MANPATH to test prepending behavior
    test.env.insert(
        "MANPATH".into(),
        "/usr/share/man:/usr/local/share/man".into(),
    );

    // Ensure the legacy path is present.
    create_dir_all(temp_dir_home.join(".gem").join("ruby").join("3.3.5")).unwrap();

    test.env.insert("PATH".into(), "/tmp/bin".into());

    let output = test.rv(&["shell", "env", "zsh"]);
    output.assert_success();

    assert_snapshot!(
        output.normalized_stdout_with_temp_dir(temp_dir.path().to_string_lossy().to_string())
    );
}
