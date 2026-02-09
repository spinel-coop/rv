use crate::common::RvTest;
#[cfg(unix)]
use crate::common::Shell;
use insta::assert_snapshot;

#[cfg(unix)]
fn switch_rubies(shell: Shell) -> Result<(), Box<dyn std::error::Error>> {
    let test = RvTest::new();

    test.create_ruby_dir("3.3.4");
    test.create_ruby_dir("3.4.1");

    let mut session = test.make_session(shell)?;
    session.send_line(r"echo '3.3' > .ruby-version")?;
    session.wait_for_prompt()?;
    session.send_line("ruby")?;
    session.exp_string("ruby\r\n3.3.4\r\naarch64-darwin23\r\naarch64\r\ndarwin23")?;
    session.wait_for_prompt()?;
    session.send_line(r"echo '3.4' > .ruby-version")?;
    session.wait_for_prompt()?;
    session.send_line("ruby")?;
    session.exp_string("ruby\r\n3.4.1\r\naarch64-darwin23\r\naarch64\r\ndarwin23")?;
    session.wait_for_prompt()?;

    Ok(())
}

#[cfg(unix)]
#[test]
fn test_switch_rubies_bash() -> Result<(), Box<dyn std::error::Error>> {
    let shell = Shell {
        name: "bash",
        startup_flag: "--norc",
        prompt_setter: "PS1='PEXPECT>'",
    };

    switch_rubies(shell)
}

#[cfg(target_os = "linux")]
#[test]
fn test_switch_rubies_fish() -> Result<(), Box<dyn std::error::Error>> {
    let shell = Shell {
        name: "fish",
        startup_flag: "--no-config",
        prompt_setter: "function fish_prompt; echo 'PEXPECT>'; end",
    };

    switch_rubies(shell)
}

#[cfg(unix)]
#[test]
fn test_switch_rubies_zsh() -> Result<(), Box<dyn std::error::Error>> {
    let shell = Shell {
        name: "zsh",
        startup_flag: "--no-rcs",
        prompt_setter: "PROMPT='PEXPECT>'",
    };

    switch_rubies(shell)
}

#[test]
fn test_zsh_shell_init_succeeds() {
    let test = RvTest::new();
    let output = test.rv(&["shell", "init", "zsh"]);
    output.assert_success();

    assert_snapshot!(output.normalized_stdout());
}

#[test]
fn test_bash_shell_init_succeeds() {
    let test = RvTest::new();
    let output = test.rv(&["shell", "init", "bash"]);
    output.assert_success();

    assert_snapshot!(output.normalized_stdout());
}

#[test]
fn test_fish_shell_init_succeeds() {
    let test = RvTest::new();
    let output = test.rv(&["shell", "init", "fish"]);
    output.assert_success();

    assert_snapshot!(output.normalized_stdout());
}

#[test]
fn test_nu_shell_init_succeeds() {
    let test = RvTest::new();
    let output = test.rv(&["shell", "init", "nu"]);
    output.assert_success();

    assert_snapshot!(output.normalized_stdout());
}

#[test]
fn test_powershell_shell_init_succeeds() {
    let test = RvTest::new();
    let output = test.rv(&["shell", "init", "powershell"]);
    output.assert_success();

    assert_snapshot!(output.normalized_stdout());
}

#[test]
fn test_shell_init_fails_without_shell() {
    let test = RvTest::new();
    let output = test.rv(&["shell", "init"]);
    output.assert_failure();

    assert_eq!(output.normalized_stdout(), "");
}
