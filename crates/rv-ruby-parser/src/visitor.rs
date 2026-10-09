//! A single statement walk shared by every traversal in this crate.
//!
//! Each consumer (definitions, call sites, the minitest `require`) used to
//! carry its own hand-rolled recursion over `class`/`module`/`def` bodies.
//! They differ only in which nested scopes they descend into, whether they
//! stop early, and what they collect — so those are the three knobs here.

use ruby_prism::{Node, ProgramNode, StatementsNode};

use super::node_source_slice;

/// Controls whether a walk continues after a statement is visited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Flow {
    Continue,
    Stop,
}

/// Which nested scopes a walk descends into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Scopes {
    pub(crate) classes: bool,
    pub(crate) modules: bool,
    pub(crate) defs: bool,
    pub(crate) singleton_classes: bool,
}

impl Scopes {
    /// `class` and `module` bodies only.
    ///
    /// These execute where they are written, and are where nested definitions
    /// live. A `def` body is skipped: it does not run until the method is
    /// called, and the definitions inside it are not namespace members.
    pub(crate) const DECLARATIVE: Self = Self {
        classes: true,
        modules: true,
        defs: false,
        singleton_classes: false,
    };

    /// Every body that can hold statements, including `def` and `class << self`.
    pub(crate) const ALL: Self = Self {
        classes: true,
        modules: true,
        defs: true,
        singleton_classes: true,
    };
}

/// Qualify a constant path written inside `parent`.
///
/// A path written with a leading `::` is absolute and keeps only its own
/// segments; anything else nests under the enclosing namespace.
pub(crate) fn qualify_constant(parent: &str, declared: &str) -> String {
    match declared.strip_prefix("::") {
        Some(absolute) => absolute.to_string(),
        None if parent.is_empty() => declared.to_string(),
        None => format!("{parent}::{declared}"),
    }
}

/// The enclosing `class`/`module` path during a walk.
///
/// Each entry is the fully-qualified path at that depth, so reading the
/// current namespace is a lookup rather than a join.
struct Namespace(Vec<String>);

impl Namespace {
    fn new() -> Self {
        Self(Vec::new())
    }

    fn current(&self) -> &str {
        self.0.last().map(String::as_str).unwrap_or("")
    }

    fn push(&mut self, declared: &str) {
        let qualified = qualify_constant(self.current(), declared);
        self.0.push(qualified);
    }

    fn pop(&mut self) {
        self.0.pop();
    }
}

/// Walk every statement in `program`, outermost first.
///
/// `visit` receives each statement and the namespace it is written in (`""`,
/// `Foo`, `Foo::Bar`), and returns [`Flow::Stop`] to end the walk early.
pub(crate) fn walk_program<F>(
    program: &ProgramNode<'_>,
    source: &[u8],
    scopes: Scopes,
    visit: &mut F,
) where
    F: FnMut(&Node<'_>, &str) -> Flow,
{
    let mut namespace = Namespace::new();
    walk_statements(&program.statements(), source, scopes, &mut namespace, visit);
}

/// Walk a single statement and everything nested inside it.
///
/// Used when a caller needs to attribute results to a particular top-level
/// statement; [`walk_program`] is the whole-file form.
pub(crate) fn walk_node<F>(node: &Node<'_>, source: &[u8], scopes: Scopes, visit: &mut F) -> Flow
where
    F: FnMut(&Node<'_>, &str) -> Flow,
{
    let mut namespace = Namespace::new();
    walk_statement(node, source, scopes, &mut namespace, visit)
}

fn walk_statements<F>(
    statements: &StatementsNode<'_>,
    source: &[u8],
    scopes: Scopes,
    namespace: &mut Namespace,
    visit: &mut F,
) -> Flow
where
    F: FnMut(&Node<'_>, &str) -> Flow,
{
    for statement in statements.body().iter() {
        if walk_statement(&statement, source, scopes, namespace, visit) == Flow::Stop {
            return Flow::Stop;
        }
    }
    Flow::Continue
}

fn walk_statement<F>(
    node: &Node<'_>,
    source: &[u8],
    scopes: Scopes,
    namespace: &mut Namespace,
    visit: &mut F,
) -> Flow
where
    F: FnMut(&Node<'_>, &str) -> Flow,
{
    if visit(node, namespace.current()) == Flow::Stop {
        return Flow::Stop;
    }

    if let Some(statements) = node.as_statements_node() {
        return walk_statements(&statements, source, scopes, namespace, visit);
    }

    if let Some(class) = node.as_class_node() {
        if !scopes.classes {
            return Flow::Continue;
        }
        return walk_named_scope(
            class.body(),
            &class.constant_path(),
            source,
            scopes,
            namespace,
            visit,
        );
    }

    if let Some(module) = node.as_module_node() {
        if !scopes.modules {
            return Flow::Continue;
        }
        return walk_named_scope(
            module.body(),
            &module.constant_path(),
            source,
            scopes,
            namespace,
            visit,
        );
    }

    if let Some(def) = node.as_def_node() {
        if !scopes.defs {
            return Flow::Continue;
        }
        // A method body adds no namespace segment: `Foo#bar` is not a
        // namespace that further definitions nest under.
        return walk_body(def.body(), None, source, scopes, namespace, visit);
    }

    if let Some(singleton) = node.as_singleton_class_node() {
        if !scopes.singleton_classes {
            return Flow::Continue;
        }
        return walk_body(singleton.body(), None, source, scopes, namespace, visit);
    }

    Flow::Continue
}

/// Descend into a `class`/`module` body, qualifying the written constant path
/// under the enclosing namespace.
fn walk_named_scope<F>(
    body: Option<Node<'_>>,
    constant_path: &Node<'_>,
    source: &[u8],
    scopes: Scopes,
    namespace: &mut Namespace,
    visit: &mut F,
) -> Flow
where
    F: FnMut(&Node<'_>, &str) -> Flow,
{
    let declared = node_source_slice(source, constant_path);
    walk_body(body, Some(&declared), source, scopes, namespace, visit)
}

fn walk_body<F>(
    body: Option<Node<'_>>,
    segment: Option<&str>,
    source: &[u8],
    scopes: Scopes,
    namespace: &mut Namespace,
    visit: &mut F,
) -> Flow
where
    F: FnMut(&Node<'_>, &str) -> Flow,
{
    let Some(statements) = body.as_ref().and_then(Node::as_statements_node) else {
        return Flow::Continue;
    };

    if let Some(segment) = segment {
        namespace.push(segment);
    }
    let flow = walk_statements(&statements, source, scopes, namespace, visit);
    if segment.is_some() {
        namespace.pop();
    }
    flow
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every statement the walk reaches, as `namespace|source-text`.
    fn visited(source: &str, scopes: Scopes) -> Vec<String> {
        let result = ruby_prism::parse(source.as_bytes());
        let program = result.node().as_program_node().expect("parses");
        let mut seen = Vec::new();

        walk_program(
            &program,
            source.as_bytes(),
            scopes,
            &mut |node, namespace| {
                let text = node_source_slice(source.as_bytes(), node);
                let first_line = text.lines().next().unwrap_or_default().to_string();
                seen.push(format!("{namespace}|{first_line}"));
                Flow::Continue
            },
        );

        seen
    }

    #[test]
    fn declarative_scopes_skip_def_bodies() {
        let source = "class Foo\n  def bar\n    inner_call\n  end\nend\n";

        let declarative = visited(source, Scopes::DECLARATIVE);
        assert!(
            !declarative.iter().any(|s| s.ends_with("|inner_call")),
            "a `def` body should not be walked, got {declarative:?}"
        );

        let all = visited(source, Scopes::ALL);
        assert!(
            all.iter().any(|s| s == "Foo|inner_call"),
            "`Scopes::ALL` should reach it, got {all:?}"
        );
    }

    #[test]
    fn namespace_accumulates_through_nesting() {
        let source = "module Outer\n  class Inner\n    marker\n  end\nend\n";
        let seen = visited(source, Scopes::DECLARATIVE);

        assert!(seen.iter().any(|s| s == "|module Outer"), "{seen:?}");
        assert!(seen.iter().any(|s| s == "Outer|class Inner"), "{seen:?}");
        assert!(seen.iter().any(|s| s == "Outer::Inner|marker"), "{seen:?}");
    }

    #[test]
    fn a_compact_namespace_is_not_double_qualified() {
        let source = "module Foo::Bar\n  marker\nend\n";
        let seen = visited(source, Scopes::DECLARATIVE);
        assert!(seen.iter().any(|s| s == "Foo::Bar|marker"), "{seen:?}");
    }

    #[test]
    fn an_absolute_path_resets_the_namespace() {
        let source = "module Outer\n  class ::Root\n    marker\n  end\nend\n";
        let seen = visited(source, Scopes::DECLARATIVE);
        assert!(seen.iter().any(|s| s == "Root|marker"), "{seen:?}");
    }

    #[test]
    fn singleton_class_bodies_are_walked_only_with_all_scopes() {
        let source = "class Foo\n  class << self\n    marker\n  end\nend\n";

        assert!(
            !visited(source, Scopes::DECLARATIVE)
                .iter()
                .any(|s| s.ends_with("|marker"))
        );
        assert!(
            visited(source, Scopes::ALL)
                .iter()
                .any(|s| s.ends_with("|marker"))
        );
    }

    #[test]
    fn stop_ends_the_whole_walk() {
        let source = "first\nsecond\nthird\n";
        let result = ruby_prism::parse(source.as_bytes());
        let program = result.node().as_program_node().expect("parses");
        let mut seen = Vec::new();

        walk_program(&program, source.as_bytes(), Scopes::ALL, &mut |node, _| {
            seen.push(node_source_slice(source.as_bytes(), node));
            if seen.len() == 2 {
                Flow::Stop
            } else {
                Flow::Continue
            }
        });

        assert_eq!(
            seen,
            vec!["first", "second"],
            "walk should stop at `second`"
        );
    }
}
