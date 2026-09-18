//! A minimal source model for Ruby files, built on top of the `ruby-prism`
//! parser.
//!
//! This crate is a exploration of `ruby_prism`: it wraps [`ruby_prism::parse`] and produces an eager,
//! fully-owned index of definitions (`def`/`class`/`module`) plus any parse
//! diagnostics. It also demonstrates comment-to-definition attachment: leading
//! doc comments (contiguous `#` lines immediately above a definition) are
//! attached to that definition.
//!
//! The API returns owned versions of the `ruby_prism` values, so callers never need to
//! worry about lifetimes beyond the single [`parse`] call.

use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

use ruby_prism::{CommentType, Node};

pub mod calls;

/// A byte-offset-based line index over the source.
///
/// Prism's Rust `Location` type exposes byte offsets but not line/column
/// numbers, so we compute them ourselves.
pub(crate) struct LineIndex {
    /// Byte offset of the start of each line (0-based line number).
    line_starts: Vec<usize>,
}

impl LineIndex {
    pub(crate) fn new(source: &[u8]) -> Self {
        let mut line_starts = vec![0usize];
        for (i, &b) in source.iter().enumerate() {
            if b == b'\n' {
                line_starts.push(i + 1);
            }
        }
        Self { line_starts }
    }

    /// Returns the byte offset where the given 1-based line starts.
    fn line_start(&self, line: u32) -> usize {
        self.line_starts[(line - 1) as usize]
    }

    /// Returns the 1-based (line, column) of the given byte offset.
    pub(crate) fn line_col(&self, offset: usize) -> (u32, u32) {
        let line = match self.line_starts.binary_search(&offset) {
            Ok(l) => l,
            Err(l) => l - 1,
        };
        let col = offset - self.line_starts[line];
        (line as u32 + 1, col as u32 + 1)
    }
}

/// A span in the source file, with 1-based line/column numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub start_offset: usize,
    pub end_offset: usize,
}

/// The kind of an [`Item`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    Class,
    Module,
    Def,
}

/// The fields shared by every [`Item`] variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonItem {
    /// The short name of the definition (`"foo"`, `"Foo"`, `"M"`).
    pub name: String,
    /// The full (constant-path) name.
    pub full_path: String,
    /// The span of the definition.
    pub span: Span,
    /// The leading doc comments attached to this definition.
    pub comments: Vec<String>,
}

/// A `class` definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassItem {
    common: CommonItem,
    /// The superclass's source text, if the class has one.
    pub superclass: Option<String>,
}

/// A `module` definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleItem(CommonItem);

/// A `def` (method) definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefItem {
    common: CommonItem,
    /// `true` for singleton methods (`def self.foo`).
    pub singleton: bool,
}

/// A definition found in a Ruby source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Class(ClassItem),
    Module(ModuleItem),
    Def(DefItem),
}

macro_rules! impl_deref {
    (field=$ty:ty) => {
        impl Deref for $ty {
            type Target = CommonItem;
            fn deref(&self) -> &CommonItem { &self.common }
        }
        impl DerefMut for $ty {
            fn deref_mut(&mut self) -> &mut CommonItem { &mut self.common }
        }
    };
    (tuple=$ty:ty) => {
        impl Deref for $ty {
            type Target = CommonItem;
            fn deref(&self) -> &CommonItem { &self.0 }
        }
        impl DerefMut for $ty {
            fn deref_mut(&mut self) -> &mut CommonItem { &mut self.0 }
        }
    };
    (enum $ty:ident { $($variant:ident),* $(,)? }) => {
        impl Deref for $ty {
            type Target = CommonItem;
            fn deref(&self) -> &CommonItem {
                match self {
                    $($ty::$variant(v) => v,)*
                }
            }
        }
        impl DerefMut for $ty {
            fn deref_mut(&mut self) -> &mut CommonItem {
                match self {
                    $($ty::$variant(v) => v,)*
                }
            }
        }
    };
}

impl_deref!(field = ClassItem);
impl_deref!(tuple = ModuleItem);
impl_deref!(field = DefItem);
impl_deref!(
    enum Item {
        Class,
        Module,
        Def,
    }
);

impl Item {
    pub fn kind(&self) -> ItemKind {
        match self {
            Item::Class(_) => ItemKind::Class,
            Item::Module(_) => ItemKind::Module,
            Item::Def(_) => ItemKind::Def,
        }
    }

    pub fn singleton(&self) -> bool {
        match self {
            Item::Def(def) => def.singleton,
            _ => false,
        }
    }

    pub fn superclass(&self) -> Option<&str> {
        match self {
            Item::Class(class) => class.superclass.as_deref(),
            _ => None,
        }
    }
}

/// An owned version of `ruby_prism`'s parse diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub message: String,
    pub line: u32,
    pub column: u32,
}

/// The result of parsing a Ruby source file.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParsedFile {
    /// All definitions found, in source order (depth-first).
    pub items: Vec<Item>,
    /// Parse errors, if any.
    pub diagnostics: Vec<Diagnostic>,
}

impl ParsedFile {
    /// `true` when the source parsed without errors.
    pub fn is_success(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

/// Parses Ruby source and returns an owned index of definitions and
/// diagnostics.
pub fn parse(source: &[u8]) -> ParsedFile {
    let result = ruby_prism::parse(source);
    let lines = LineIndex::new(source);

    // Collect inline comments with their end line, in source order.
    let mut comments_by_end_line: HashMap<u32, String> = HashMap::new();
    let usable_comments = result
        .comments()
        .filter(|com| com.type_() == CommentType::InlineComment);
    for comment in usable_comments {
        let loc = comment.location();
        let start = loc.start_offset();
        let (end_line, _) = lines.line_col(loc.end_offset());
        if is_standalone_comment(source, &lines, start) {
            comments_by_end_line.insert(end_line, {
                let text = String::from_utf8_lossy(comment.text());
                let stripped = text.trim_start_matches('#');
                stripped.strip_prefix(' ').unwrap_or(stripped).to_string()
            });
        }
    }

    // Walk the AST, collecting definitions.
    let mut items = Vec::new();
    let prog_node = result.node().as_program_node();
    if let Some(program) = prog_node {
        for statement in program.statements().body().iter() {
            walk_statement(&statement, source, &lines, &mut items);
        }
    }

    // Attach leading comments to each item.
    for item in &mut items {
        let start_line = item.span.start_line;
        let mut attached = Vec::new();
        let mut cursor = start_line;
        while cursor > 1 {
            cursor -= 1;
            match comments_by_end_line.get(&cursor) {
                Some(text) => attached.insert(0, text.clone()),
                None => break,
            }
        }
        item.comments = attached;
    }

    // Surface diagnostics.
    let mut diagnostics = Vec::new();
    for diagnostic in result.errors() {
        let loc = diagnostic.location();
        let (line, column) = lines.line_col(loc.start_offset());
        diagnostics.push(Diagnostic {
            message: diagnostic.message().to_string(),
            line,
            column,
        });
    }

    ParsedFile { items, diagnostics }
}

/// Recursively walks a statement, collecting `def`/`class`/`module` items.
fn walk_statement(node: &Node<'_>, source: &[u8], lines: &LineIndex, items: &mut Vec<Item>) {
    if let Some(def) = node.as_def_node() {
        let start = def.def_keyword_loc().start_offset();
        let end = def.location().end_offset();
        let bytes: &[u8] = def.name().as_slice();
        let name = String::from_utf8_lossy(bytes).into_owned();
        items.push(Item::Def(DefItem {
            common: CommonItem {
                name: name.clone(),
                full_path: name,
                span: make_span(lines, start, end),
                comments: Vec::new(),
            },
            singleton: def.receiver().is_some(),
        }));
    } else if let Some(class) = node.as_class_node() {
        let start = class.class_keyword_loc().start_offset();
        let end = class.location().end_offset();
        let bytes: &[u8] = class.name().as_slice();
        let name = String::from_utf8_lossy(bytes).into_owned();
        let full_name = node_source_slice(source, &class.constant_path());
        let superclass = class
            .superclass()
            .map(|node| node_source_slice(source, &node));
        items.push(Item::Class(ClassItem {
            common: CommonItem {
                name,
                full_path: full_name,
                span: make_span(lines, start, end),
                comments: Vec::new(),
            },
            superclass,
        }));
        if let Some(body) = class.body() {
            walk_statements(&body, source, lines, items);
        }
    } else if let Some(module) = node.as_module_node() {
        let start = module.module_keyword_loc().start_offset();
        let end = module.location().end_offset();
        let bytes: &[u8] = module.name().as_slice();
        let name = String::from_utf8_lossy(bytes).into_owned();
        let full_name = node_source_slice(source, &module.constant_path());
        items.push(Item::Module(ModuleItem(CommonItem {
            name,
            full_path: full_name,
            span: make_span(lines, start, end),
            comments: Vec::new(),
        })));
        if let Some(body) = module.body() {
            walk_statements(&body, source, lines, items);
        }
    }
}

/// Walks a body node (a `StatementsNode`) if it is one.
fn walk_statements(node: &Node<'_>, source: &[u8], lines: &LineIndex, items: &mut Vec<Item>) {
    if let Some(statements) = node.as_statements_node() {
        for statement in statements.body().iter() {
            walk_statement(&statement, source, lines, items);
        }
    }
}

fn make_span(lines: &LineIndex, start: usize, end: usize) -> Span {
    let (start_line, start_column) = lines.line_col(start);
    let (end_line, end_column) = lines.line_col(end);
    Span {
        start_line,
        start_column,
        end_line,
        end_column,
        start_offset: start,
        end_offset: end,
    }
}

/// Returns the source text of a node's location.
fn node_source_slice(source: &[u8], node: &Node<'_>) -> String {
    let loc = node.location();
    let bytes: &[u8] = &source[loc.start_offset()..loc.end_offset()];
    String::from_utf8_lossy(bytes).into_owned()
}

/// `true` when only whitespace precedes the comment on its line (i.e. it is a
/// standalone comment, not a trailing one).
fn is_standalone_comment(source: &[u8], lines: &LineIndex, comment_start: usize) -> bool {
    let (line, _) = lines.line_col(comment_start);
    let line_start = lines.line_start(line);
    source[line_start..comment_start]
        .iter()
        .all(u8::is_ascii_whitespace)
}

#[cfg(test)]
mod tests;
