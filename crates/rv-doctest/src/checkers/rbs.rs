//! RBS-aware arity checker for doctest snippets.
//!
//! RBS signature environment and arity checking functionality.
//! Combines RBS types/environment with the checker logic.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use ruby_rbs::node::{MethodDefinitionNode, Node, NodeList, TypeNameNode, parse};

use crate::{CheckError, Snippet};
use rv_ruby_parser::calls::CallSite;
use rv_ruby_parser::calls::parse_calls as parse_calls_ast;

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

/// RBS-aware arity checker for doctest snippets.
pub struct RbsChecker {
    env: RbsEnvironment,
}

impl RbsChecker {
    pub fn new(env: RbsEnvironment) -> Self {
        Self { env }
    }

    pub fn check(&self, snippet: &Snippet, source_path: &str) -> CheckReport {
        let calls = parse_calls_ast(snippet.code.as_bytes());
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

pub(crate) fn resolve_class(call: &CallSite, snippet: &Snippet) -> Option<String> {
    match (&call.class_name, &call.receiver_text) {
        (Some(name), _) => Some(name.clone()),
        (None, None) if !snippet.parent_path.is_empty() => Some(snippet.parent_path.clone()),
        _ => None,
    }
}

pub fn parse_calls(source: &[u8]) -> Vec<CallSite> {
    parse_calls_ast(source)
}
