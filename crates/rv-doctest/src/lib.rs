pub mod checker;
mod checkers;
pub mod rbs;

use async_trait::async_trait;
use rv_ruby_parser::{ItemKind, ParsedFile as ParsedFileFromParser};

use thiserror::Error;

pub use checker::{CheckReport, CheckStats, RbsChecker, RbsViolation, UnknownEntry};
pub use checkers::{AssertionChecker, SyntaxChecker};
pub use rbs::{ArityResult, MethodSig, RbsEnvironment};

/// Error returned from snippet checking.
#[derive(Debug, Error)]
pub enum CheckError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("syntax check failed:\n{stderr}")]
    Syntax { stderr: String },
    #[error("assertion failed:\nstdout:\n{stdout}\nstderr:\n{stderr}")]
    Assertion { stdout: String, stderr: String },
    #[error("RBS error: {0}")]
    Rbs(String),
}

/// A reader of Ruby code that we can validate.
#[async_trait]
pub trait RubyChecker {
    async fn check(&mut self, code: &str) -> Result<(), CheckError>;
}

/// Checker dispatcher: routes to Syntax or Assert based on snippet content.
pub enum Checker {
    Syntax(SyntaxChecker),
    Assert(AssertionChecker),
}

impl Checker {
    /// Returns the appropriate checker variant for the given snippet,
    /// creating it with the provided Ruby interpreter.
    pub fn for_snippet(ruby: rv_ruby::Ruby, snippet: &Snippet) -> Self {
        if AssertionChecker::applies_to(&snippet.code) {
            Checker::Assert(AssertionChecker::new(ruby))
        } else {
            Checker::Syntax(SyntaxChecker::new(ruby))
        }
    }
}

#[async_trait]
impl RubyChecker for Checker {
    async fn check(&mut self, code: &str) -> Result<(), CheckError> {
        match self {
            Checker::Syntax(c) => c.check(code).await,
            Checker::Assert(c) => c.check(code).await,
        }
    }
}

/// A Ruby code snippet found in a documentation comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snippet {
    pub item_name: String,
    pub item_kind: ItemKind,
    pub parent_path: String,
    pub start_line: u32,
    pub code: String,
}

/// A failure to check a snippet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub snippet: Snippet,
    pub message: String,
    /// Absolute 1-based line in the source file the failure refers to.
    pub line: u32,
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
                    // Comments attach contiguously above the item, so
                    // `comments[i]` sits at `span.start_line - len + i`.
                    // Point at the first code line inside the fence.
                    let fence_line = item.span.start_line - lines.len() as u32 + comment_idx as u32;
                    snippets.push(Snippet {
                        item_name: item.name.clone(),
                        item_kind: item.kind(),
                        parent_path: String::new(),
                        start_line: fence_line + 1,
                        code: content.join("\n"),
                    });
                }
                comment_idx = if closed_code_block {
                    code_end + 1
                } else {
                    code_end
                };
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

/// Checks multiple snippets, collecting all failures.
/// Dispatches each snippet to the appropriate checker (Syntax or Assertion).
pub async fn check_snippets(snippets: &[Snippet], ruby: &rv_ruby::Ruby) -> Vec<Failure> {
    let mut failures = Vec::new();
    for snippet in snippets {
        let mut checker = Checker::for_snippet(ruby.clone(), snippet);
        match checker.check(&snippet.code).await {
            Ok(()) => {}
            Err(e) => {
                failures.push(Failure {
                    snippet: snippet.clone(),
                    message: e.to_string(),
                    line: snippet.start_line,
                });
            }
        }
    }
    failures
}

/// Convenience: extract & check from a parsed file.
pub async fn check(parsed: &ParsedFileFromParser, ruby: &rv_ruby::Ruby) -> Vec<Failure> {
    check_snippets(&extract(parsed), ruby).await
}

#[cfg(test)]
mod tests;
