//! Minimal YARD tag parser for comment blocks already extracted by the
//! Prism-based parser.
//!
//! Works on the `Vec<String>` comment lines stored in each [`Item`] — these
//! are already stripped of the `#` prefix and leading whitespace.  Parses
//! the subset of YARD tags useful for doctest: `@example`, `@param`,
//! `@return`, `@raise`, and `@type`.

/// Structured YARD documentation extracted from a comment block.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct YardDoc {
    /// Lines before the first `@`-tag — the free-form description.
    pub description: Vec<String>,
    /// `@example` code blocks.  Each entry is the raw example body with
    /// indentation normalised.
    pub examples: Vec<Example>,
    /// `@param [Type] name` entries.
    pub params: Vec<ParamTag>,
    /// `@return [Type]` entry, if present.
    pub returns: Option<String>,
    /// `@raise [Error]` entries.
    pub raises: Vec<String>,
    /// `@type [Type]` annotation, if present.
    pub type_annotation: Option<String>,
    /// Whether the comment block contained any recognised YARD tags.
    pub has_tags: bool,
}

/// A single `@example` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Example {
    /// Optional title after `@example`.
    pub title: Option<String>,
    /// The code body with consistent indentation removed.
    pub body: String,
}

/// A single `@param [Type] name` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamTag {
    pub name: String,
    pub r#type: String,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Parse YARD tags from the comment lines attached to a definition.
///
/// `comments` are the lines already stripped of `#` and leading whitespace
/// (exactly as stored in `CommonItem::comments`).
pub fn parse(comments: &[String]) -> YardDoc {
    let mut doc = YardDoc::default();
    let mut in_example = false;
    let mut example_title: Option<String> = None;
    let mut example_lines: Vec<String> = Vec::new();

    for line in comments {
        let trimmed = line.trim();
        let is_blank = trimmed.is_empty();

        // Flush any open @example block when we hit a new tag or end of
        // description section.
        if in_example && !is_blank && is_tag_line(trimmed) {
            doc.examples.push(Example {
                title: example_title.take(),
                body: deindent(&example_lines),
            });
            example_lines.clear();
            in_example = false;
        }

        if in_example {
            // Preserve original indentation for @example body.
            example_lines.push(line.clone());
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("@example") {
            doc.has_tags = true;
            in_example = true;
            example_title = rest.trim().is_empty().then_some(None).flatten();
            if !rest.trim().is_empty() {
                example_title = Some(rest.trim().to_string());
            }
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("@param ") {
            doc.has_tags = true;
            if let Some(pt) = parse_param(rest) {
                doc.params.push(pt);
            }
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("@return ") {
            doc.has_tags = true;
            doc.returns = extract_bracketed_type(rest);
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("@raise ") {
            doc.has_tags = true;
            if let Some(err) = extract_bracketed_type(rest) {
                doc.raises.push(err);
            } else {
                doc.raises.push(rest.trim().to_string());
            }
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("@type ") {
            doc.has_tags = true;
            doc.type_annotation = extract_bracketed_type(rest);
            continue;
        }

        // Any other @tag that we don't recognise — still counts as a tag.
        if trimmed.starts_with('@') {
            doc.has_tags = true;
            continue;
        }

        // Free-form description line (only collected before the first tag).
        // Skip blank lines in description.
        if !doc.has_tags && !is_blank {
            doc.description.push(line.clone());
        }
    }

    // Flush trailing @example block.
    if in_example {
        doc.examples.push(Example {
            title: example_title,
            body: deindent(&example_lines),
        });
    }

    doc
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn is_tag_line(line: &str) -> bool {
    line.starts_with('@')
}

/// Parse `[Type] name  - optional description`.
fn parse_param(rest: &str) -> Option<ParamTag> {
    let (r#type, after_type) = extract_bracketed_type_with_rest(rest)?;
    let name = after_type
        .trim()
        .split(|c: char| c.is_whitespace() || c == '-')
        .next()?
        .trim_end_matches(':')
        .to_string();
    if name.is_empty() {
        return None;
    }
    Some(ParamTag { name, r#type })
}

/// Extract `Type` from `[Type] rest...`, returning `(Type, rest)`.
/// Handles nested brackets like `[Array<String>]`.
fn extract_bracketed_type_with_rest(s: &str) -> Option<(String, &str)> {
    let s = s.trim();
    let start = s.find('[')?;
    let mut depth: u32 = 0;
    for (i, ch) in s[start..].char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    let end = start + i;
                    return Some((s[start + 1..end].to_string(), &s[end + 1..]));
                }
            }
            _ => {}
        }
    }
    None
}

/// Extract `Type` from `[Type] ...`.  Discards the rest.
fn extract_bracketed_type(s: &str) -> Option<String> {
    extract_bracketed_type_with_rest(s).map(|(t, _)| t)
}

/// Remove the common leading whitespace from a block of example lines,
/// preserving relative indentation.
fn deindent(lines: &[String]) -> String {
    if lines.is_empty() {
        return String::new();
    }

    // Strip leading and trailing blank lines.
    let first = lines.iter().position(|l| !l.trim().is_empty());
    let last = lines.iter().rposition(|l| !l.trim().is_empty());
    let (first, last) = match (first, last) {
        (Some(f), Some(l)) => (f, l),
        _ => return String::new(),
    };

    let body: Vec<&str> = lines[first..=last]
        .iter()
        .map(String::as_str)
        .collect();

    let indent = body
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);

    body.iter()
        .map(|l| {
            if l.len() >= indent {
                &l[indent..]
            } else {
                l
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn comments(lines: &[&str]) -> Vec<String> {
        lines.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn empty_comments() {
        let doc = parse(&[]);
        assert!(!doc.has_tags);
        assert!(doc.description.is_empty());
        assert!(doc.examples.is_empty());
    }

    #[test]
    fn description_only() {
        let doc = parse(&comments(&[
            "Adds two numbers together.",
            "",
            "Returns the sum.",
        ]));
        assert!(!doc.has_tags);
        assert_eq!(doc.description.len(), 2);
        assert_eq!(doc.description[0], "Adds two numbers together.");
    }

    #[test]
    fn param_and_return() {
        let doc = parse(&comments(&[
            "Does a thing.",
            "@param [String] name  The user's name.",
            "@param [Integer] age",
            "@return [User]",
        ]));
        assert!(doc.has_tags);
        assert_eq!(doc.description.len(), 1);
        assert_eq!(doc.params.len(), 2);
        assert_eq!(doc.params[0].name, "name");
        assert_eq!(doc.params[0].r#type, "String");
        assert_eq!(doc.params[1].name, "age");
        assert_eq!(doc.params[1].r#type, "Integer");
        assert_eq!(doc.returns.as_deref(), Some("User"));
    }

    #[test]
    fn param_with_keyword_splat() {
        let doc = parse(&comments(&[
            "@param [Hash] opts  Keyword options.",
        ]));
        assert_eq!(doc.params[0].name, "opts");
    }

    #[test]
    fn complex_types() {
        let doc = parse(&comments(&[
            "@param [Array<String>] items  The items.",
            "@return [Hash{String => Integer}]",
        ]));
        assert_eq!(doc.params[0].r#type, "Array<String>");
        assert_eq!(doc.returns.as_deref(), Some("Hash{String => Integer}"));
    }

    #[test]
    fn raise_tag() {
        let doc = parse(&comments(&[
            "@raise [ArgumentError] When name is blank.",
            "@raise [TypeError]",
        ]));
        assert_eq!(doc.raises.len(), 2);
        assert_eq!(doc.raises[0], "ArgumentError");
        assert_eq!(doc.raises[1], "TypeError");
    }

    #[test]
    fn type_annotation() {
        let doc = parse(&comments(&["@type [String]"]));
        assert_eq!(doc.type_annotation.as_deref(), Some("String"));
    }

    #[test]
    fn example_with_title() {
        let doc = parse(&comments(&[
            "Does something useful.",
            "@example Sum two numbers",
            "  result = add(1, 2)",
            "  assert_equal 3, result",
        ]));
        assert_eq!(doc.examples.len(), 1);
        assert_eq!(doc.examples[0].title.as_deref(), Some("Sum two numbers"));
        assert_eq!(doc.examples[0].body, "result = add(1, 2)\nassert_equal 3, result");
    }

    #[test]
    fn example_without_title() {
        let doc = parse(&comments(&[
            "@example",
            "  result = add(1, 2)",
            "  assert_equal 3, result",
        ]));
        assert_eq!(doc.examples.len(), 1);
        assert!(doc.examples[0].title.is_none());
        assert_eq!(doc.examples[0].body, "result = add(1, 2)\nassert_equal 3, result");
    }

    #[test]
    fn multiple_examples() {
        let doc = parse(&comments(&[
            "This method can add.",
            "@example Basic",
            "  add(1, 2)",
            "@example With zero",
            "  add(0, 5)",
        ]));
        assert_eq!(doc.examples.len(), 2);
        assert_eq!(doc.examples[0].title.as_deref(), Some("Basic"));
        assert_eq!(doc.examples[1].title.as_deref(), Some("With zero"));
    }

    #[test]
    fn example_followed_by_param() {
        let doc = parse(&comments(&[
            "@example",
            "  add(1, 2)",
            "@param [Integer] x",
        ]));
        assert_eq!(doc.examples.len(), 1);
        assert_eq!(doc.params.len(), 1);
    }

    #[test]
    fn deindent_preserves_relative() {
        let doc = parse(&comments(&[
            "@example",
            "  if true",
            "    puts 'hi'",
            "  end",
        ]));
        assert_eq!(doc.examples[0].body, "if true\n  puts 'hi'\nend");
    }

    #[test]
    fn deindent_strips_leading_blank() {
        let doc = parse(&comments(&[
            "@example",
            "",
            "  add(1, 2)",
            "",
        ]));
        assert_eq!(doc.examples[0].body, "add(1, 2)");
    }

    #[test]
    fn unknown_tags_skipped_but_mark_has_tags() {
        let doc = parse(&comments(&[
            "@api public",
            "@since 1.0",
            "Some description.",
        ]));
        assert!(doc.has_tags);
        assert!(doc.description.is_empty()); // tags came before description
        assert!(doc.examples.is_empty());
    }

    #[test]
    fn full_yard_block() {
        let doc = parse(&comments(&[
            "Add two integers and return the result.",
            "",
            "This method is pure and does not mutate its inputs.",
            "",
            "@param [Integer] a  The left operand.",
            "@param [Integer] b  The right operand.",
            "@return [Integer]",
            "@raise [ArgumentError] If inputs are negative.",
            "@example Basic usage",
            "  result = add(1, 2)",
            "  assert_equal 3, result",
            "@example Large numbers",
            "  result = add(1000, 2000)",
            "  assert_equal 3000, result",
        ]));
        assert!(doc.has_tags);
        assert_eq!(doc.description.len(), 2); // blank line between paragraphs skipped
        assert_eq!(doc.params.len(), 2);
        assert_eq!(doc.params[0].name, "a");
        assert_eq!(doc.params[1].name, "b");
        assert_eq!(doc.returns.as_deref(), Some("Integer"));
        assert_eq!(doc.raises, &["ArgumentError"]);
        assert_eq!(doc.examples.len(), 2);
    }

    #[test]
    fn no_tags_description_only() {
        let doc = parse(&comments(&[
            "A simple greeting method.",
        ]));
        assert!(!doc.has_tags);
        assert_eq!(doc.description.len(), 1);
    }
}