//! Method call extraction from Ruby source using Prism AST.

use ruby_prism::Node;

use super::{LineIndex, ParsedSource, node_source_slice, visitor};

/// A method call extracted from Ruby source code.
#[derive(Debug, Clone, PartialEq)]
pub struct CallSite {
    /// Resolved class constant if the receiver is a class constant.
    pub class_name: Option<String>,
    /// Raw receiver text for reporting.
    pub receiver_text: Option<String>,
    /// Method name (possibly with `!` or `?` suffix).
    pub name: String,
    /// Number of positional arguments.
    pub arg_count: usize,
    /// Whether a block was passed.
    pub has_block: bool,
    /// 1-based line number in the source.
    pub line: usize,
}

/// Extracts every method call from an already-parsed source.
pub(crate) fn collect_calls(parsed: &ParsedSource<'_>) -> Vec<CallSite> {
    if !parsed.is_success() {
        return Vec::new();
    }

    let Some(program) = parsed.program() else {
        return Vec::new();
    };

    let source = parsed.source();
    let lines = parsed.lines();
    let mut calls = Vec::new();

    visitor::walk_program(&program, source, visitor::Scopes::ALL, &mut |node, _| {
        if let Some(call) = node.as_call_node()
            && let Some(call_site) = extract_call_site(&call, lines, source)
        {
            calls.push(call_site);
        }
        visitor::Flow::Continue
    });

    calls
}

/// Extracts every method call from `source`.
///
/// Parses `source`; build a [`ParsedSource`] instead when other results are
/// wanted from the same bytes.
pub fn parse_calls(source: &[u8]) -> Vec<CallSite> {
    collect_calls(&ParsedSource::new(source))
}

/// A `require` of minitest found in a snippet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MinitestRequire {
    /// 1-based line of the `require` itself.
    pub require_line: usize,
    /// 1-based last line of the *top-level* statement containing the require.
    ///
    /// An assertion preamble must be inserted after this line rather than
    /// after `require_line`: the `require` may sit inside a `class`, `module`,
    /// or `def` body, and a preamble injected there would apply to that body
    /// instead of `main`.
    pub top_level_end_line: usize,
}

/// Finds the first minitest `require` in an already-parsed source.
pub(crate) fn find_minitest_require(parsed: &ParsedSource<'_>) -> Option<MinitestRequire> {
    if !parsed.is_success() {
        return None;
    }

    let program = parsed.program()?;
    let source = parsed.source();
    let lines = parsed.lines();

    for statement in program.statements().body().iter() {
        let mut require_line = None;

        // Only scopes that execute where they are written: a `class` or
        // `module` body runs when the definition is evaluated, so a `require`
        // inside one has taken effect by the time this top-level statement
        // finishes. A `def` body does not run until the method is called.
        visitor::walk_node(
            &statement,
            source,
            visitor::Scopes::DECLARATIVE,
            &mut |node, _| {
                let Some(call) = node.as_call_node() else {
                    return visitor::Flow::Continue;
                };
                if call.name().as_slice() != b"require" {
                    return visitor::Flow::Continue;
                }
                if !get_first_string_arg(&call).is_some_and(|arg| arg.starts_with("minitest")) {
                    return visitor::Flow::Continue;
                }

                let (line, _) = lines.line_col(call.location().start_offset());
                require_line = Some(line as usize);
                visitor::Flow::Stop
            },
        );

        if let Some(require_line) = require_line {
            let (end_line, _) = lines.line_col(statement.location().end_offset());
            return Some(MinitestRequire {
                require_line,
                top_level_end_line: end_line as usize,
            });
        }
    }

    None
}

/// Finds the first `require 'minitest*'` / `require "minitest*"` in `source`,
/// wherever it is nested, along with the top-level statement that contains it.
///
/// Parses `source`; build a [`ParsedSource`] instead when other results are
/// wanted from the same bytes.
pub fn minitest_require(source: &[u8]) -> Option<MinitestRequire> {
    find_minitest_require(&ParsedSource::new(source))
}

/// Returns the 1-based line number for the first `require 'minitest*'` or
/// `require "minitest*"` call found.
pub fn minitest_require_line(source: &[u8]) -> Option<usize> {
    minitest_require(source).map(|found| found.require_line)
}

fn get_first_string_arg(call: &ruby_prism::CallNode<'_>) -> Option<String> {
    let args = call.arguments()?;
    for arg in args.arguments().iter() {
        if let Some(string_node) = arg.as_string_node() {
            let bytes: &[u8] = string_node.unescaped();
            return Some(String::from_utf8_lossy(bytes).into_owned());
        }
    }
    None
}

fn extract_call_site(
    call: &ruby_prism::CallNode<'_>,
    lines: &LineIndex,
    source: &[u8],
) -> Option<CallSite> {
    let location = call.location();
    let (start_line, _) = lines.line_col(location.start_offset());

    let name_bytes: &[u8] = call.name().as_slice();
    let name = String::from_utf8_lossy(name_bytes).into_owned();

    let args = call.arguments();
    let arg_count = args.map(|a| a.arguments().iter().count()).unwrap_or(0);

    let has_block = call.block().is_some();

    let receiver_node = call.receiver();
    let (class_name, receiver_text) = if let Some(r) = receiver_node {
        extract_receiver_info(&r, source)
    } else {
        (None, None)
    };

    Some(CallSite {
        class_name,
        receiver_text,
        name,
        arg_count,
        has_block,
        line: start_line as usize,
    })
}

fn extract_receiver_info(receiver: &Node<'_>, source: &[u8]) -> (Option<String>, Option<String>) {
    let receiver_text = node_source_slice(source, receiver);

    if let Some(const_path) = receiver.as_constant_path_node() {
        return match extract_full_constant_path(&const_path) {
            Some(class_name) => (Some(class_name.clone()), Some(class_name)),
            // A constant path we cannot name (no constant parts at all); keep
            // the raw text for reporting but do not claim a class.
            None => (None, Some(receiver_text)),
        };
    }

    if let Some(const_read) = receiver.as_constant_read_node() {
        let name = const_read.name();
        let name_bytes = name.as_slice();
        let class_name = String::from_utf8_lossy(name_bytes).into_owned();
        return (Some(class_name.clone()), Some(class_name));
    }

    if let ruby_prism::Node::SelfNode { .. } = receiver {
        return (None, Some("self".to_string()));
    }
    if let ruby_prism::Node::InstanceVariableReadNode { .. } = receiver {
        return (None, Some(receiver_text));
    }
    if let ruby_prism::Node::GlobalVariableReadNode { .. } = receiver {
        return (None, Some(receiver_text));
    }

    (None, Some(receiver_text))
}

/// Join a constant path node into a `Foo::Bar::Baz` string.
///
/// Returns `None` when no constant parts can be recovered. A root-scoped path
/// such as `::Foo::Bar` has no parent on its outermost node and resolves to
/// `Foo::Bar`, matching how the constant is named in an RBS signature.
fn extract_full_constant_path(const_path: &ruby_prism::ConstantPathNode<'_>) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();

    fn collect_parts(node: ruby_prism::Node<'_>, parts: &mut Vec<String>) {
        if let Some(cp) = node.as_constant_path_node() {
            if let Some(parent) = cp.parent() {
                collect_parts(parent, parts);
            }
            if let Some(name) = cp.name() {
                parts.push(String::from_utf8_lossy(name.as_slice()).into_owned());
            }
        } else if let Some(cr) = node.as_constant_read_node() {
            let name = cr.name();
            parts.push(String::from_utf8_lossy(name.as_slice()).into_owned());
        }
    }

    if let Some(parent) = const_path.parent() {
        collect_parts(parent, &mut parts);
    }
    if let Some(name) = const_path.name() {
        parts.push(String::from_utf8_lossy(name.as_slice()).into_owned());
    }

    (!parts.is_empty()).then(|| parts.join("::"))
}
