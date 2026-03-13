use std::fs::create_dir_all;

use crate::common::RvTest;
use insta::assert_snapshot;
use rv_cache::rm_rf;

#[test]
fn test_zsh_succeeds() {
    let test = RvTest::new();
    let output = test.rv(&["shell", "env", "zsh"]);

    assert_snapshot!(output.normalized_stdout());
    output.assert_success();
}

#[test]
fn test_bash_succeeds() {
    let test = RvTest::new();
    let output = test.rv(&["shell", "env", "bash"]);

    assert_snapshot!(output.normalized_stdout());
    output.assert_success();
}

#[test]
fn test_fish_succeeds() {
    let test = RvTest::new();
    let output = test.rv(&["shell", "env", "fish"]);

    assert_snapshot!(output.normalized_stdout());
    output.assert_success();
}

#[test]
fn test_nu_succeeds() {
    let test = RvTest::new();
    let output = test.rv(&["shell", "env", "nu"]);

    assert_snapshot!(output.normalized_stdout());
    output.assert_success();
}

#[test]
fn test_powershell_succeeds() {
    let test = RvTest::new();
    let output = test.rv(&["shell", "env", "powershell"]);

    assert_snapshot!(output.normalized_stdout());
    output.assert_success();
}

#[test]
fn test_shell_env_with_path() {
    let mut test = RvTest::new();
    test.env.insert("PATH".into(), "/tmp/bin".into());

    // No rubies available, PATH unchanged
    let output = test.rv(&["shell", "env", "zsh"]);
    output.assert_success();
    output.assert_stdout_contains("export PATH=/tmp/bin");

    // With rubies available
    test.create_ruby_dir("ruby-3.3.5");

    // Prepends ruby dirs to PATH
    let expected_path = [
        "/tmp/home/.local/share/rv/gems/ruby/3.3.0/bin",
        "/tmp/home/.local/share/rv/rubies/ruby-3.3.5/lib/ruby/gems/3.3.0/bin",
        "/tmp/home/.local/share/rv/rubies/ruby-3.3.5/bin",
        "/tmp/bin",
    ]
    .join(":");
    let output = test.rv(&["shell", "env", "zsh"]);
    output.assert_success();
    output.assert_stdout_contains(&format!("export PATH='{expected_path}'"));

    // But does not duplicate entries the second time
    let data_dir = test.data_dir();
    let rubies_dir = test.rubies_dir();
    let new_path = std::env::join_paths([
        format!("{data_dir}/rv/gems/ruby/3.3.0/bin"),
        format!("{rubies_dir}/ruby-3.3.5/lib/ruby/gems/3.3.0/bin"),
        format!("{rubies_dir}/ruby-3.3.5/bin"),
        "/tmp/bin".to_string(),
    ])
    .unwrap()
    .into_string()
    .unwrap();
    test.env.insert("PATH".into(), new_path);
    let output = test.rv(&["shell", "env", "zsh"]);
    output.assert_success();
    output.assert_stdout_contains(&format!("export PATH='{expected_path}'"));
}

#[test]
fn test_shell_env_clears_ruby_and_gem_vars_when_no_rubies_available() {
    let mut test = RvTest::new();
    test.env.insert("PATH".into(), "/tmp/bin".into());
    test.env.insert("RUBY_ROOT".into(), "/tmp/ruby".into());
    test.env.insert("RUBY_ENGINE".into(), "ruby".into());
    test.env.insert("RUBY_VERSION".into(), "3.4.5".into());
    test.env.insert("RUBYOPT".into(), "--verbose".into());
    test.env.insert("GEM_HOME".into(), "/tmp/root/.gems".into());
    test.env.insert(
        "GEM_PATH".into(),
        "/tmp/root/.gems/bin:/tmp/ruby/gems".into(),
    );
    let output = test.rv(&["shell", "env", "zsh"]);

    assert_snapshot!(output.normalized_stdout());
    output.assert_success();
}

#[test]
fn test_shell_env_fallback_to_highest_installed_ruby_if_no_rubies_matching_pin_installed() {
    let mut test = RvTest::new();
    test.env.insert("PATH".into(), "/tmp/bin".into());
    test.create_ruby_dir("ruby-4.0.1");

    let project_dir = test.temp_root().join("project");
    std::fs::create_dir_all(project_dir.as_path()).unwrap();
    std::fs::write(project_dir.join(".ruby-version"), b"3.4.8").unwrap();
    test.cwd = project_dir;

    let expected_path = [
        "/tmp/home/.local/share/rv/gems/ruby/4.0.0/bin",
        "/tmp/home/.local/share/rv/rubies/ruby-4.0.1/lib/ruby/gems/4.0.0/bin",
        "/tmp/home/.local/share/rv/rubies/ruby-4.0.1/bin",
        "/tmp/bin",
    ]
    .join(":");
    let output = test.rv(&["shell", "env", "zsh"]);
    output.assert_success();
    output.assert_stdout_contains(&format!("export PATH='{expected_path}'"));
}

#[test]
fn test_shell_env_pinned_to_dev() {
    let mut test = RvTest::new();
    test.env.insert("PATH".into(), "/tmp/bin".into());
    test.create_ruby_dir("ruby-4.1.0-dev");

    let project_dir = test.temp_root().join("project");
    std::fs::create_dir_all(project_dir.as_path()).unwrap();
    std::fs::write(project_dir.join(".ruby-version"), b"4.1.0-dev").unwrap();
    test.cwd = project_dir;

    let expected_path = [
        "/tmp/home/.local/share/rv/gems/ruby/4.1.0/bin",
        "/tmp/home/.local/share/rv/rubies/ruby-dev/lib/ruby/gems/4.1.0/bin",
        "/tmp/home/.local/share/rv/rubies/ruby-dev/bin",
        "/tmp/bin",
    ]
    .join(":");
    let output = test.rv(&["shell", "env", "zsh"]);
    output.assert_success();
    output.assert_stdout_contains(&format!("export PATH='{expected_path}'"));
}

// MANPATH is a Unix concept — on Windows, the #[cfg(not(windows))] guard in config.rs
// means MANPATH is never exported. These tests use dual inline snapshots so the Ruby
// env setup (RUBY_ROOT, GEM_HOME, PATH, etc.) is verified on ALL platforms.
#[test]
fn test_shell_env_with_ruby_and_xdg_compatible_gem_path() {
    let mut test = RvTest::new();
    test.create_ruby_dir("ruby-3.3.5");

    // Ensure the legacy path is not present.
    rm_rf(test.legacy_gem_path("3.3")).unwrap();

    test.env.insert("PATH".into(), "/tmp/bin".into());
    test.env.insert("RUBY_ROOT".into(), "/tmp/ruby".into());
    test.env.insert("RUBY_ENGINE".into(), "ruby".into());
    test.env.insert("RUBY_VERSION".into(), "3.4.5".into());
    test.env.insert("RUBYOPT".into(), "--verbose".into());
    test.env.insert("GEM_HOME".into(), "/tmp/root/.gems".into());
    test.env.insert(
        "GEM_PATH".into(),
        "/tmp/root/.gems/bin:/tmp/ruby/gems".into(),
    );

    let output = test.rv(&["shell", "env", "zsh"]);
    output.assert_success();

    let stdout = output.normalized_stdout();

    #[cfg(unix)]
    assert_snapshot!(stdout, @r"
    unset RUBYOPT
    export RUBY_ROOT=/tmp/home/.local/share/rv/rubies/ruby-3.3.5
    export RUBY_ENGINE=ruby
    export RUBY_VERSION=3.3.5
    export GEM_HOME=/tmp/home/.local/share/rv/rubies/ruby-3.3.5/lib/ruby/gems/3.3.0
    export GEM_PATH='/tmp/home/.local/share/rv/gems/ruby/3.3.0:/tmp/home/.local/share/rv/rubies/ruby-3.3.5/lib/ruby/gems/3.3.0'
    export MANPATH='/tmp/home/.local/share/rv/rubies/ruby-3.3.5/share/man:'
    export PATH='/tmp/home/.local/share/rv/gems/ruby/3.3.0/bin:/tmp/home/.local/share/rv/rubies/ruby-3.3.5/lib/ruby/gems/3.3.0/bin:/tmp/home/.local/share/rv/rubies/ruby-3.3.5/bin:/tmp/bin'
    hash -r
    ");

    // On Windows, RUBY_ROOT / GEM_HOME / GEM_PATH get single-quoted because the
    // pre-normalization Windows path contains `C:` — the colon triggers quoting by
    // shell_escape::unix::escape(). After test-root replacement the colon is gone
    // but the quotes remain. This is semantically correct for bash.
    #[cfg(windows)]
    assert_snapshot!(stdout, @r"
    unset RUBYOPT
    export RUBY_ROOT='/tmp/home/.local/share/rv/rubies/ruby-3.3.5'
    export RUBY_ENGINE=ruby
    export RUBY_VERSION=3.3.5
    export GEM_HOME='/tmp/home/.local/share/rv/rubies/ruby-3.3.5/lib/ruby/gems/3.3.0'
    export GEM_PATH='/tmp/home/.local/share/rv/gems/ruby/3.3.0:/tmp/home/.local/share/rv/rubies/ruby-3.3.5/lib/ruby/gems/3.3.0'
    export PATH='/tmp/home/.local/share/rv/gems/ruby/3.3.0/bin:/tmp/home/.local/share/rv/rubies/ruby-3.3.5/lib/ruby/gems/3.3.0/bin:/tmp/home/.local/share/rv/rubies/ruby-3.3.5/bin:/tmp/bin'
    hash -r
    ");
}

#[test]
fn test_shell_env_with_ruby_and_legacy_gem_path() {
    let mut test = RvTest::new();
    test.create_ruby_dir("ruby-3.3.5");

    // Ensure the legacy path is present.
    create_dir_all(test.legacy_gem_path("3.3")).unwrap();

    test.env.insert("PATH".into(), "/tmp/bin".into());
    test.env.insert("RUBY_ROOT".into(), "/tmp/ruby".into());
    test.env.insert("RUBY_ENGINE".into(), "ruby".into());
    test.env.insert("RUBY_VERSION".into(), "3.4.5".into());
    test.env.insert("RUBYOPT".into(), "--verbose".into());
    test.env.insert("GEM_HOME".into(), "/tmp/root/.gems".into());
    test.env.insert(
        "GEM_PATH".into(),
        "/tmp/root/.gems/bin:/tmp/ruby/gems".into(),
    );

    let output = test.rv(&["shell", "env", "zsh"]);
    output.assert_success();

    let stdout = output.normalized_stdout();

    #[cfg(unix)]
    assert_snapshot!(stdout, @r"
    unset RUBYOPT
    export RUBY_ROOT=/tmp/home/.local/share/rv/rubies/ruby-3.3.5
    export RUBY_ENGINE=ruby
    export RUBY_VERSION=3.3.5
    export GEM_HOME=/tmp/home/.local/share/rv/rubies/ruby-3.3.5/lib/ruby/gems/3.3.0
    export GEM_PATH='/tmp/home/.gem/ruby/3.3.0:/tmp/home/.local/share/rv/rubies/ruby-3.3.5/lib/ruby/gems/3.3.0'
    export MANPATH='/tmp/home/.local/share/rv/rubies/ruby-3.3.5/share/man:'
    export PATH='/tmp/home/.gem/ruby/3.3.0/bin:/tmp/home/.local/share/rv/rubies/ruby-3.3.5/lib/ruby/gems/3.3.0/bin:/tmp/home/.local/share/rv/rubies/ruby-3.3.5/bin:/tmp/bin'
    hash -r
    ");

    #[cfg(windows)]
    assert_snapshot!(stdout, @r"
    unset RUBYOPT
    export RUBY_ROOT='/tmp/home/.local/share/rv/rubies/ruby-3.3.5'
    export RUBY_ENGINE=ruby
    export RUBY_VERSION=3.3.5
    export GEM_HOME='/tmp/home/.local/share/rv/rubies/ruby-3.3.5/lib/ruby/gems/3.3.0'
    export GEM_PATH='/tmp/home/.gem/ruby/3.3.0:/tmp/home/.local/share/rv/rubies/ruby-3.3.5/lib/ruby/gems/3.3.0'
    export PATH='/tmp/home/.gem/ruby/3.3.0/bin:/tmp/home/.local/share/rv/rubies/ruby-3.3.5/lib/ruby/gems/3.3.0/bin:/tmp/home/.local/share/rv/rubies/ruby-3.3.5/bin:/tmp/bin'
    hash -r
    ");
}

#[test]
fn test_powershell_env_with_ruby() {
    let mut test = RvTest::new();
    test.create_ruby_dir("ruby-3.3.5");

    // Ensure the legacy path is present.
    create_dir_all(test.legacy_gem_path("3.3")).unwrap();

    test.env.insert("PATH".into(), "/tmp/bin".into());

    let output = test.rv(&["shell", "env", "powershell"]);
    output.assert_success();

    let stdout = output.normalized_stdout();

    #[cfg(unix)]
    assert_snapshot!(stdout, @r#"
    Remove-Item Env:\RUBYOPT -ErrorAction SilentlyContinue
    $env:RUBY_ROOT = "/tmp/home/.local/share/rv/rubies/ruby-3.3.5"
    $env:RUBY_ENGINE = "ruby"
    $env:RUBY_VERSION = "3.3.5"
    $env:GEM_HOME = "/tmp/home/.local/share/rv/rubies/ruby-3.3.5/lib/ruby/gems/3.3.0"
    $env:GEM_PATH = "/tmp/home/.gem/ruby/3.3.0:/tmp/home/.local/share/rv/rubies/ruby-3.3.5/lib/ruby/gems/3.3.0"
    $env:MANPATH = "/tmp/home/.local/share/rv/rubies/ruby-3.3.5/share/man:"
    $env:PATH = "/tmp/home/.gem/ruby/3.3.0/bin:/tmp/home/.local/share/rv/rubies/ruby-3.3.5/lib/ruby/gems/3.3.0/bin:/tmp/home/.local/share/rv/rubies/ruby-3.3.5/bin:/tmp/bin"
    "#);

    #[cfg(windows)]
    assert_snapshot!(stdout, @r#"
    Remove-Item Env:\RUBYOPT -ErrorAction SilentlyContinue
    $env:RUBY_ROOT = "/tmp/home/.local/share/rv/rubies/ruby-3.3.5"
    $env:RUBY_ENGINE = "ruby"
    $env:RUBY_VERSION = "3.3.5"
    $env:GEM_HOME = "/tmp/home/.local/share/rv/rubies/ruby-3.3.5/lib/ruby/gems/3.3.0"
    $env:GEM_PATH = "/tmp/home/.gem/ruby/3.3.0;/tmp/home/.local/share/rv/rubies/ruby-3.3.5/lib/ruby/gems/3.3.0"
    $env:PATH = "/tmp/home/.gem/ruby/3.3.0/bin;/tmp/home/.local/share/rv/rubies/ruby-3.3.5/lib/ruby/gems/3.3.0/bin;/tmp/home/.local/share/rv/rubies/ruby-3.3.5/bin;/tmp/bin"
    "#);
}

#[cfg(unix)]
#[test]
fn test_shell_env_with_existing_manpath() {
    let mut test = RvTest::new();
    test.create_ruby_dir("ruby-3.3.5");

    // Set existing MANPATH to test prepending behavior
    test.env.insert(
        "MANPATH".into(),
        "/usr/share/man:/usr/local/share/man".into(),
    );

    let output = test.rv(&["shell", "env", "zsh"]);
    output.assert_success();

    output.assert_stdout_contains("export MANPATH='/tmp/home/.local/share/rv/rubies/ruby-3.3.5/share/man:/usr/share/man:/usr/local/share/man'");

    // Check it's not duplicated when rv ruby already in MANPATH
    test.env.insert(
        "MANPATH".into(),
        format!(
            "{}/ruby-3.3.5/share/man:/usr/share/man:/usr/local/share/man",
            test.rubies_dir()
        ),
    );

    let output = test.rv(&["shell", "env", "zsh"]);
    output.assert_success();

    assert!(
        !output.normalized_stdout().contains("export MANPATH"),
        "MANPATH should not require modifications if already set",
    )
}
