use super::calls::parse_calls;
use super::{Diagnostic, Item, ItemKind, ParsedFile, parse};
use indoc::indoc;

/// The fully-qualified path of every item, in source order.
fn full_paths(items: &[Item]) -> Vec<&str> {
    items.iter().map(|i| i.full_path.as_str()).collect()
}

fn find_item_kind_of<'a>(items: &'a [super::Item], kind: ItemKind, name: &str) -> &'a super::Item {
    items
        .iter()
        .find(|i| i.kind() == kind && i.name.as_str() == name)
        .unwrap_or_else(|| panic!("no item {kind:?} {name:?} in {items:?}"))
}

struct ExpectedItem {
    kind: ItemKind,
    name: &'static str,
    singleton: bool,
    comments: &'static [&'static str],
}

fn assert_items(parsed: &ParsedFile, expected: &[ExpectedItem]) {
    assert_eq!(parsed.items.len(), expected.len(), "unexpected item count");

    for item in expected {
        let found = find_item_kind_of(&parsed.items, item.kind, item.name);
        let comments = found
            .comments
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        assert_eq!(
            comments, item.comments,
            "comments for {:?} {:?}",
            item.kind, item.name
        );
        assert_eq!(
            found.singleton(),
            item.singleton,
            "singleton for {:?} {:?}",
            item.kind,
            item.name
        );
    }
}

macro_rules! comment_case {
    ($name:ident, $source:expr, [$($item:expr),* $(,)?]) => {
        #[test]
        fn $name() {
            let parsed = parse($source.as_bytes());
            assert!(parsed.is_success(), "parsing failed for case '{}'", stringify!($name));
            assert_items(&parsed, &[$($item),*]);
        }
    };
}

comment_case!(
    def_with_single_preceding_comment,
    indoc! {"
            # a comment
            def foo
            end
        "},
    [ExpectedItem {
        kind: ItemKind::Def,
        name: "foo",
        singleton: false,
        comments: &["a comment"],
    }]
);

comment_case!(
    def_with_blank_line_before_comment_is_not_attached,
    indoc! {"
            # a comment

            def foo
            end
        "},
    [ExpectedItem {
        kind: ItemKind::Def,
        name: "foo",
        singleton: false,
        comments: &[],
    }]
);

comment_case!(
    class_with_multiline_comment_block,
    indoc! {"
            # first line
            # second line
            class Foo
            end
        "},
    [ExpectedItem {
        kind: ItemKind::Class,
        name: "Foo",
        singleton: false,
        comments: &["first line", "second line"],
    }]
);

comment_case!(
    module_with_preceding_comment,
    indoc! {"
            # module docs
            module M
            end
        "},
    [ExpectedItem {
        kind: ItemKind::Module,
        name: "M",
        singleton: false,
        comments: &["module docs"],
    }]
);

comment_case!(
    nested_class_containing_def,
    indoc! {"
            # outer docs
            class Outer
              # inner docs
              def inner
              end
            end
        "},
    [
        ExpectedItem {
            kind: ItemKind::Class,
            name: "Outer",
            singleton: false,
            comments: &["outer docs"],
        },
        ExpectedItem {
            kind: ItemKind::Def,
            name: "inner",
            singleton: false,
            comments: &["inner docs"],
        },
    ]
);

comment_case!(
    class_with_documented_static_method,
    indoc! {"
            # Foo docs
            class Foo
              # .bar singleton docs
              def self.bar
              end

              # baz instance docs
              def baz
              end
            end
        "},
    [
        ExpectedItem {
            kind: ItemKind::Class,
            name: "Foo",
            singleton: false,
            comments: &["Foo docs"],
        },
        ExpectedItem {
            kind: ItemKind::Def,
            name: "bar",
            singleton: true,
            comments: &[".bar singleton docs"],
        },
        ExpectedItem {
            kind: ItemKind::Def,
            name: "baz",
            singleton: false,
            comments: &["baz instance docs"],
        },
    ]
);

comment_case!(
    module_with_multiple_documented_methods,
    indoc! {"
            # Utils docs
            module Utils
              # answer docs
              def answer
              end

              # noop docs
              def noop
              end
            end
        "},
    [
        ExpectedItem {
            kind: ItemKind::Module,
            name: "Utils",
            singleton: false,
            comments: &["Utils docs"],
        },
        ExpectedItem {
            kind: ItemKind::Def,
            name: "answer",
            singleton: false,
            comments: &["answer docs"],
        },
        ExpectedItem {
            kind: ItemKind::Def,
            name: "noop",
            singleton: false,
            comments: &["noop docs"],
        },
    ]
);

#[test]
fn comments_without_definition_are_ignored() {
    let source = indoc! {"
            # no definition follows this comment

            # nor this one
        "};
    let parsed = parse(source.as_bytes());

    assert!(
        parsed.items.is_empty(),
        "expected no items, found {:?}",
        parsed.items
    );
}

#[test]
fn superclass_detection() {
    let source = indoc! {"
            class Foo < Bar
            end

            class Baz < A::B
            end

            class Plain
            end
        "};
    let parsed = parse(source.as_bytes());

    assert_eq!(
        find_item_kind_of(&parsed.items, ItemKind::Class, "Foo").superclass(),
        Some("Bar"),
        "superclass of Foo"
    );
    assert_eq!(
        find_item_kind_of(&parsed.items, ItemKind::Class, "Baz").superclass(),
        Some("A::B"),
        "superclass of Baz"
    );
    assert_eq!(
        find_item_kind_of(&parsed.items, ItemKind::Class, "Plain").superclass(),
        None,
        "superclass of Plain"
    );
}

#[test]
fn invalid_source_surfaces_errors() {
    let source = "def foo(\n";
    let parsed = parse(source.as_bytes());

    assert!(!parsed.is_success(), "expected the source to fail parsing");
    assert_eq!(
        parsed.diagnostics,
        [
            Diagnostic {
                message: "unexpected end-of-input; expected a `)` to close the parameters"
                    .to_string(),
                line: 1,
                column: 9,
            },
            Diagnostic {
                message:
                    "unexpected end-of-input, assuming it is closing the parent top level context"
                        .to_string(),
                line: 1,
                column: 9,
            },
            Diagnostic {
                message: "expected an `end` to close the `def` statement".to_string(),
                line: 1,
                column: 1,
            },
        ]
    );
}
#[test]
fn comment_with_indentation_preserves_leading_spaces() {
    let source = "#     indented code\ndef foo\nend\n";
    let parsed = parse(source.as_bytes());
    assert!(parsed.is_success());
    let item = find_item_kind_of(&parsed.items, ItemKind::Def, "foo");
    // After Phase 1 transform: strip '#', then strip exactly ONE space.
    // Source '#     indented code' has 5 spaces after #.
    // Expected: '    indented code' (4 spaces preserved).
    assert_eq!(
        item.comments,
        vec!["    indented code"],
        "indentation should be preserved (4 spaces after stripping # + 1 space)"
    );
}

use super::calls::minitest_require_line;

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
    assert_eq!(line, Some(4), "found require on line 4");
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

#[test]
fn call_on_root_scoped_constant_resolves_without_the_leading_colons() {
    let calls = parse_calls(b"::Kernel.puts 1\n");
    assert_eq!(calls.len(), 1, "expected one call, got {calls:?}");
    assert_eq!(calls[0].class_name.as_deref(), Some("Kernel"));
    assert_eq!(calls[0].name, "puts");
}

#[test]
fn call_on_nested_root_scoped_constant_keeps_every_segment() {
    let calls = parse_calls(b"::Foo::Bar.baz\n");
    assert_eq!(calls.len(), 1, "expected one call, got {calls:?}");
    assert_eq!(calls[0].class_name.as_deref(), Some("Foo::Bar"));
}

#[test]
fn call_on_nested_constant_path_keeps_every_segment() {
    let calls = parse_calls(b"Foo::Bar::Baz.qux\n");
    assert_eq!(calls.len(), 1, "expected one call, got {calls:?}");
    assert_eq!(calls[0].class_name.as_deref(), Some("Foo::Bar::Baz"));
}

#[test]
fn full_path_qualifies_definitions_by_their_enclosing_namespace() {
    let source = indoc! {"
        module Math
          class Calculator
            def add(a, b)
            end

            def self.build
            end
          end
        end
    "};
    let parsed = parse(source.as_bytes());
    let paths = full_paths(&parsed.items);

    assert_eq!(
        paths,
        vec![
            "Math",
            "Math::Calculator",
            "Math::Calculator#add",
            "Math::Calculator.build",
        ]
    );
}

#[test]
fn full_path_of_a_top_level_definition_is_its_own_name() {
    let parsed = parse(b"def add(a, b)\nend\n");
    assert_eq!(parsed.items[0].full_path, "add");
}

#[test]
fn full_path_of_a_compact_namespace_is_not_double_qualified() {
    let source = indoc! {"
        module Foo::Bar
          def baz
          end
        end
    "};
    let parsed = parse(source.as_bytes());
    let paths = full_paths(&parsed.items);

    assert_eq!(paths, vec!["Foo::Bar", "Foo::Bar#baz"]);
}

#[test]
fn minitest_require_in_def_body_is_not_detected() {
    // A `def` body does not run until the method is called, so the require
    // cannot be relied on having happened at the enclosing statement's end.
    let source = indoc! {"
        def setup_env
          require 'minitest/autorun'
        end
        setup_env
    "};
    assert_eq!(minitest_require_line(source.as_bytes()), None);
}

#[test]
fn minitest_require_reports_the_enclosing_top_level_statement() {
    use super::calls::minitest_require;

    let source = indoc! {"
        class Helper
          require 'minitest/autorun'
        end
        assert_equal 1, 1
    "};
    let found = minitest_require(source.as_bytes()).expect("require found");
    assert_eq!(found.require_line, 2, "the require itself is on line 2");
    assert_eq!(
        found.top_level_end_line, 3,
        "the enclosing `class` statement ends on line 3"
    );
}

#[test]
fn minitest_require_at_top_level_is_its_own_statement() {
    use super::calls::minitest_require;

    let found = minitest_require(b"require 'minitest'\nassert_equal 1, 1\n").expect("found");
    assert_eq!(found.require_line, 1);
    assert_eq!(found.top_level_end_line, 1);
}

#[test]
fn parsed_source_serves_every_consumer_from_one_parse() {
    use super::ParsedSource;

    let source = indoc! {"
        require 'minitest/autorun'

        class Calculator
          def add(a, b)
            helper(a, b)
          end
        end
    "};

    let parsed = ParsedSource::new(source.as_bytes());

    assert!(parsed.is_success());
    assert!(parsed.diagnostics().is_empty());

    let items = parsed.items();
    let paths: Vec<&str> = items.iter().map(|i| i.full_path.as_str()).collect();
    assert_eq!(paths, vec!["Calculator", "Calculator#add"]);

    let names: Vec<String> = parsed.calls().iter().map(|c| c.name.clone()).collect();
    assert!(names.contains(&"require".to_string()), "{names:?}");
    assert!(names.contains(&"helper".to_string()), "{names:?}");

    assert_eq!(
        parsed.minitest_require().map(|m| m.require_line),
        Some(1),
        "the same parse also answers the minitest question"
    );
}

#[test]
fn parsed_source_reports_diagnostics_for_broken_source() {
    use super::ParsedSource;

    let parsed = ParsedSource::new(b"def broken(\n");
    assert!(!parsed.is_success());
    assert!(!parsed.diagnostics().is_empty());
    assert!(parsed.calls().is_empty(), "no calls from a broken parse");
    assert_eq!(parsed.minitest_require(), None);
}
