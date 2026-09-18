//! RBS-aware arity checker for doctest snippets.
//!
//! RBS signature environment and arity checking functionality.
//! Combines RBS types/environment with the checker logic.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use regex::Regex;

use ruby_rbs::node::{MethodDefinitionNode, Node, NodeList, TypeNameNode, parse};

use crate::{CheckError, Snippet};

/// Result of an arity check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArityResult {
    Valid,
    TooFew { expected: usize, found: usize },
    TooMany { expected: usize, found: usize },
    MissingBlock,
}

/// Method signature from RBS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodSig {
    pub name: String,
    pub required_args: usize,
    pub optional_args: usize,
    pub has_rest: bool,
    pub has_block: bool,
}

impl MethodSig {
    pub fn check_arity(&self, arg_count: usize, has_block: bool) -> ArityResult {
        if self.has_block && !has_block {
            return ArityResult::MissingBlock;
        }

        let max_args = if self.has_rest {
            usize::MAX
        } else {
            self.required_args + self.optional_args
        };

        if arg_count < self.required_args {
            ArityResult::TooFew {
                expected: self.required_args,
                found: arg_count,
            }
        } else if arg_count > max_args {
            ArityResult::TooMany {
                expected: max_args,
                found: arg_count,
            }
        } else {
            ArityResult::Valid
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RbsViolation {
    pub class_name: String,
    pub method_name: String,
    pub message: String,
    pub line: u32,
}

#[derive(Debug, Default)]
pub struct RbsEnvironment {
    class_methods: HashMap<String, HashMap<String, MethodSig>>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CheckReport {
    pub violations: Vec<RbsViolation>,
    pub stats: CheckStats,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckStats {
    pub checked: u32,
    pub unknown_receivers: BTreeMap<String, UnknownEntry>,
    pub unknown_classes: BTreeMap<String, UnknownEntry>,
    pub unknown_methods: BTreeMap<String, UnknownEntry>,
}

impl CheckStats {
    pub fn checked_count(&self) -> u32 {
        self.checked
    }

    pub fn total_unknown_receivers(&self) -> u32 {
        self.unknown_receivers.values().map(|e| e.count).sum()
    }

    pub fn total_unknown_classes(&self) -> u32 {
        self.unknown_classes.values().map(|e| e.count).sum()
    }

    pub fn total_unknown_methods(&self) -> u32 {
        self.unknown_methods.values().map(|e| e.count).sum()
    }

    pub fn skipped(&self) -> u32 {
        self.total_unknown_receivers() + self.total_unknown_classes() + self.total_unknown_methods()
    }

    pub fn merge(&mut self, other: CheckStats) {
        self.checked += other.checked;
        for (k, v) in other.unknown_receivers {
            self.unknown_receivers.entry(k).or_default().merge_from(&v);
        }
        for (k, v) in other.unknown_classes {
            self.unknown_classes.entry(k).or_default().merge_from(&v);
        }
        for (k, v) in other.unknown_methods {
            self.unknown_methods.entry(k).or_default().merge_from(&v);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownEntry {
    pub count: u32,
    pub locations: Vec<String>,
}

impl UnknownEntry {
    pub fn new() -> Self {
        Self {
            count: 0,
            locations: Vec::new(),
        }
    }

    pub fn record(&mut self, location: String) {
        self.count += 1;
        if !self.locations.contains(&location) {
            self.locations.push(location);
        }
    }

    pub fn merge_from(&mut self, other: &UnknownEntry) {
        self.count += other.count;
        for location in &other.locations {
            if !self.locations.contains(location) {
                self.locations.push(location.clone());
            }
        }
    }
}

impl Default for UnknownEntry {
    fn default() -> Self {
        Self::new()
    }
}

impl RbsEnvironment {
    pub fn load(dirs: &[&str]) -> Result<Self, CheckError> {
        let mut class_methods = HashMap::new();

        for dir in dirs {
            let pattern = format!("{}/**/*.rbs", dir.trim_end_matches('/'));
            let paths = glob::glob(&pattern)
                .map_err(|e| CheckError::Rbs(format!("invalid RBS glob `{pattern}`: {e}")))?;

            for path in paths {
                let path =
                    path.map_err(|e| CheckError::Rbs(format!("failed to read RBS path: {e}")))?;
                load_file(&path, &mut class_methods)?;
            }
        }

        Ok(Self { class_methods })
    }

    pub fn lookup(&self, class_path: &str, method: &str) -> Option<&MethodSig> {
        self.class_methods.get(class_path)?.get(method)
    }

    pub fn has_class(&self, class_path: &str) -> bool {
        self.class_methods.contains_key(class_path)
    }
}

fn load_file(
    path: &Path,
    index: &mut HashMap<String, HashMap<String, MethodSig>>,
) -> std::io::Result<()> {
    let content = std::fs::read_to_string(path)?;

    let signature =
        parse(&content).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    for node in signature.declarations().iter() {
        walk_node(&node, "", index);
    }

    Ok(())
}

fn type_name_string(name: &TypeNameNode) -> String {
    name.namespace()
        .path()
        .iter()
        .filter_map(|n| match n {
            Node::Symbol(sym) => Some(sym.to_string()),
            _ => None,
        })
        .chain(std::iter::once(name.name().to_string()))
        .collect::<Vec<_>>()
        .join("::")
}

fn walk_node(
    node: &Node,
    parent_path: &str,
    index: &mut HashMap<String, HashMap<String, MethodSig>>,
) {
    let (name, members) = match node {
        Node::Class(class_node) => (type_name_string(&class_node.name()), class_node.members()),
        Node::Module(module_node) => (type_name_string(&module_node.name()), module_node.members()),
        _ => return,
    };

    let full_path = if parent_path.is_empty() {
        name
    } else {
        format!("{parent_path}::{name}")
    };

    let sigs = extract_method_sigs(&members);
    if !sigs.is_empty() {
        index.insert(full_path.clone(), sigs);
    }

    for member_node in members.iter() {
        walk_node(&member_node, &full_path, index);
    }
}

fn extract_method_sigs(members: &NodeList) -> HashMap<String, MethodSig> {
    let mut sigs = HashMap::new();

    for member_node in members.iter() {
        match &member_node {
            Node::MethodDefinition(method) => insert_method(method, &mut sigs),
            Node::AttrReader(attr) => {
                let name = attr.name().to_string();
                sigs.entry(name.clone()).or_insert(MethodSig {
                    name,
                    required_args: 0,
                    optional_args: 0,
                    has_rest: false,
                    has_block: false,
                });
            }
            Node::AttrWriter(attr) => {
                let name = attr.name().to_string();
                sigs.entry(name.clone()).or_insert(MethodSig {
                    name,
                    required_args: 1,
                    optional_args: 0,
                    has_rest: false,
                    has_block: false,
                });
            }
            Node::Alias(alias) => {
                let old_name = alias.old_name().to_string();
                if let Some(sig) = sigs.get(&old_name) {
                    let new_name = alias.new_name().to_string();
                    sigs.insert(new_name, sig.clone());
                }
            }
            _ => {}
        }
    }

    sigs
}

fn insert_method(method: &MethodDefinitionNode, sigs: &mut HashMap<String, MethodSig>) {
    let name = method.name().to_string();

    let Some(overload) = method.overloads().iter().next() else {
        return;
    };
    let Node::MethodDefinitionOverload(overload) = overload else {
        return;
    };

    let Node::MethodType(method_type) = overload.method_type() else {
        return;
    };

    let (required, optional, has_rest) = match method_type.type_() {
        Node::FunctionType(function) => (
            function.required_positionals().iter().count(),
            function.optional_positionals().iter().count(),
            function.rest_positionals().is_some(),
        ),
        _ => (0, 0, true),
    };

    let has_block = method_type.block().is_some();

    sigs.insert(
        name.clone(),
        MethodSig {
            name,
            required_args: required,
            optional_args: optional,
            has_rest,
            has_block,
        },
    );
}

pub fn full_path_to_parent(full_path: &str) -> Option<String> {
    if full_path.is_empty() {
        return None;
    }
    let last_segment = full_path.rfind(':').or_else(|| full_path.rfind('.'))?;
    Some(full_path[..last_segment].to_string())
}

/// A method call extracted from a snippet.
#[derive(Debug, Clone, PartialEq)]
pub struct CallSite {
    /// Resolved class constant if the receiver is a class constant (`Foo`, `Foo::Bar`).
    pub class_name: Option<String>,
    /// Raw receiver text for greppable reporting (`user.name`, `self.foo`).
    /// `None` for bare calls.
    pub receiver_text: Option<String>,
    pub name: String,
    pub arg_count: usize,
    pub has_block: bool,
    /// 0-based physical line of the call within the snippet's code.
    pub line: usize,
}

/// RBS-aware arity checker for doctest snippets.
pub struct RbsChecker {
    env: RbsEnvironment,
}

impl RbsChecker {
    pub fn new(env: RbsEnvironment) -> Self {
        Self { env }
    }

    pub fn check(&self, snippet: &Snippet, source_path: &str) -> CheckReport {
        let calls = parse_calls(snippet.code.as_bytes());
        let mut report = CheckReport::default();

        for call in calls {
            let resolved_class = resolve_class(&call, snippet);
            let call_line = snippet.start_line + call.line as u32;
            let location = format!("{source_path}:{call_line}");

            match resolved_class {
                Some(class) if !self.env.has_class(&class) => {
                    record_unknown_in_map(&class, &mut report.stats.unknown_classes, &location);
                }
                Some(class) => match self.env.lookup(&class, &call.name) {
                    Some(sig) => {
                        report.stats.checked += 1;
                        match sig.check_arity(call.arg_count, call.has_block) {
                            ArityResult::TooFew { expected, found } => {
                                report.violations.push(RbsViolation {
                                    class_name: class,
                                    method_name: call.name,
                                    message: format!(
                                        "expected at least {expected} args, found {found}"
                                    ),
                                    line: call_line,
                                });
                            }
                            ArityResult::TooMany { expected, found } => {
                                report.violations.push(RbsViolation {
                                    class_name: class,
                                    method_name: call.name,
                                    message: format!(
                                        "expected at most {expected} args, found {found}"
                                    ),
                                    line: call_line,
                                });
                            }
                            ArityResult::MissingBlock => {
                                report.violations.push(RbsViolation {
                                    class_name: class,
                                    method_name: call.name,
                                    message: "expected a block".to_string(),
                                    line: call_line,
                                });
                            }
                            ArityResult::Valid => {}
                        }
                    }
                    None => {
                        record_unknown_in_map(
                            &format!("{class}.{}", call.name),
                            &mut report.stats.unknown_methods,
                            &location,
                        );
                    }
                },
                None => {
                    let key = call
                        .receiver_text
                        .clone()
                        .unwrap_or_else(|| call.name.clone());
                    record_unknown_in_map(&key, &mut report.stats.unknown_receivers, &location);
                }
            }
        }

        report
    }
}

fn record_unknown_in_map(key: &str, target: &mut BTreeMap<String, UnknownEntry>, location: &str) {
    target
        .entry(key.to_string())
        .or_default()
        .record(location.to_string());
}

pub fn parse_calls(source: &[u8]) -> Vec<CallSite> {
    let code = String::from_utf8_lossy(source);
    extract_calls(&code)
}

const KEYWORDS: &[&str] = &[
    "alias",
    "and",
    "attr_accessor",
    "attr_reader",
    "attr_writer",
    "begin",
    "break",
    "case",
    "class",
    "def",
    "do",
    "else",
    "elsif",
    "end",
    "ensure",
    "extend",
    "for",
    "if",
    "include",
    "lambda",
    "module",
    "next",
    "not",
    "or",
    "private",
    "protected",
    "public",
    "raise",
    "redo",
    "require",
    "require_relative",
    "rescue",
    "retry",
    "return",
    "self",
    "super",
    "then",
    "unless",
    "until",
    "when",
    "while",
    "yield",
];

fn extract_calls(code: &str) -> Vec<CallSite> {
    let qualified =
        Regex::new(r"(::)?([A-Z][\w]*(?:::[A-Z][\w]*)*)\.(\w+)([?!]?)\s*\((?P<args>.*)").unwrap();
    let unresolvable =
        Regex::new(r"(?:^|[\s,=(\[{])(@?[a-z_]\w*|self)\.(\w+)([?!]?)\s*\((?P<args>.*)").unwrap();
    let bare = Regex::new(r"^\s*([a-z_]\w*)([?!]?)\s*\((?P<args>.*)").unwrap();

    let mut calls = Vec::new();

    let lines: Vec<(usize, String)> = logical_lines(code);
    for (line_no, line) in lines {
        let line = line.as_str();
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        for cap in qualified.captures_iter(line) {
            let class_name = format!(
                "{}{}",
                cap.get(1).map(|g| g.as_str()).unwrap_or(""),
                &cap[2]
            );
            let name = format!("{}{}", &cap[3], &cap[4]);
            let args = cap.name("args").map(|m| m.as_str()).unwrap_or("");
            calls.push(CallSite {
                class_name: Some(class_name),
                receiver_text: None,
                name,
                arg_count: count_args(strip_matching_paren(args)),
                has_block: has_block(line),
                line: line_no,
            });
        }

        for cap in unresolvable.captures_iter(line) {
            let receiver = cap[1].to_string();
            let name = format!("{}{}", &cap[2], &cap[3]);
            let args = cap.name("args").map(|m| m.as_str()).unwrap_or("");
            calls.push(CallSite {
                class_name: None,
                receiver_text: Some(format!("{receiver}.{name}")),
                name,
                arg_count: count_args(strip_matching_paren(args)),
                has_block: has_block(line),
                line: line_no,
            });
        }

        if let Some(cap) = bare.captures(line) {
            let name = format!("{}{}", &cap[1], &cap[2]);
            if KEYWORDS.contains(&name.as_str()) {
                continue;
            }
            let args = cap.name("args").map(|m| m.as_str()).unwrap_or("");
            calls.push(CallSite {
                class_name: None,
                receiver_text: None,
                name,
                arg_count: count_args(strip_matching_paren(args)),
                has_block: has_block(line),
                line: line_no,
            });
        }
    }

    calls
}

fn logical_lines(code: &str) -> Vec<(usize, String)> {
    let mut buf = String::new();
    let mut start_line = 0usize;
    let mut depth = 0i32;
    let mut in_string: Option<char> = None;
    let mut escaped = false;
    let mut result = Vec::new();

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
            result.push((start_line, std::mem::take(&mut buf)));
        }
    }
    if !buf.is_empty() {
        result.push((start_line, buf));
    }
    result
}

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

fn has_block(line: &str) -> bool {
    let has_brace = line.contains('{') && line.contains('}');
    let has_do = line.split_whitespace().any(|tok| tok == "do")
        || line.contains(" do ")
        || line.ends_with(" do");
    has_brace || has_do
}

fn resolve_class(call: &CallSite, snippet: &Snippet) -> Option<String> {
    match (&call.class_name, &call.receiver_text) {
        (Some(name), _) => Some(name.clone()),
        (None, None) if !snippet.parent_path.is_empty() => Some(snippet.parent_path.clone()),
        _ => None,
    }
}
