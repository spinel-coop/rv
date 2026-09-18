//! RBS-aware arity checker for doctest snippets.
//!
//! Extracts method calls from fenced Ruby code blocks and validates
//! argument counts against RBS method signatures. Every extracted call is
//! classified — checked, or skipped with a reason — so callers can report
//! how much of their documentation is actually covered by signatures.

use std::collections::BTreeMap;

use regex::Regex;

use crate::rbs::{ArityResult, RbsEnvironment};
use crate::Snippet;

pub struct RbsChecker {
    env: RbsEnvironment,
}

/// A method call extracted from a snippet.
#[derive(Debug, Clone, PartialEq)]
struct MethodCall {
    /// Resolved class constant if the receiver is one (`Foo`, `Foo::Bar`).
    class_name: Option<String>,
    /// Raw receiver text for greppable reporting (`user.name`, `self.foo`).
    /// `None` for bare calls.
    receiver_text: Option<String>,
    method_name: String,
    arg_count: usize,
    has_block: bool,
    /// 0-based physical line of the call within the snippet's code.
    line_offset: u32,
}

/// A violation of an RBS method arity found in a snippet.
#[derive(Debug, Clone, PartialEq)]
pub struct RbsViolation {
    pub class_name: String,
    pub method_name: String,
    pub message: String,
    /// Absolute 1-based line of the call in the source file.
    pub line: u32,
}

/// One skipped call name plus where it was seen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownEntry {
    pub count: u32,
    /// Plain `path:line` strings, one per occurrence, insertion-ordered
    /// and deduplicated.
    pub locations: Vec<String>,
}

impl UnknownEntry {
    fn new() -> Self {
        Self {
            count: 0,
            locations: Vec::new(),
        }
    }

    fn record(&mut self, location: String) {
        self.count += 1;
        if !self.locations.contains(&location) {
            self.locations.push(location);
        }
    }
}

/// How many calls were checked versus skipped, with greppable names and
/// source locations for the skipped ones.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckStats {
    pub checked: u32,
    pub unknown_receivers: BTreeMap<String, UnknownEntry>,
    pub unknown_classes: BTreeMap<String, UnknownEntry>,
    pub unknown_methods: BTreeMap<String, UnknownEntry>,
}

impl CheckStats {
    /// Fold `other` into `self`, summing per-name counts and unioning
    /// locations (deduplicated, insertion-ordered).
    pub fn merge(&mut self, other: &CheckStats) {
        self.checked += other.checked;
        merge_entries(&mut self.unknown_receivers, &other.unknown_receivers);
        merge_entries(&mut self.unknown_classes, &other.unknown_classes);
        merge_entries(&mut self.unknown_methods, &other.unknown_methods);
    }

    /// Total number of skipped calls across all categories.
    pub fn skipped(&self) -> u32 {
        self.unknown_receivers.values().map(|e| e.count).sum::<u32>()
            + self.unknown_classes.values().map(|e| e.count).sum::<u32>()
            + self.unknown_methods.values().map(|e| e.count).sum::<u32>()
    }
}

fn merge_entries(into: &mut BTreeMap<String, UnknownEntry>, from: &BTreeMap<String, UnknownEntry>) {
    for (name, entry) in from {
        into.entry(name.clone())
            .or_insert_with(|| UnknownEntry {
                count: 0,
                locations: Vec::new(),
            })
            .merge_from(entry);
    }
}

impl UnknownEntry {
    fn merge_from(&mut self, other: &UnknownEntry) {
        self.count += other.count;
        for location in &other.locations {
            if !self.locations.contains(location) {
                self.locations.push(location.clone());
            }
        }
    }
}

/// The outcome of checking one snippet: arity violations plus coverage stats.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CheckReport {
    pub violations: Vec<RbsViolation>,
    pub stats: CheckStats,
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
    ///
    /// `source_path` is recorded in the stats so skipped entries can be
    /// traced back to the files they came from. Every extracted call is
    /// classified: it increments `checked` (and may produce a violation),
    /// or it lands in exactly one skip bucket.
    pub fn check(&self, snippet: &Snippet, source_path: &str) -> CheckReport {
        let calls = extract_calls(&snippet.code);
        let mut report = CheckReport::default();

        for call in &calls {
            // Only bare calls (no receiver at all) fall back to `parent_path`.
            // An explicit lowercase/ivar/self receiver is unresolvable, period.
            let resolved_class = match (&call.class_name, &call.receiver_text) {
                (Some(name), _) => Some(name.clone()),
                (None, None) if !snippet.parent_path.is_empty() => {
                    Some(snippet.parent_path.clone())
                }
                _ => None,
            };
            let call_line = snippet.start_line + call.line_offset;

            let Some(resolved_class) = resolved_class else {
                let key = call
                    .receiver_text
                    .clone()
                    .unwrap_or_else(|| call.method_name.clone());
                report
                    .stats
                    .unknown_receivers
                    .entry(key)
                    .or_insert_with(UnknownEntry::new)
                    .record(format!("{source_path}:{call_line}"));
                continue;
            };

            if !self.env.has_class(&resolved_class) {
                report
                    .stats
                    .unknown_classes
                    .entry(resolved_class.clone())
                    .or_insert_with(UnknownEntry::new)
                    .record(format!("{source_path}:{call_line}"));
                continue;
            }

            if self.env.lookup(&resolved_class, &call.method_name).is_none() {
                report
                    .stats
                    .unknown_methods
                    .entry(format!("{resolved_class}.{}", call.method_name))
                    .or_insert_with(UnknownEntry::new)
                    .record(format!("{source_path}:{call_line}"));
                continue;
            }

            report.stats.checked += 1;

            match self.env.check_arity(
                &resolved_class,
                &call.method_name,
                call.arg_count,
                call.has_block,
            ) {
                ArityResult::TooFew { expected, found } => {
                    report.violations.push(RbsViolation {
                        class_name: resolved_class,
                        method_name: call.method_name.clone(),
                        message: format!("expected at least {expected} args, found {found}"),
                        line: call_line,
                    });
                }
                ArityResult::TooMany { expected, found } => {
                    report.violations.push(RbsViolation {
                        class_name: resolved_class,
                        method_name: call.method_name.clone(),
                        message: format!("expected at most {expected} args, found {found}"),
                        line: call_line,
                    });
                }
                ArityResult::MissingBlock => {
                    report.violations.push(RbsViolation {
                        class_name: resolved_class,
                        method_name: call.method_name.clone(),
                        message: "expected a block".to_string(),
                        line: call_line,
                    });
                }
                ArityResult::Valid => {}
            }
        }

        report
    }
}

/// Extract method calls from a code snippet using line-based heuristics.
///
/// Handles three shapes:
///   - `Class.method(args)` — receiver is a class constant; resolves directly.
///   - `receiver.method(args)` — lowercase/`self`/ivar receiver; reported as
///     an unknown receiver.
///   - `method(args)` — bare call, resolved against the snippet's
///     `parent_path` if one is set.
fn extract_calls(code: &str) -> Vec<MethodCall> {
    // (content, 0-based physical line of the logical line's first line)
    // Receiver-qualified call on a class constant: `Foo.method(...)`, `Foo::Bar.method(...)`
    let qualified =
        Regex::new(r"(::)?([A-Z][\w]*(?:::[A-Z][\w]*)*)\.(\w+)([?!]?)\s*\((?P<args>.*)").unwrap();
    // Unresolvable receiver: `user.name(...)`, `self.foo(...)`, `@bar.baz(...)`
    let unresolvable =
        Regex::new(r"(?:^|[\s,=(\[{])(@?[a-z_]\w*|self)\.(\w+)([?!]?)\s*\((?P<args>.*)").unwrap();
    // Bare call at (possibly indented) line start: `method(...)`
    let bare = Regex::new(r"^\s*([a-z_]\w*)([?!]?)\s*\((?P<args>.*)").unwrap();

    let mut calls = Vec::new();

    for (line_no, line) in logical_lines(code) {
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
                receiver_text: None,
                method_name,
                arg_count: count_args(strip_matching_paren(args)),
                has_block: has_block(&line),
                line_offset: line_no as u32,
            });
        }

        for cap in unresolvable.captures_iter(&line) {
            let receiver = cap[1].to_string();
            let method_name = format!("{}{}", &cap[2], &cap[3]);
            let args = cap.name("args").map(|m| m.as_str()).unwrap_or("");
            calls.push(MethodCall {
                class_name: None,
                receiver_text: Some(format!("{receiver}.{method_name}")),
                method_name,
                arg_count: count_args(strip_matching_paren(args)),
                has_block: has_block(&line),
                line_offset: line_no as u32,
            });
        }

        if let Some(cap) = bare.captures(&line) {
            let method_name = format!("{}{}", &cap[1], &cap[2]);
            if !KEYWORDS.contains(&method_name.as_str()) {
                let args = cap.name("args").map(|m| m.as_str()).unwrap_or("");
                calls.push(MethodCall {
                    class_name: None,
                    receiver_text: None,
                    method_name,
                    arg_count: count_args(strip_matching_paren(args)),
                    has_block: has_block(&line),
                    line_offset: line_no as u32,
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
///
/// Each yielded pair is `(0-based physical start line, joined content)` so
/// callers can map a call back to the exact line it starts on.
fn logical_lines(code: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut start_line = 0usize;
    let mut depth = 0i32;
    let mut in_string: Option<char> = None;
    let mut escaped = false;

    for (i, line) in code.lines().enumerate() {
        if buf.is_empty() {
            start_line = i;
        } else {
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
            out.push((start_line, std::mem::take(&mut buf)));
        }
    }
    if !buf.is_empty() {
        out.push((start_line, buf));
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
