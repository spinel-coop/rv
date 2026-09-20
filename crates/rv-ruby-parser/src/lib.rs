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
pub(crate) mod visitor;

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

/// One parse of a Ruby source buffer, reusable by every consumer in this
/// crate.
///
/// [`parse`], [`calls::parse_calls`] and [`calls::minitest_require`] each used
/// to run their own [`ruby_prism::parse`], so a caller wanting two of them paid
/// for two parses of the same bytes. Build one of these instead and ask it for
/// each result.
///
/// Prism's tree borrows the source buffer, so this handle cannot be stored
/// alongside owned bytes or held across an `.await`. Construct it in a scope
/// that borrows the source, take the owned results you need, and drop it.
pub struct ParsedSource<'src> {
    result: ruby_prism::ParseResult<'src>,
    source: &'src [u8],
    lines: LineIndex,
}

impl<'src> ParsedSource<'src> {
    /// Parse `source` once.
    pub fn new(source: &'src [u8]) -> Self {
        Self {
            result: ruby_prism::parse(source),
            source,
            lines: LineIndex::new(source),
        }
    }

    /// `true` when the source parsed without errors.
    pub fn is_success(&self) -> bool {
        self.result.errors().next().is_none()
    }

    /// Every definition in the source, in source order, with leading doc
    /// comments attached.
    pub fn items(&self) -> Vec<Item> {
        let Some(program) = self.result.node().as_program_node() else {
            return Vec::new();
        };

        let mut items = collect_items(&program, self.source, &self.lines);
        self.attach_comments(&mut items);
        items
    }

    /// Every method call in the source.
    pub fn calls(&self) -> Vec<calls::CallSite> {
        calls::collect_calls(self)
    }

    /// The first `require` of minitest, wherever it is nested.
    pub fn minitest_require(&self) -> Option<calls::MinitestRequire> {
        calls::find_minitest_require(self)
    }

    /// Parse errors, if any.
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        self.result
            .errors()
            .map(|diagnostic| {
                let (line, column) = self.lines.line_col(diagnostic.location().start_offset());
                Diagnostic {
                    message: diagnostic.message().to_string(),
                    line,
                    column,
                }
            })
            .collect()
    }

    /// The owned snapshot returned by [`parse`].
    pub fn to_parsed_file(&self) -> ParsedFile {
        ParsedFile {
            items: self.items(),
            diagnostics: self.diagnostics(),
        }
    }

    pub(crate) fn program(&self) -> Option<ruby_prism::ProgramNode<'_>> {
        self.result.node().as_program_node()
    }

    pub(crate) fn source(&self) -> &'src [u8] {
        self.source
    }

    pub(crate) fn lines(&self) -> &LineIndex {
        &self.lines
    }

    /// Attach each item's contiguous run of leading `#` comment lines.
    fn attach_comments(&self, items: &mut [Item]) {
        let mut comments_by_end_line: HashMap<u32, String> = HashMap::new();
        let usable_comments = self
            .result
            .comments()
            .filter(|com| com.type_() == CommentType::InlineComment);
        for comment in usable_comments {
            let loc = comment.location();
            let start = loc.start_offset();
            let (end_line, _) = self.lines.line_col(loc.end_offset());
            if is_standalone_comment(self.source, &self.lines, start) {
                comments_by_end_line.insert(end_line, {
                    let text = String::from_utf8_lossy(comment.text());
                    let stripped = text.trim_start_matches('#');
                    stripped.strip_prefix(' ').unwrap_or(stripped).to_string()
                });
            }
        }

        for item in items {
            let mut attached = Vec::new();
            let mut cursor = item.span.start_line;
            while cursor > 1 {
                cursor -= 1;
                match comments_by_end_line.get(&cursor) {
                    Some(text) => attached.insert(0, text.clone()),
                    None => break,
                }
            }
            item.comments = attached;
        }
    }
}

/// Parses Ruby source and returns an owned index of definitions and
/// diagnostics.
///
/// Use [`ParsedSource`] directly when more than one kind of result is needed
/// from the same bytes, so they share a single parse.
pub fn parse(source: &[u8]) -> ParsedFile {
    ParsedSource::new(source).to_parsed_file()
}

/// Collects every `def`/`class`/`module` in `program`, in source order.
///
/// Descends into `class` and `module` bodies only: definitions inside a `def`
/// body are not members of the enclosing namespace.
fn collect_items(
    program: &ruby_prism::ProgramNode<'_>,
    source: &[u8],
    lines: &LineIndex,
) -> Vec<Item> {
    let mut items = Vec::new();

    visitor::walk_program(
        program,
        source,
        visitor::Scopes::DECLARATIVE,
        &mut |node, namespace| {
            if let Some(def) = node.as_def_node() {
                let start = def.def_keyword_loc().start_offset();
                let end = def.location().end_offset();
                let bytes: &[u8] = def.name().as_slice();
                let name = String::from_utf8_lossy(bytes).into_owned();
                let singleton = def.receiver().is_some();
                let full_path = if namespace.is_empty() {
                    name.clone()
                } else {
                    // `Foo.bar` for a singleton method, `Foo#bar` for an instance one.
                    let separator = if singleton { '.' } else { '#' };
                    format!("{namespace}{separator}{name}")
                };
                items.push(Item::Def(DefItem {
                    common: CommonItem {
                        name,
                        full_path,
                        span: make_span(lines, start, end),
                        comments: Vec::new(),
                    },
                    singleton,
                }));
            } else if let Some(class) = node.as_class_node() {
                let start = class.class_keyword_loc().start_offset();
                let end = class.location().end_offset();
                let bytes: &[u8] = class.name().as_slice();
                let name = String::from_utf8_lossy(bytes).into_owned();
                let declared = node_source_slice(source, &class.constant_path());
                items.push(Item::Class(ClassItem {
                    common: CommonItem {
                        name,
                        full_path: visitor::qualify_constant(namespace, &declared),
                        span: make_span(lines, start, end),
                        comments: Vec::new(),
                    },
                    superclass: class
                        .superclass()
                        .map(|node| node_source_slice(source, &node)),
                }));
            } else if let Some(module) = node.as_module_node() {
                let start = module.module_keyword_loc().start_offset();
                let end = module.location().end_offset();
                let bytes: &[u8] = module.name().as_slice();
                let name = String::from_utf8_lossy(bytes).into_owned();
                let declared = node_source_slice(source, &module.constant_path());
                items.push(Item::Module(ModuleItem(CommonItem {
                    name,
                    full_path: visitor::qualify_constant(namespace, &declared),
                    span: make_span(lines, start, end),
                    comments: Vec::new(),
                })));
            }

            visitor::Flow::Continue
        },
    );

    items
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
pub(crate) fn node_source_slice(source: &[u8], node: &Node<'_>) -> String {
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
