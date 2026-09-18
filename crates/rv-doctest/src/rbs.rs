//! RBS signature environment for arity checking.
//!
//! Loads `.rbs` files from directories and builds an index of
//! class-to-method signatures used to validate doctest snippets.

use std::collections::HashMap;
use std::path::Path;

use ruby_rbs::node::{
    ClassNode, MethodDefinitionNode, ModuleNode, Node, NodeList, SymbolNode, TypeNameNode, parse,
};

use crate::CheckError;

/// A parsed method signature from RBS, holding the arity information
/// needed for validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodSig {
    pub name: String,
    pub required_args: usize,
    pub optional_args: usize,
    pub has_rest: bool,
    pub has_block: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArityResult {
    Valid,
    TooFew { expected: usize, found: usize },
    TooMany { expected: usize, found: usize },
    MissingBlock,
}

#[derive(Debug, Default)]
pub struct RbsEnvironment {
    class_methods: HashMap<String, HashMap<String, MethodSig>>,
}

impl RbsEnvironment {
    /// Load all `.rbs` files under the given directories and build the
    /// signature index.
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

    /// Look up a method signature by fully qualified class path and method name.
    pub fn lookup(&self, class_path: &str, method: &str) -> Option<&MethodSig> {
        self.class_methods.get(class_path)?.get(method)
    }

    /// Whether the environment has any signatures for a class path.
    pub fn has_class(&self, class_path: &str) -> bool {
        self.class_methods.contains_key(class_path)
    }

    /// Check whether `arg_count` positional arguments are valid for the method.
    /// Unknown classes or methods return `Valid` (permissive by design).
    pub fn check_arity(
        &self,
        class_path: &str,
        method: &str,
        arg_count: usize,
        _has_block: bool,
    ) -> ArityResult {
        let Some(sig) = self.lookup(class_path, method) else {
            return ArityResult::Valid;
        };

        let max_args = if sig.has_rest {
            usize::MAX
        } else {
            sig.required_args + sig.optional_args
        };

        if arg_count < sig.required_args {
            ArityResult::TooFew {
                expected: sig.required_args,
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

fn load_file(
    path: &Path,
    index: &mut HashMap<String, HashMap<String, MethodSig>>,
) -> Result<(), CheckError> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| CheckError::Rbs(format!("failed to read {}: {e}", path.display())))?;

    let signature = parse(&content)
        .map_err(|e| CheckError::Rbs(format!("failed to parse {}: {e}", path.display())))?;

    for node in signature.declarations().iter() {
        walk_node(&node, "", index);
    }

    Ok(())
}

/// Render a `TypeNameNode` (e.g. `Foo::Bar`) as a string.
fn type_name_string(name: &TypeNameNode) -> String {
    let mut parts: Vec<String> = name
        .namespace()
        .path()
        .iter()
        .map(|n| match n {
            Node::Symbol(sym) => sym.to_string(),
            _ => String::new(),
        })
        .filter(|s| !s.is_empty())
        .collect();
    parts.push(name.name().to_string());
    parts.join("::")
}

fn walk_node(
    node: &Node,
    parent_path: &str,
    index: &mut HashMap<String, HashMap<String, MethodSig>>,
) {
    match node {
        Node::Class(class_node) => walk_type_node(
            &class_node.name(),
            &class_node.members(),
            parent_path,
            index,
        ),
        Node::Module(module_node) => walk_type_node(
            &module_node.name(),
            &module_node.members(),
            parent_path,
            index,
        ),
        _ => {}
    }
}

fn walk_type_node(
    name: &TypeNameNode,
    members: &NodeList,
    parent_path: &str,
    index: &mut HashMap<String, HashMap<String, MethodSig>>,
) {
    let name = type_name_string(name);
    let full_path = if parent_path.is_empty() {
        name
    } else {
        format!("{parent_path}::{name}")
    };

    let sigs = extract_method_sigs(members);
    if !sigs.is_empty() {
        index.insert(full_path.clone(), sigs);
    }

    for member_node in members.iter() {
        walk_node(&member_node, &full_path, index);
    }
}

/// Extract method signatures from a class/module member list.
/// Only the first overload of each method is kept.
fn extract_method_sigs(members: &NodeList) -> HashMap<String, MethodSig> {
    let mut sigs = HashMap::new();

    for member_node in members.iter() {
        match &member_node {
            Node::MethodDefinition(method) => insert_method(method, &mut sigs),
            Node::AttrReader(attr) => {
                let name = symbol_string(&attr.name());
                sigs.entry(name.clone()).or_insert(MethodSig {
                    name,
                    required_args: 0,
                    optional_args: 0,
                    has_rest: false,
                    has_block: false,
                });
            }
            Node::AttrAccessor(attr) => {
                let name = symbol_string(&attr.name());
                sigs.entry(name.clone()).or_insert(MethodSig {
                    name,
                    required_args: 0,
                    optional_args: 0,
                    has_rest: false,
                    has_block: false,
                });
            }
            Node::AttrWriter(attr) => {
                let name = symbol_string(&attr.name());
                sigs.entry(name.clone()).or_insert(MethodSig {
                    name,
                    required_args: 1,
                    optional_args: 0,
                    has_rest: false,
                    has_block: false,
                });
            }
            Node::Alias(alias) => {
                let old_name = symbol_string(&alias.old_name());
                if let Some(sig) = sigs.get(&old_name) {
                    let new_name = symbol_string(&alias.new_name());
                    sigs.insert(new_name, sig.clone());
                }
            }
            _ => {}
        }
    }

    sigs
}

fn symbol_string(sym: &SymbolNode) -> String {
    sym.to_string()
}

fn insert_method(method: &MethodDefinitionNode, sigs: &mut HashMap<String, MethodSig>) {
    let name = symbol_string(&method.name());

    // First overload is treated as the canonical signature.
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

#[cfg(test)]
mod tests;
