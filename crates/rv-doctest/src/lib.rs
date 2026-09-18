//! RBS-aware arity checker and snippet runner for doctest documentation.
//!
//! Extracts method calls from fenced Ruby code blocks and validates
//! argument counts against RBS method signatures.

use rv_ruby_parser::ItemKind;

mod checker;
mod checkers;

pub use checker::{assertion_applies_to, assertion_check, check_file_syntax, syntax_check};
#[derive(Clone, PartialEq, Eq)]
pub struct Snippet {
    pub item_name: String,
    pub item_kind: ItemKind,
    pub parent_path: String,
    pub start_line: u32,
    pub code: String,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Failure {
    pub snippet: Snippet,
    pub message: String,
    pub line: u32,
}

pub use checkers::rbs::{
    ArityResult, CheckReport, CheckStats, MethodSig, RbsEnvironment, RbsViolation, UnknownEntry,
};
pub use checkers::rbs::{RbsChecker, parse_calls};

#[derive(Debug, thiserror::Error)]
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

/// Extracts snippets from Ruby documentation comments.
pub fn extract(parsed: &rv_ruby_parser::ParsedFile) -> Vec<Snippet> {
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
                    let fence_line = item.span.start_line - lines.len() as u32 + comment_idx as u32;
                    let parent_path =
                        checkers::rbs::full_path_to_parent(&item.full_path).unwrap_or_default();
                    snippets.push(Snippet {
                        item_name: item.name.clone(),
                        parent_path,
                        item_kind: item.kind(),
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
pub async fn check_snippets(snippets: &[Snippet], ruby: &rv_ruby::Ruby) -> Vec<Failure> {
    let mut failures = Vec::new();
    for snippet in snippets {
        let result = if assertion_applies_to(&snippet.code) {
            assertion_check(ruby.clone(), &snippet.code).await
        } else {
            syntax_check(ruby.clone(), &snippet.code).await
        };

        if let Err(e) = result {
            failures.push(Failure {
                snippet: snippet.clone(),
                message: e.to_string(),
                line: snippet.start_line,
            });
        }
    }
    failures
}

#[cfg(test)]
mod tests;
