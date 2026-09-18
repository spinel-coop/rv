//! Snippet checking: syntax validation and minitest assertion execution.

use std::path::PathBuf;

use tokio::io::AsyncWriteExt;

use rv_ruby::Ruby;

use crate::CheckError;

const MINITEST_REQUIRE: (&str, &str) = ("require \"minitest", "require 'minitest");

const MINITEST_FRAME: [&str; 3] = [
    "include Minitest::Assertions",
    "class << self; attr_accessor :assertions; end",
    "self.assertions = 0",
];

pub fn assertion_applies_to(code: &str) -> bool {
    code.contains(MINITEST_REQUIRE.0) || code.contains(MINITEST_REQUIRE.1)
}

fn inject_assertion_frame(code: &str) -> String {
    if !assertion_applies_to(code) {
        return code.to_string();
    }
    let mut out: Vec<&str> = Vec::new();
    let mut injected = false;
    for line in code.lines() {
        out.push(line);
        if !injected && (line.contains(MINITEST_REQUIRE.0) || line.contains(MINITEST_REQUIRE.1)) {
            out.extend(MINITEST_FRAME);
            injected = true;
        }
    }
    let mut joined = out.join("\n");
    if code.ends_with('\n') {
        joined.push('\n');
    }
    joined
}

pub(crate) enum RunMode {
    Syntax,
    Assertion,
}

pub async fn run_ruby(ruby: Ruby, code: &str, mode: RunMode) -> Result<(), CheckError> {
    let processed = match mode {
        RunMode::Syntax => code.to_string(),
        RunMode::Assertion => inject_assertion_frame(code),
    };

    let tmp_path: PathBuf = tempfile::Builder::new()
        .suffix(".rb")
        .tempfile()?
        .into_temp_path()
        .to_path_buf();

    {
        let mut file = tokio::fs::File::create(&tmp_path).await?;
        file.write_all(processed.as_bytes()).await?;
    }

    let mut cmd = tokio::process::Command::new(ruby.executable_path());
    match mode {
        RunMode::Syntax => {
            cmd.args(["-c"]).arg(&tmp_path);
        }
        RunMode::Assertion => {
            cmd.arg(&tmp_path);
        }
    }

    let output = cmd.output().await?;

    if output.status.success() {
        Ok(())
    } else {
        match mode {
            RunMode::Syntax => Err(CheckError::Syntax {
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            }),
            RunMode::Assertion => Err(CheckError::Assertion {
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            }),
        }
    }
}

pub async fn syntax_check(ruby: Ruby, code: &str) -> Result<(), CheckError> {
    run_ruby(ruby, code, RunMode::Syntax).await
}

pub async fn assertion_check(ruby: Ruby, code: &str) -> Result<(), CheckError> {
    run_ruby(ruby, code, RunMode::Assertion).await
}
