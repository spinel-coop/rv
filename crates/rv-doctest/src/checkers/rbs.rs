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
    /// The most positional arguments this signature accepts.
    pub fn max_args(&self) -> usize {
        if self.has_rest {
            usize::MAX
        } else {
            self.required_args + self.optional_args
        }
    }

    /// Broaden `self` to accept everything `other` accepts.
    ///
    /// An RBS method can declare several overloads, and a call is only a
    /// violation when *no* overload accepts it. The signature used for
    /// checking is therefore the union of them all: the smallest required
    /// count, the largest accepted count, and a block that is mandatory only
    /// when every overload requires one.
    fn widen(self, other: MethodSig) -> MethodSig {
        let max_args = self.max_args().max(other.max_args());
        let required_args = self.required_args.min(other.required_args);
        let has_rest = self.has_rest || other.has_rest;

        MethodSig {
            name: self.name,
            required_args,
            optional_args: if has_rest {
                0
            } else {
                max_args - required_args
            },
            has_rest,
            has_block: self.has_block && other.has_block,
        }
    }

    pub fn check_arity(&self, arg_count: usize, has_block: bool) -> ArityResult {
        if self.has_block && !has_block {
            return ArityResult::MissingBlock;
        }

        let max_args = self.max_args();

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

    // A class can be declared across several `.rbs` files (or reopened within
    // one), so merge into whatever is already indexed rather than replacing
    // it. The entry is registered even when it has no methods of its own, so
    // that `has_class` is true for a class declaring only nested types.
    let sigs = extract_method_sigs(&members);
    index.entry(full_path.clone()).or_default().extend(sigs);

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
    let mut merged: Option<MethodSig> = None;

    for overload in method.overloads().iter() {
        let Node::MethodDefinitionOverload(overload) = overload else {
            continue;
        };
        let Node::MethodType(method_type) = overload.method_type() else {
            continue;
        };

        let (required, optional, has_rest) = match method_type.type_() {
            Node::FunctionType(function) => (
                function.required_positionals().iter().count(),
                function.optional_positionals().iter().count(),
                function.rest_positionals().is_some(),
            ),
            _ => (0, 0, true),
        };

        // `block()` is also `Some` for an optional block (`?{ ... }`), which a
        // caller is free to omit; only a required block is mandatory.
        let has_block = method_type.block().is_some_and(|block| block.required());

        let sig = MethodSig {
            name: name.clone(),
            required_args: required,
            optional_args: optional,
            has_rest,
            has_block,
        };

        merged = Some(match merged {
            Some(existing) => existing.widen(sig),
            None => sig,
        });
    }

    if let Some(sig) = merged {
        sigs.insert(name, sig);
    }
}

/// The enclosing namespace of a fully-qualified definition path.
///
/// `Foo::Bar` -> `Foo`, `Foo::Bar#baz` -> `Foo::Bar`, `Foo.baz` -> `Foo`.
/// Returns `None` for an unqualified name, which has no parent.
pub fn full_path_to_parent(full_path: &str) -> Option<String> {
    // `rfind(':')` would land on the second colon of `::` and leave a trailing
    // one behind, so match the separator itself.
    let namespace = full_path.rfind("::");
    let method = full_path.rfind(['#', '.']);

    let split_at = match (namespace, method) {
        (Some(ns), Some(m)) => ns.max(m),
        (Some(ns), None) => ns,
        (None, Some(m)) => m,
        (None, None) => return None,
    };

    // A leading separator (`::Foo`) means the top level, which has no parent.
    (split_at > 0).then(|| full_path[..split_at].to_string())
}

/// RBS-aware arity checker for doctest snippets.
pub struct RbsChecker {
    env: RbsEnvironment,
}

impl RbsChecker {
    pub fn new(env: RbsEnvironment) -> Self {
        Self { env }
    }

    /// Check every call in `calls` against the RBS index.
    ///
    /// `calls` comes from [`crate::SnippetAnalysis`], so the snippet's code is
    /// parsed once and shared with the other checkers.
    pub fn check(&self, snippet: &Snippet, calls: &[CallSite], source_path: &str) -> CheckReport {
        let mut report = CheckReport::default();

        for call in calls.iter().cloned() {
            let resolved_class = resolve_class(&call, snippet);
            // `CallSite.line` is 1-based and `snippet.start_line` is already
            // the snippet's first code line, so the two overlap by one.
            let call_line = snippet.start_line + (call.line as u32).saturating_sub(1);
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

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;

    /// Load an `RbsEnvironment` from `(filename, contents)` pairs written into
    /// a throwaway `sig/` directory.
    fn env_from(files: &[(&str, &str)]) -> RbsEnvironment {
        let tmp = tempfile::tempdir().unwrap();
        let sig_dir = tmp.path().join("sig");
        std::fs::create_dir_all(&sig_dir).unwrap();
        for (name, contents) in files {
            std::fs::write(sig_dir.join(name), contents).unwrap();
        }
        RbsEnvironment::load(&[sig_dir.to_str().unwrap()]).unwrap()
    }

    fn snippet_of(code: &str, parent_path: &str, start_line: u32) -> Snippet {
        Snippet {
            item_name: "documented".to_string(),
            item_kind: rv_ruby_parser::ItemKind::Def,
            parent_path: parent_path.to_string(),
            start_line,
            code: code.to_string(),
        }
    }

    /// An environment with a single `Widget` class whose body is `members`.
    fn widget_env(members: &str) -> RbsEnvironment {
        env_from(&[("a.rbs", &format!("class Widget\n{members}\nend\n"))])
    }

    /// Run one fence through the checker, as `check_snippets` would.
    fn check_code(
        env: RbsEnvironment,
        code: &str,
        parent_path: &str,
        start_line: u32,
    ) -> CheckReport {
        let checker = RbsChecker::new(env);
        let snippet = snippet_of(code, parent_path, start_line);
        let calls = parse_calls_ast(snippet.code.as_bytes());
        checker.check(&snippet, &calls, "lib.rb")
    }

    #[test]
    fn full_path_to_parent_splits_on_the_namespace_separator() {
        // `rfind(':')` would land on the second colon and yield `Foo:`.
        assert_eq!(full_path_to_parent("Foo::Bar").as_deref(), Some("Foo"));
        assert_eq!(
            full_path_to_parent("Foo::Bar::Baz").as_deref(),
            Some("Foo::Bar")
        );
    }

    #[test]
    fn full_path_to_parent_splits_on_the_method_separator() {
        assert_eq!(
            full_path_to_parent("Foo::Bar#baz").as_deref(),
            Some("Foo::Bar")
        );
        assert_eq!(full_path_to_parent("Foo.baz").as_deref(), Some("Foo"));
        assert_eq!(full_path_to_parent("Foo#baz").as_deref(), Some("Foo"));
    }

    #[test]
    fn full_path_to_parent_has_no_parent_for_a_bare_name() {
        assert_eq!(full_path_to_parent("add"), None);
        assert_eq!(full_path_to_parent(""), None);
        assert_eq!(full_path_to_parent("::Foo"), None);
    }

    #[test]
    fn every_overload_is_accepted_not_just_the_first() {
        let env = widget_env(indoc! {"
            def one_or_two: (Integer) -> void
                          | (Integer, Integer) -> void
        "});

        let sig = env.lookup("Widget", "one_or_two").expect("indexed");
        assert_eq!(sig.check_arity(1, false), ArityResult::Valid);
        assert_eq!(sig.check_arity(2, false), ArityResult::Valid);
        assert_eq!(
            sig.check_arity(3, false),
            ArityResult::TooMany {
                expected: 2,
                found: 3
            }
        );
    }

    #[test]
    fn a_block_is_required_only_when_every_overload_requires_one() {
        let env = widget_env(indoc! {"
            def each: () { (untyped) -> void } -> self
                      | () -> Enumerator[untyped, untyped]
            def always: () { (untyped) -> void } -> void
        "});

        // `each` has a blockless overload, so omitting the block is fine.
        assert!(!env.lookup("Widget", "each").unwrap().has_block);
        assert_eq!(
            env.lookup("Widget", "each").unwrap().check_arity(0, false),
            ArityResult::Valid
        );

        assert_eq!(
            env.lookup("Widget", "always")
                .unwrap()
                .check_arity(0, false),
            ArityResult::MissingBlock
        );
    }

    #[test]
    fn an_optional_block_is_not_required() {
        let env = widget_env(indoc! {"
            def maybe: () ?{ (untyped) -> void } -> void
        "});

        let sig = env.lookup("Widget", "maybe").expect("indexed");
        assert!(!sig.has_block, "`?{{ ... }}` declares an optional block");
        assert_eq!(sig.check_arity(0, false), ArityResult::Valid);
    }

    #[test]
    fn a_class_split_across_files_keeps_every_method() {
        let env = env_from(&[
            ("a.rbs", "class Widget\n  def only_in_a: () -> void\nend\n"),
            ("b.rbs", "class Widget\n  def only_in_b: () -> void\nend\n"),
        ]);

        assert!(env.lookup("Widget", "only_in_a").is_some(), "a.rbs lost");
        assert!(env.lookup("Widget", "only_in_b").is_some(), "b.rbs lost");
    }

    #[test]
    fn a_class_declaring_only_nested_types_is_still_known() {
        let env = env_from(&[("a.rbs", "class Shell\n  type inner = Integer\nend\n")]);

        assert!(
            env.has_class("Shell"),
            "a class with no methods of its own should still be known"
        );
        assert!(env.lookup("Shell", "nope").is_none());
    }

    #[test]
    fn violation_lines_point_at_the_call_not_past_it() {
        let env = env_from(&[(
            "a.rbs",
            "class Calculator\n  def add: (Integer, Integer) -> Integer\nend\n",
        )]);
        // A fence whose first code line is file line 4.
        let report = check_code(env, "Calculator.add 1, 2\nCalculator.add 1, 2, 3", "", 4);

        assert_eq!(report.violations.len(), 1, "{:?}", report.violations);
        assert_eq!(
            report.violations[0].line, 5,
            "the bad call is on the fence's second line, i.e. file line 5"
        );
    }

    #[test]
    fn a_receiverless_call_resolves_against_the_enclosing_namespace() {
        let env = env_from(&[(
            "a.rbs",
            indoc! {"
                module Math
                  class Calculator
                    def add: (Integer, Integer) -> Integer
                  end
                end
            "},
        )]);
        let report = check_code(env, "add 1, 2, 3", "Math::Calculator", 1);

        assert_eq!(report.stats.total_unknown_receivers(), 0);
        assert_eq!(report.violations.len(), 1, "{:?}", report.violations);
        assert_eq!(report.violations[0].class_name, "Math::Calculator");
    }
}
