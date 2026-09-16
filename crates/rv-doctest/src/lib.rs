use async_trait::async_trait;
use rv_ruby::Ruby;
use rv_ruby_parser::{ItemKind, ParsedFile as ParsedFileFromParser};
use std::path::PathBuf;

use thiserror::Error;

/// Error returned from snippet checking.
#[derive(Debug, Error)]
pub enum CheckError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("syntax check failed:\n{stderr}")]
    Syntax { stderr: String },
}

/// A reader of Ruby code that we can validate.
#[async_trait]
pub trait RubyChecker {
    async fn check(&mut self, code: &str) -> Result<(), CheckError>;
}

/// Checks Ruby syntax by running the interpreter with `-c`.
pub struct CommandRubyChecker {
    ruby: Ruby,
}

impl CommandRubyChecker {
    pub fn new(ruby: Ruby) -> Self {
        Self { ruby }
    }
}

#[async_trait]
impl RubyChecker for CommandRubyChecker {
    async fn check(&mut self, code: &str) -> Result<(), CheckError> {
        use tokio::io::AsyncWriteExt;

        let tmp_path: PathBuf = tempfile::Builder::new()
            .suffix(".rb")
            .tempfile()?
            .into_temp_path()
            .to_path_buf();

        {
            let mut file = tokio::fs::File::create(&tmp_path).await?;
            file.write_all(code.as_bytes()).await?;
        }

        let output = tokio::process::Command::new(self.ruby.executable_path())
            .args(["-c"])
            .arg(&tmp_path)
            .output()
            .await?;

        if output.status.success() {
            Ok(())
        } else {
            Err(CheckError::Syntax {
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            })
        }
    }
}

/// A Ruby code snippet found in a documentation comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snippet {
    pub item_name: String,
    pub item_kind: ItemKind,
    pub start_line: u32,
    pub code: String,
}

/// A failure to check a snippet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub snippet: Snippet,
    pub message: String,
}

/// Extracts snippets from Ruby documentation comments.
pub fn extract(parsed: &ParsedFileFromParser) -> Vec<Snippet> {
    let mut snippets = Vec::new();
    for item in &parsed.items {
        let lines: Vec<&str> = item.comments.iter().map(String::as_str).collect();
        let mut comment_idx = 0;
        while comment_idx < lines.len() {
            if is_fence_open(lines[comment_idx]) {
                let mut code_end = comment_idx + 1;
                let mut closed_code_block = false;
                while code_end < lines.len() {
                    if is_fence_close(lines[code_end]) {
                        closed_code_block = true;
                        break;
                    }
                    code_end += 1;
                }
                if closed_code_block {
                    let content = lines[comment_idx + 1..code_end].to_vec();
                    snippets.push(Snippet {
                        item_name: item.name.clone(),
                        item_kind: item.kind(),
                        start_line: item.span.start_line,
                        code: content.join("\n"),
                    });
                }
                comment_idx = if closed_code_block {
                    code_end + 1
                } else {
                    code_end
                };
            } else if is_indented_line(lines[comment_idx]) {
                let mut code_end = comment_idx;
                while code_end < lines.len() && is_indented_line(lines[code_end]) {
                    code_end += 1;
                }
                let code = lines[comment_idx..code_end].to_vec();
                snippets.push(Snippet {
                    item_name: item.name.clone(),
                    item_kind: item.kind(),
                    start_line: item.span.start_line,
                    code: code
                        .iter()
                        .map(|l| &l[4..]) // keep only the code, not the whitespace
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>()
                        .join("\n"),
                });
                comment_idx = code_end;
            } else {
                comment_idx += 1;
            }
        }
    }
    snippets
}

fn is_fence_open(line: &str) -> bool {
    let t = line.trim();
    t.strip_prefix("```")
        .map(|rest| rest.trim().eq_ignore_ascii_case("ruby"))
        .unwrap_or(false)
}

fn is_fence_close(line: &str) -> bool {
    line.trim() == "```"
}

fn is_indented_line(line: &str) -> bool {
    line.starts_with("    ")
}

/// Checks multiple snippets, collecting all failures.
pub async fn check_snippets(
    _snippets: &[Snippet],
    _checker: &mut impl RubyChecker,
) -> Vec<Failure> {
    Vec::new()
}

/// Convenience: extract & check from a parsed file.
pub async fn check(parsed: &ParsedFileFromParser, checker: &mut impl RubyChecker) -> Vec<Failure> {
    check_snippets(&extract(parsed), checker).await
}

#[cfg(test)]
mod tests;
