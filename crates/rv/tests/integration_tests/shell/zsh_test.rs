use rexpect::{reader::Options, session::PtyReplSession};

use crate::common::RvTest;

fn make_session(test: &RvTest) -> Result<PtyReplSession, Box<dyn std::error::Error>> {
    let mut cmd = test.command("zsh");
    cmd.arg("--no-rcs").arg("--login").arg("--interactive");
    cmd.env("TERM", "xterm-256color")
        .env("COLUMNS", "1000")
        .env_remove("RV_TEST_EXE")
        .env("HOST", ">>>")
        .env("PATH", "/bin");
    let pty_session = rexpect::spawn_with_options(
        cmd,
        Options {
            timeout_ms: Some(1000),
            strip_ansi_escape_codes: true,
        },
    )?;
    let mut session = PtyReplSession {
        prompt: "%                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       \r \r\r[PEXPECT]$ ".to_owned(),
        pty_session,
        quit_command: Some("builtin exit".to_owned()),
        echo_on: true,
    };

    session.send_line("prompt restore")?;
    session.send_line(r"PS1='[PEXPECT]%(!.#.$) '")?;
    session.send_line(r"unset RPROMPT")?;
    session.send_line(r"unset PROMPT_COMMAND")?;

    session.wait_for_prompt()?;
    session.send_line(&format!(
        "eval \"$({} shell init zsh)\"",
        test.rv_command().get_program().display()
    ))?;
    assert_eq!(session.wait_for_prompt()?.trim(), "");

    Ok(session)
}

#[test]
fn test_no_rubies() -> Result<(), Box<dyn std::error::Error>> {
    let test = RvTest::new();
    let mut session = make_session(&test)?;
    session.send_line("mkdir foobartest")?;
    session.wait_for_prompt()?;
    session.send_line("cd foobartest")?;
    assert_eq!(session.wait_for_prompt()?.trim(), "");
    session.send_line("cd ..")?;
    session.send_line(r"echo '3.4' > foobartest/.ruby-version")?;
    session.send_line("cd foobartest")?;
    assert_eq!(session.wait_for_prompt()?.trim(), "");
    session.send_line(&format!(
        "{} ruby pin",
        test.rv_command().get_program().display()
    ))?;
    session.exp_string("/foobartest is pinned to Ruby 3.4")?;
    session.wait_for_prompt()?;
    session.send_line("cd ..")?;
    assert_eq!(session.wait_for_prompt()?.trim(), "");

    Ok(())
}

#[test]
fn test_rubies() -> Result<(), Box<dyn std::error::Error>> {
    let test = RvTest::new();
    test.create_ruby_dir("3.3.4");
    test.create_ruby_dir("3.4.1");
    let mut session = make_session(&test)?;
    session.send_line("mkdir foobartest")?;
    session.wait_for_prompt()?;
    session.send_line("cd foobartest")?;
    assert_eq!(session.wait_for_prompt()?.trim(), "");
    session.send_line("cd ..")?;
    session.send_line(r"echo '3.3' > foobartest/.ruby-version")?;
    session.send_line("cd foobartest")?;
    assert_eq!(session.wait_for_prompt()?.trim(), "");
    session.send_line(&format!(
        "{} ruby pin",
        test.rv_command().get_program().display()
    ))?;
    session.exp_string("/foobartest is pinned to Ruby 3.3")?;
    session.wait_for_prompt()?;
    session.send_line("ruby")?;
    assert_eq!(
        session.wait_for_prompt()?.trim(),
        "ruby\r\n3.3.4\r\naarch64-darwin23\r\naarch64\r\ndarwin23"
    );
    session.send_line("cd ..")?;
    assert_eq!(session.wait_for_prompt()?.trim(), "");
    session.send_line("ruby")?;
    assert_eq!(
        session.wait_for_prompt()?.trim(),
        "ruby\r\n3.4.1\r\naarch64-darwin23\r\naarch64\r\ndarwin23"
    );

    Ok(())
}
