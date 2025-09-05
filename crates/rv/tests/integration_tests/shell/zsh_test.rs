use rexpect::{reader::Options, session::PtyReplSession};

use crate::common::RvTest;

fn make_session(test: &RvTest) -> Result<PtyReplSession, Box<dyn std::error::Error>> {
    let mut cmd = test.command("zsh");
    cmd.arg("--no-rcs").arg("--login").arg("--interactive");
    cmd.env_remove("RV_TEST_EXE").env("PROMPT", "PEXPECT>");
    let pty_session = rexpect::spawn_with_options(
        cmd,
        Options {
            timeout_ms: Some(4000),
            strip_ansi_escape_codes: true,
        },
    )?;
    let mut session = PtyReplSession {
        prompt: "PEXPECT>".to_owned(),
        pty_session,
        quit_command: Some("builtin exit".to_owned()),
        echo_on: true,
    };

    session.send_line(&format!(
        "eval \"$({} shell init zsh)\"",
        test.rv_command().get_program().display()
    ))?;
    session.wait_for_prompt()?;

    Ok(session)
}

#[test]
fn test_switching_rubies() -> Result<(), Box<dyn std::error::Error>> {
    let test = RvTest::new();
    test.create_ruby_dir("3.3.4");
    test.create_ruby_dir("3.4.1");
    let subdir = test.temp_dir.path().join("foobartest");
    std::fs::create_dir_all(&subdir).expect("Failed to create ruby directory");
    std::fs::write(subdir.join(".ruby-version"), b"3.3").unwrap();

    let mut session = make_session(&test)?;
    session.send_line("cd foobartest")?;
    session.wait_for_prompt()?;
    session.send_line("ruby")?;
    session.exp_string("ruby\r\n3.3.4\r\naarch64-darwin23\r\naarch64\r\ndarwin23")?;
    session.wait_for_prompt()?;
    session.send_line("cd ..")?;
    session.wait_for_prompt()?;
    session.send_line("ruby")?;
    session.exp_string("ruby\r\n3.4.1\r\naarch64-darwin23\r\naarch64\r\ndarwin23")?;
    session.wait_for_prompt()?;

    Ok(())
}
