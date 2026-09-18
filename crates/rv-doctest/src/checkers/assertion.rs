use async_trait::async_trait;
use rv_ruby::Ruby;
use std::path::PathBuf;

use crate::{CheckError, RubyChecker};

/// Minitest assertion frame injected after the `require "minitest"` line.
/// Provides the `Minitest::Assertions` mixin and the assertion counter
/// that `Minitest::Assertions#assert` requires on the receiver.
const MINITEST_FRAME: [&str; 3] = [
    "include Minitest::Assertions",
    "class << self; attr_accessor :assertions; end",
    "self.assertions = 0",
];

pub struct AssertionChecker {
    ruby: Ruby,
}

impl AssertionChecker {
    pub fn new(ruby: Ruby) -> Self {
        Self { ruby }
    }

    /// Returns true if the snippet opts into execution by requiring minitest.
    pub fn applies_to(code: &str) -> bool {
        code.contains("require \"minitest") || code.contains("require 'minitest")
    }
}

fn is_minitest_require(line: &str) -> bool {
    line.contains("require \"minitest") || line.contains("require 'minitest")
}

/// Inserts the minitest assertion frame immediately after the first
/// `require "minitest..."` line. If no such line exists, returns the
/// code unchanged.
fn inject_assertion_frame(code: &str) -> String {
    if !(code.contains("require \"minitest") || code.contains("require 'minitest")) {
        return code.to_string();
    }
    let mut out: Vec<&str> = Vec::new();
    let mut injected = false;
    for line in code.lines() {
        out.push(line);
        if !injected && is_minitest_require(line) {
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

#[async_trait]
impl RubyChecker for AssertionChecker {
    async fn check(&mut self, code: &str) -> Result<(), CheckError> {
        use tokio::io::AsyncWriteExt;

        let tmp_path: PathBuf = tempfile::Builder::new()
            .suffix(".rb")
            .tempfile()?
            .into_temp_path()
            .to_path_buf();

        {
            let mut file = tokio::fs::File::create(&tmp_path).await?;
            file.write_all(inject_assertion_frame(code).as_bytes())
                .await?;
        }

        let output = tokio::process::Command::new(self.ruby.executable_path())
            .arg(&tmp_path)
            .output()
            .await?;

        if output.status.success() {
            Ok(())
        } else {
            Err(CheckError::Assertion {
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injection_places_frame_after_require() {
        let code = "require \"minitest\"\nassert_equal 2, 1 + 1\n";
        let injected = inject_assertion_frame(code);
        let lines: Vec<&str> = injected.lines().collect();
        assert_eq!(lines[0], "require \"minitest\"");
        assert_eq!(lines[1], "include Minitest::Assertions");
        assert_eq!(lines[2], "class << self; attr_accessor :assertions; end");
        assert_eq!(lines[3], "self.assertions = 0");
        assert_eq!(lines[4], "assert_equal 2, 1 + 1");
    }

    #[test]
    fn injection_ignored_without_require() {
        let code = "assert_equal 2, 1 + 1\n";
        assert_eq!(inject_assertion_frame(code), code);
    }

    #[test]
    fn injection_after_late_require() {
        let code = "# comment\nrequire \"minitest\"\nassert_equal 2, 1 + 1\n";
        let injected = inject_assertion_frame(code);
        let lines: Vec<&str> = injected.lines().collect();
        assert_eq!(lines[0], "# comment");
        assert_eq!(lines[1], "require \"minitest\"");
        assert_eq!(lines[2], "include Minitest::Assertions");
        assert_eq!(lines[5], "assert_equal 2, 1 + 1");
    }

    #[test]
    fn injection_matches_single_quoted_require() {
        let code = "require 'minitest'\nassert_equal 2, 1 + 1\n";
        let injected = inject_assertion_frame(code);
        let lines: Vec<&str> = injected.lines().collect();
        assert_eq!(lines[1], "include Minitest::Assertions");
    }

    #[test]
    fn injection_only_happens_once() {
        let code = "require \"minitest\"\nrequire \"minitest/autorun\"\n";
        let injected = inject_assertion_frame(code);
        assert_eq!(
            injected.matches("include Minitest::Assertions").count(),
            1
        );
    }
}
