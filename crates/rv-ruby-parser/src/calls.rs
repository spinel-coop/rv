//! Method call extraction from Ruby source using Prism AST.

use ruby_prism::Node;

use super::{node_source_slice, LineIndex};

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
        && let Some(call_site) = extract_call_site(&call, lines, source) {
            calls.push(call_site);
        }

    // Handle nested statements
    if let Some(stmts) = node.as_statements_node() {
        for stmt in stmts.body().iter() {
            process_stmt(&stmt, lines, source, calls);
        }
    }
    
    // Handle nested bodies (class/module/def)
    if let Some(class) = node.as_class_node() {
        if let Some(body) = class.body() {
            process_statements_from_body(body, lines, source, calls);
        }
    }
    if let Some(module) = node.as_module_node() {
        if let Some(body) = module.body() {
            process_statements_from_body(body, lines, source, calls);
        }
    }
    if let Some(def) = node.as_def_node() {
        if let Some(body) = def.body() {
            process_statements_from_body(body, lines, source, calls);
        }
    }
    if let Some(singleton) = node.as_singleton_class_node() {
        if let Some(body) = singleton.body() {
            process_statements_from_body(body, lines, source, calls);
        }
    }
}

fn process_statements_from_body(
    body: Node<'_>,
    lines: &LineIndex,
    source: &[u8],
    calls: &mut Vec<CallSite>,
) {
    if let Some(stmts) = body.as_statements_node() {
        for stmt in stmts.body().iter() {
            process_stmt(&stmt, lines, source, calls);
        }
    }
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
        let class_name = extract_full_constant_path(&const_path);
        return (Some(class_name.clone()), Some(class_name));
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

fn extract_full_constant_path(const_path: &ruby_prism::ConstantPathNode<'_>) -> String {
    let mut parts: Vec<String> = Vec::new();

    fn collect_parts(node: ruby_prism::Node<'_>, parts: &mut Vec<String>) {
        if let Some(cp) = node.as_constant_path_node() {
            collect_parts(cp.parent().unwrap(), parts);
            if let Some(name) = cp.name() {
                parts.push(String::from_utf8_lossy(name.as_slice()).into_owned());
            }
        } else if let Some(cr) = node.as_constant_read_node() {
            let name = cr.name();
            parts.push(String::from_utf8_lossy(name.as_slice()).into_owned());
        }
    }

    collect_parts(const_path.parent().unwrap(), &mut parts);
    if let Some(name) = const_path.name() {
        parts.push(String::from_utf8_lossy(name.as_slice()).into_owned());
    }
    parts.join("::")
}