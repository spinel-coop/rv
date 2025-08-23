use crate::common::RvTest;
use insta::assert_snapshot;

#[test]
fn test_shell_env_succeeds() {
    let test = RvTest::new();
    let output = test.rv(&["shell", "env", "zsh"]);

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
fn test_shell_env_with_ruby() {
    let mut test = RvTest::new();
    test.create_ruby_dir("ruby-3.3.5");

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

    assert_snapshot!(output.normalized_stdout());
}
