use super::{Diagnostic, ItemKind, ParsedFile, parse};
use indoc::indoc;

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
