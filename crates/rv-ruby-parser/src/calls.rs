//! Method call extraction from Ruby source using Prism AST.

use ruby_prism::Node;

use super::{LineIndex, node_source_slice};

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
    /// 0-based line number in the source.
    pub line: usize,
}

pub fn parse_calls(source: &[u8]) -> Vec<CallSite> {
    let result = ruby_prism::parse(source);

    if result.errors().next().is_some() {
        return Vec::new();
    }

    let lines = LineIndex::new(source);
    let mut calls = Vec::new();
    walk_stmts(&result.node(), &lines, source, &mut calls);
    calls
}

/// Returns the 1-based line number for the first `require 'minitest*'` or
/// `require "minitest*"` call found.
pub fn minitest_require_line(source: &[u8]) -> Option<usize> {
    let result = ruby_prism::parse(source);

    if result.errors().next().is_some() {
        return None;
    }

    let lines = LineIndex::new(source);
    find_minitest_require_line(&result.node(), &lines, source)
}

fn find_minitest_require_line(node: &Node<'_>, lines: &LineIndex, source: &[u8]) -> Option<usize> {
    if let Some(call) = node.as_call_node()
        && call.name().as_slice() == b"require"
        && let Some(arg_str) = get_first_string_arg(&call, source)
        && arg_str.starts_with("minitest")
    {
        let location = call.location();
        let (start_line, _) = lines.line_col(location.start_offset());
        return Some(start_line as usize);
    }

    if let Some(stmts) = node.as_statements_node() {
        for stmt in stmts.body().iter() {
            if let Some(line) = process_stmt_for_minitest(&stmt, lines, source) {
                return Some(line);
            }
        }
    }

    None
}

fn process_stmt_for_minitest(node: &Node<'_>, lines: &LineIndex, source: &[u8]) -> Option<usize> {
    if let Some(call) = node.as_call_node() {
        if call.name().as_slice() == b"require" {
            if let Some(arg_str) = get_first_string_arg(&call, source) {
                if arg_str.starts_with("minitest") {
                    let location = call.location();
                    let (start_line, _) = lines.line_col(location.start_offset());
                    return Some(start_line as usize);
                }
            }
        }
    }

    if let Some(stmts) = node.as_statements_node() {
        for stmt in stmts.body().iter() {
            if let Some(line) = process_stmt_for_minitest(&stmt, lines, source) {
                return Some(line);
            }
        }
    }

    if let Some(body) = node.as_class_node().and_then(|n| n.body()) {
        for stmt in body
            .as_statements_node()
            .map(|s| s.body().iter())
            .into_iter()
            .flatten()
        {
            if let Some(line) = process_stmt_for_minitest(&stmt, lines, source) {
                return Some(line);
            }
        }
    }
    if let Some(body) = node.as_module_node().and_then(|n| n.body()) {
        for stmt in body
            .as_statements_node()
            .map(|s| s.body().iter())
            .into_iter()
            .flatten()
        {
            if let Some(line) = process_stmt_for_minitest(&stmt, lines, source) {
                return Some(line);
            }
        }
    }
    if let Some(body) = node.as_def_node().and_then(|n| n.body()) {
        for stmt in body
            .as_statements_node()
            .map(|s| s.body().iter())
            .into_iter()
            .flatten()
        {
            if let Some(line) = process_stmt_for_minitest(&stmt, lines, source) {
                return Some(line);
            }
        }
    }

    None
}

fn get_first_string_arg(call: &ruby_prism::CallNode<'_>, source: &[u8]) -> Option<String> {
    let args = call.arguments()?;
    for arg in args.arguments().iter() {
        let loc = arg.location();
        let slice = &source[loc.start_offset()..loc.end_offset()];
        let s = String::from_utf8_lossy(slice);
        
        // Check if it's a double-quoted or single-quoted string
        if s.len() >= 2 {
            let first = s.chars().next()?;
            let last = s.chars().last()?;
            if (first == '"' && last == '"') || (first == '\'' && last == '\'') {
                return Some(s[1..s.len()-1].to_string());
            }
        }
    }
    None
}

fn walk_stmts(node: &Node<'_>, lines: &LineIndex, source: &[u8], calls: &mut Vec<CallSite>) {
    let stmts = match node.as_statements_node() {
        Some(s) => s,
        None => return,
    };

    for stmt in stmts.body().iter() {
        process_stmt(&stmt, lines, source, calls);
    }
}

fn process_stmt(node: &Node<'_>, lines: &LineIndex, source: &[u8], calls: &mut Vec<CallSite>) {
    if let Some(call) = node.as_call_node()
        && let Some(call_site) = extract_call_site(&call, lines, source)
    {
        calls.push(call_site);
    }
}

fn extract_call_site(
    call: &ruby_prism::CallNode<'_>,
    lines: &LineIndex,
    source: &[u8],
) -> Option<CallSite> {
    let location = call.location();
    let line = location.start_offset();
    let (line_num, _) = lines.line_col(location.start_offset());

    let name = String::from_utf8_lossy(call.name().as_slice()).into_owned();
    let arguments = call.arguments();

    let has_parens = arguments.map(|a| a.parens()).unwrap_or(false);
    let arg_count = arguments.map(|a| a.arguments().len()).unwrap_or(0);

    let (class_name, receiver_text) = if let Some(receiver) = call.receiver() {
        extract_receiver_info(receiver, source)
    } else {
        (None, None)
    };

    let has_block = arguments.map(|a| a.block().is_some()).unwrap_or(false);

    Some(CallSite {
        class_name,
        receiver_text,
        name,
        arg_count,
        has_block,
        line: line_num as usize - 1,
    })
}

fn extract_receiver_info(receiver: &Node<'_>, source: &[u8]) -> (Option<String>, Option<String>) {
    let receiver_text = node_source_slice(source, receiver);

    if let Some(const_path) = receiver.as_constant_path_node() {
        let class_name = extract_full_constant_path(&const_path);
        return (Some(class_name.clone()), Some(class_name));
    }

    if let Some(const_read) = receiver.as_constant_read_node() {
        let name = const_read.name();
        let name_bytes = name.as_slice();
        let class_name = String::from_utf8_lossy(name_bytes).into_owned();
        return (Some(class_name.clone()), Some(class_name));
    }

    if matches!(receiver, Node::SelfNode { .. }) {
        return (None, Some("self".to_string()));
    }
    if matches!(receiver, Node::InstanceVariableReadNode { .. })
        || matches!(receiver, Node::GlobalVariableReadNode { .. })
    {
        return (None, Some(receiver_text));
    }

    (None, Some(receiver_text))
}

fn extract_full_constant_path(const_path: &ruby_prism::ConstantPathNode<'_>) -> String {
    let mut parts: Vec<String> = Vec::new();

    if let Some(nested) = const_path.nested() {
        let nested_name = nested.name();
        parts.push(String::from_utf8_lossy(nested_name.as_slice()).into_owned());
    }

    if let Some(const_read) = const_path.constant() {
        let name = const_read.name();
        let name_bytes = name.as_slice();
        let name_str = String::from_utf8_lossy(name_bytes).into_owned();
        parts.insert(0, name_str);
    }

    parts.join("::")
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;

    #[test]
    fn parse_calls_simple_method_call() {
        let source = b"foo(1, 2)\n";
        let calls = parse_calls(source);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "foo");
        assert_eq!(calls[0].arg_count, 2);
    }

    #[test]
    fn parse_calls_with_block() {
        let source = b"foo(1) { |x| x }\n";
        let calls = parse_calls(source);
        assert_eq!(calls.len(), 1);
        assert!(calls[0].has_block);
    }

    #[test]
    fn parse_calls_no_parens() {
        let source = b"foo 1, 2\n";
        let calls = parse_calls(source);
        assert_eq!(calls.len(), 1);
        assert!(!calls[0].has_parens);
    }

    #[test]
    fn parse_calls_syntax_error() {
        let source = b"def foo(\n";
        let calls = parse_calls(source);
        assert!(calls.is_empty());
    }

    #[test]
    fn parse_calls_chained() {
        let source = b"user.update!(\n  role: :admin,\n).save\n";
        let calls = parse_calls(source);
        assert!(calls.len() >= 2);
    }

    #[test]
    fn minitest_require_single_quotes() {
        let source = indoc! {"
            require 'minitest'
            def test
            end
        "};
        let line = minitest_require_line(source.as_bytes());
        assert_eq!(line, Some(1), "found require on line 1");
    }

    #[test]
    fn minitest_require_double_quotes() {
        let source = indoc! {"
            require \"minitest\"
            def test
            end
        "};
        let line = minitest_require_line(source.as_bytes());
        assert_eq!(line, Some(1), "found require on line 1");
    }

    #[test]
    fn minitest_require_with_subpath() {
        let source = indoc! {"
            require 'minitest/autorun'
            def test
            end
        "};
        let line = minitest_require_line(source.as_bytes());
        assert_eq!(line, Some(1), "found require on line 1");
    }

    #[test]
    fn minitest_require_in_class() {
        let source = indoc! {"
            class Foo
              require 'minitest'
              def self.test
              end
            end
        "};
        let line = minitest_require_line(source.as_bytes());
        assert_eq!(line, Some(2), "found require on line 2");
    }

    #[test]
    fn minitest_require_after_other_code() {
        let source = indoc! {"
            def foo
              1
            end
            require 'minitest'
            def bar
            end
        "};
        let line = minitest_require_line(source.as_bytes());
        assert_eq!(line, Some(3), "found require on line 3");
    }

    #[test]
    fn no_minitest_require() {
        let source = indoc! {"
            require 'json'
            require 'rails'
        "};
        let line = minitest_require_line(source.as_bytes());
        assert_eq!(line, None, "no minitest require found");
    }

    #[test]
    fn minitest_require_none_syntax_error() {
        let source = "def foo(\n";
        let line = minitest_require_line(source.as_bytes());
        assert_eq!(line, None, "syntax error returns None");
    }
}
