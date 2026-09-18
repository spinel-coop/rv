//! RBS-aware arity checker for doctest snippets.
//!
//! Extracts method calls from fenced Ruby code blocks and validates
//! argument counts against RBS method signatures. Permissive by design:
//! unknown receivers or methods are skipped rather than flagged.

use regex::Regex;

use crate::rbs::{ArityResult, RbsEnvironment};
use crate::Snippet;

pub struct RbsChecker {
    env: RbsEnvironment,
}

/// A method call extracted from a snippet.
#[derive(Debug, Clone, PartialEq)]
struct MethodCall {
    class_name: Option<String>,
    method_name: String,
    arg_count: usize,
    has_block: bool,
}

/// A violation of an RBS method arity found in a snippet.
#[derive(Debug, Clone, PartialEq)]
pub struct RbsViolation {
    pub class_name: String,
    pub method_name: String,
    pub message: String,
}

/// Ruby keywords and builtins that must never be treated as method calls.
const KEYWORDS: &[&str] = &[
    "alias", "and", "attr_accessor", "attr_reader", "attr_writer", "begin", "break", "case",
    "class", "def", "do", "else", "elsif", "end", "ensure", "extend", "for", "if", "include",
    "lambda", "module", "next", "not", "or", "private", "protected", "public", "raise", "redo",
    "require", "require_relative", "rescue", "retry", "return", "self", "super", "then", "unless",
    "until", "when", "while", "yield",
];

impl RbsChecker {
    pub fn new(env: RbsEnvironment) -> Self {
        Self { env }
    }

    /// Check all method calls in a snippet against the RBS environment.
    pub fn check(&self, snippet: &Snippet) -> Vec<RbsViolation> {
        let calls = extract_calls(&snippet.code);
        let mut violations = Vec::new();

        for call in &calls {
            let resolved_class = match &call.class_name {
                Some(name) => name.clone(),
                None if !snippet.parent_path.is_empty() => snippet.parent_path.clone(),
                None => continue,
            };

            match self.env.check_arity(
                &resolved_class,
                &call.method_name,
                call.arg_count,
                call.has_block,
            ) {
                ArityResult::TooFew { expected, found } => {
                    violations.push(RbsViolation {
                        class_name: resolved_class,
                        method_name: call.method_name.clone(),
                        message: format!("expected at least {expected} args, found {found}"),
                    });
                }
                ArityResult::TooMany { expected, found } => {
                    violations.push(RbsViolation {
                        class_name: resolved_class,
                        method_name: call.method_name.clone(),
                        message: format!("expected at most {expected} args, found {found}"),
                    });
                }
                ArityResult::MissingBlock => {
                    violations.push(RbsViolation {
                        class_name: resolved_class,
                        method_name: call.method_name.clone(),
                        message: "expected a block".to_string(),
                    });
                }
                ArityResult::Valid => {}
            }
        }

        violations
    }
}

/// Extract method calls from a code snippet using line-based heuristics.
///
/// Handles two shapes:
///   - `Receiver.method(args)` — receiver that looks like a class constant
///     (`Foo`, `Foo::Bar`, `::Foo`) resolves to a class name; any other
///     receiver produces a call with an unknown class (skipped later).
///   - `method(args)` — bare call, resolved later against the snippet's
///     `parent_path`.
fn extract_calls(code: &str) -> Vec<MethodCall> {
    // Receiver-qualified call: `Foo.method(...)`, `Foo::Bar.method x, y`
    let qualified =
        Regex::new(r"(::)?([A-Z][\w]*(?:::[A-Z][\w]*)*)\.(\w+)([?!]?)\s*(\()(?P<args>.*)")
            .unwrap();
    // Bare call at (possibly indented) line start: `method(...)`
    let bare = Regex::new(r"^\s*([a-z_]\w*)([?!]?)\s*(\()(?P<args>.*)").unwrap();

    let mut calls = Vec::new();

    for line in logical_lines(code) {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        for cap in qualified.captures_iter(&line) {
            let class_name = format!(
                "{}{}",
                cap.get(1).map(|g| g.as_str()).unwrap_or(""),
                &cap[2]
            );
            let method_name = format!("{}{}", &cap[3], &cap[4]);
            let args = cap.name("args").map(|m| m.as_str()).unwrap_or("");
            calls.push(MethodCall {
                class_name: Some(class_name),
                method_name,
                arg_count: count_args(strip_matching_paren(args)),
                has_block: has_block(&line),
            });
        }

        if let Some(cap) = bare.captures(&line) {
            let method_name = format!("{}{}", &cap[1], &cap[2]);
            if !KEYWORDS.contains(&method_name.as_str()) {
                let args = cap.name("args").map(|m| m.as_str()).unwrap_or("");
                calls.push(MethodCall {
                    class_name: None,
                    method_name,
                    arg_count: count_args(strip_matching_paren(args)),
                    has_block: has_block(&line),
                });
            }
        }
    }

    calls
}

/// Join physical lines into logical lines: a line that opens a bracket,
/// brace, or paren without closing it is continued with the following
/// lines until balance is restored. Strings are honored so `(` inside a
/// literal doesn't count.
fn logical_lines(code: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut depth = 0i32;
    let mut in_string: Option<char> = None;
    let mut escaped = false;

    for line in code.lines() {
        if !buf.is_empty() {
            buf.push(' ');
        }
        for ch in line.chars() {
            if let Some(quote) = in_string {
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == quote {
                    in_string = None;
                }
                buf.push(ch);
                continue;
            }
            match ch {
                '"' | '\'' => in_string = Some(ch),
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => depth = depth.saturating_sub(1),
                _ => {}
            }
            buf.push(ch);
        }
        if depth == 0 {
            out.push(std::mem::take(&mut buf));
        }
    }
    if !buf.is_empty() {
        out.push(buf);
    }
    out
}

/// Trim a trailing substring that was captured after `(` so we count the
/// argument list, not the rest of the expression. This only matters when a
/// closing `)` exists on the same line; otherwise the rest of the line is
/// treated as the argument list.
fn strip_matching_paren(s: &str) -> &str {
    let mut depth = 1;
    for (i, ch) in s.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                depth -= 1;
                if depth == 0 {
                    return &s[..i];
                }
            }
            _ => {}
        }
    }
    s
}

/// Count comma-separated arguments, ignoring commas nested in
/// brackets/braces/parens and inside string literals.
fn count_args(s: &str) -> usize {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return 0;
    }
    let mut count = 1;
    let mut depth = 0i32;
    let mut in_string: Option<char> = None;
    let mut escaped = false;

    for ch in trimmed.chars() {
        if let Some(quote) = in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote {
                in_string = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => in_string = Some(ch),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => count += 1,
            _ => {}
        }
    }
    count
}

/// Detect a block at the end of the line: `{ ... }` or `do ... end` /
/// `do |...|`.
fn has_block(line: &str) -> bool {
    let has_brace = line.contains('{') && line.contains('}');
    let has_do = line.split_whitespace().any(|tok| tok == "do")
        || line.contains(" do ")
        || line.ends_with(" do");
    has_brace || has_do
}

#[cfg(test)]
mod tests;
