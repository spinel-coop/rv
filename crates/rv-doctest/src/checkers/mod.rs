//! Snippet checking: syntax validation and minitest assertion execution.

use tokio::io::AsyncWriteExt;

use rv_ruby::Ruby;
use rv_ruby_parser::calls::{MinitestRequire, minitest_require};

use crate::CheckError;

const MINITEST_FRAME: [&str; 3] = [
    "include Minitest::Assertions",
    "class << self; attr_accessor :assertions; end",
    "self.assertions = 0",
];

/// `true` when `code` is an assertion example rather than a plain snippet.
///
/// Parses `code`; prefer [`crate::SnippetAnalysis::is_assertion`] when the
/// snippet has already been analyzed.
pub fn assertion_applies_to(code: &str) -> bool {
    minitest_require(code.as_bytes()).is_some()
}

/// Insert the assertion preamble into `code`, given where its minitest
/// `require` was found.
fn inject_assertion_frame(code: &str, found: MinitestRequire) -> String {
    let mut lines: Vec<&str> = code.lines().collect();
    // Insert after the whole top-level statement that loads minitest, not
    // after the `require` line itself: a `require` nested in a `class` or
    // `def` body would otherwise put the frame in that scope, so
    // `include Minitest::Assertions` would apply there instead of to `main`
    // and a top-level `assert_*` would raise `NoMethodError`.
    let insert_at = found.top_level_end_line.min(lines.len());

    if insert_at > 0 {
        for (offset, frame_line) in MINITEST_FRAME.iter().enumerate() {
            lines.insert(insert_at + offset, frame_line);
        }
    }

    let mut result = lines.join("\n");
    if code.ends_with('\n') {
        result.push('\n');
    }
    result
}

pub(crate) enum RunMode {
    Syntax,
    /// Run the snippet, injecting the assertion preamble after this `require`.
    Assertion(MinitestRequire),
}

pub async fn run_ruby(ruby: Ruby, code: &str, mode: RunMode) -> Result<(), CheckError> {
    let processed = match mode {
        RunMode::Syntax => code.to_string(),
        RunMode::Assertion(found) => inject_assertion_frame(code, found),
    };

    // Split the temp file into its open handle and its `TempPath` guard, and
    // hold the guard until `ruby` has exited. Taking only the path would drop
    // the guard here, deleting the file and leaving whatever we wrote next
    // unowned and never cleaned up.
    let (std_file, tmp_path) = tempfile::Builder::new()
        .suffix(".rb")
        .tempfile()?
        .into_parts();

    {
        let mut file = tokio::fs::File::from_std(std_file);
        file.write_all(processed.as_bytes()).await?;
        // `tokio::fs::File` buffers, so flush before `ruby` reads the path.
        file.flush().await?;
    }

    let mut cmd = tokio::process::Command::new(ruby.executable_path());
    match mode {
        RunMode::Syntax => {
            cmd.args(["-c"]).arg(tmp_path.as_os_str());
        }
        RunMode::Assertion(_) => {
            cmd.arg(tmp_path.as_os_str());
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
            RunMode::Assertion(_) => Err(CheckError::Assertion {
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            }),
        }
    }
}

pub async fn syntax_check(ruby: Ruby, code: &str) -> Result<(), CheckError> {
    run_ruby(ruby, code, RunMode::Syntax).await
}

pub async fn assertion_check(
    ruby: Ruby,
    code: &str,
    found: MinitestRequire,
) -> Result<(), CheckError> {
    run_ruby(ruby, code, RunMode::Assertion(found)).await
}

pub mod rbs;

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;

    /// Inject the frame the way `check_snippets` does: locate the require,
    /// then hand it to the injector.
    fn inject(code: &str) -> String {
        match minitest_require(code.as_bytes()) {
            Some(found) => inject_assertion_frame(code, found),
            None => code.to_string(),
        }
    }

    /// Assert that injecting the frame into `input` yields `expected`.
    fn assert_injected(input: &str, expected: &str) {
        assert_eq!(inject(input), expected);
    }

    #[test]
    fn frame_follows_a_top_level_require() {
        assert_injected(
            indoc! {"
                require \"minitest/autorun\"
                assert_equal 1, 1
            "},
            indoc! {"
                require \"minitest/autorun\"
                include Minitest::Assertions
                class << self; attr_accessor :assertions; end
                self.assertions = 0
                assert_equal 1, 1
            "},
        );
    }

    #[test]
    fn frame_stays_at_the_top_level_when_the_require_is_nested() {
        // Injecting after the `require` line itself would put the frame inside
        // `class Helper`, so `include Minitest::Assertions` would apply to the
        // class and a top-level `assert_equal` would raise `NoMethodError`.
        assert_injected(
            indoc! {"
                class Helper
                  require \"minitest/autorun\"
                end
                assert_equal 1, 1
            "},
            indoc! {"
                class Helper
                  require \"minitest/autorun\"
                end
                include Minitest::Assertions
                class << self; attr_accessor :assertions; end
                self.assertions = 0
                assert_equal 1, 1
            "},
        );
    }

    #[test]
    fn a_require_inside_a_def_body_is_not_an_assertion_snippet() {
        // The require has not run when the `def` statement ends, so there is
        // no safe place for the frame; the snippet is syntax-checked instead.
        let code = indoc! {"
            def setup_env
              require \"minitest/autorun\"
            end
            setup_env
        "};

        assert!(!assertion_applies_to(code));
        assert_eq!(inject(code), code);
    }

    #[test]
    fn code_without_minitest_is_left_alone() {
        let code = "1 + 1\n";
        assert!(!assertion_applies_to(code));
        assert_eq!(inject(code), code);
    }
}
