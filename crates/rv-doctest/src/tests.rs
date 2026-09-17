use crate::{Snippet, extract};
use indoc::indoc;
use rv_ruby_parser::ItemKind;
use rv_ruby_parser::parse;

fn snippets(source: &str) -> Vec<Snippet> {
    extract(&parse(source.as_bytes()))
}

#[test]
fn strict_fence_is_extracted() {
    let source = indoc! {"
        # Adds two numbers.
        #
        #   ```ruby
        #   add(1, 2)
        #   ```
        #
        def add(a, b)
          a + b
        end
    "};
    let snips = snippets(source);
    assert_eq!(snips.len(), 1);
    assert_eq!(snips[0].item_name, "add");
    assert_eq!(snips[0].item_kind, ItemKind::Def);
    assert_eq!(snips[0].code, "  add(1, 2)");
}

#[test]
fn bare_fence_is_ignored() {
    let source = indoc! {"
        # Example:
        #
        #   ```
        #   add(1, 2)
        #   ```
        #
        def add(a, b) = a + b
    "};
    assert!(snippets(source).is_empty());
}

#[test]
fn non_ruby_fence_is_ignored() {
    let source = indoc! {"
        # Example:
        #
        #   ```text
        #   some output
        #   ```
        #
        def foo = 1
    "};
    assert!(snippets(source).is_empty());
}

#[test]
fn info_string_is_case_insensitive() {
    let source = indoc! {"
        #   ```RUBY
        #   1 + 1
        #   ```
        def foo = 1
    "};
    assert_eq!(snippets(source).len(), 1);
}

#[test]
fn unterminated_fence_is_ignored() {
    let source = indoc! {"
        # Example:
        #
        #   ```ruby
        #   add(1, 2)
        #
        def add(a, b)
          a + b
        end
    "};
    assert!(snippets(source).is_empty());
}

#[test]
fn prose_is_not_a_snippet() {
    let source = indoc! {"
        # Adds two numbers.
        #
        # Returns the sum.
        #
        def add(a, b)
          a + b
        end
    "};
    assert!(snippets(source).is_empty());
}

#[test]
fn multiple_fences_in_one_comment() {
    let source = indoc! {"
        # Adds numbers.
        #
        #   ```ruby
        #   add(1, 2)
        #   ```
        #
        #   ```ruby
        #   add(3, 4)
        #   ```
        #
        def add(a, b)
          a + b
        end
    "};
    let snips = snippets(source);
    assert_eq!(snips.len(), 2);
    assert_eq!(snips[0].code, "  add(1, 2)");
    assert_eq!(snips[1].code, "  add(3, 4)");
}

#[test]
fn snippet_in_module_is_extracted() {
    let source = indoc! {"
        # Utils.
        #
        #   ```ruby
        #   Utils.answer
        #   ```
        #
        module Utils
          #   ```ruby
          #   Utils.noop
          #   ```
          def noop = nil
        end
    "};
    let snips = snippets(source);
    assert_eq!(snips.len(), 2);
    assert_eq!(snips[0].item_name, "Utils");
    assert_eq!(snips[0].item_kind, ItemKind::Module);
    assert_eq!(snips[0].code, "  Utils.answer");
    assert_eq!(snips[1].item_name, "noop");
    assert_eq!(snips[1].item_kind, ItemKind::Def);
    assert_eq!(snips[1].code, "  Utils.noop");
}

#[test]
fn snippet_in_class_is_extracted() {
    let source = indoc! {"
        # Foo.
        #
        #   ```ruby
        #   Foo.new
        #   ```
        #
        class Foo
        end
    "};
    let snips = snippets(source);
    assert_eq!(snips.len(), 1);
    assert_eq!(snips[0].item_name, "Foo");
    assert_eq!(snips[0].item_kind, ItemKind::Class);
    assert_eq!(snips[0].code, "  Foo.new");
}

#[test]
fn items_without_comments_yield_no_snippets() {
    let source = "def add(a, b)\n  a + b\nend\n";
    assert!(snippets(source).is_empty());
}
