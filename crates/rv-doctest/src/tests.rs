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
    assert_eq!(snips[0].parent_path, String::new());
    assert_eq!(snips[0].code, "  add(1, 2)");
}

#[test]
fn snippet_start_line_points_inside_fence() {
    // 1: # Adds two numbers.
    // 2: #
    // 3: #   ```ruby
    // 4: #   add(1, 2)
    // 5: #   ```
    // 6: #
    // 7: def add(a, b)
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
    // Points at the first code line inside the fence, not the `def`.
    assert_eq!(snips[0].start_line, 4);
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
    assert_eq!(snips[0].parent_path, String::new());
    assert_eq!(snips[0].code, "  Utils.answer");
    assert_eq!(snips[1].item_name, "noop");
    assert_eq!(snips[1].item_kind, ItemKind::Def);
    assert_eq!(snips[1].parent_path, String::new());
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
    assert_eq!(snips[0].parent_path, String::new());
    assert_eq!(snips[0].code, "  Foo.new");
}

#[test]
fn items_without_comments_yield_no_snippets() {
    let source = "def add(a, b)\n  a + b\nend\n";
    assert!(snippets(source).is_empty());
}

#[test]
fn assertion_checker_applies_to_minitest() {
    assert!(crate::AssertionChecker::applies_to(
        "require \"minitest\"\n"
    ));
    assert!(crate::AssertionChecker::applies_to(
        "require 'minitest/autorun'\n"
    ));
    assert!(crate::AssertionChecker::applies_to(
        "require \"minitest\"\nrequire \"minitest/autorun\"\n"
    ));
    assert!(!crate::AssertionChecker::applies_to(
        "require \"test/unit\"\n"
    ));
    assert!(!crate::AssertionChecker::applies_to("1 + 1\n"));
}

#[test]
fn checker_dispatches_to_assert_for_minitest() {
    let snips = snippets(indoc! {"
        # Docs.
        #
        #   ```ruby
        #   require \"minitest\"
        #   ```
        #
        def foo = 1
    "});
    assert_eq!(snips.len(), 1);
    assert!(crate::AssertionChecker::applies_to(&snips[0].code));
}

#[test]
fn checker_dispatches_to_syntax_for_non_minitest() {
    let snips = snippets(indoc! {"
        # Docs.
        #
        #   ```ruby
        #   add(1, 2)
        #   ```
        #
        def add(a, b) = a + b
    "});
    assert_eq!(snips.len(), 1);
    assert!(!crate::AssertionChecker::applies_to(&snips[0].code));
}

use rv_ruby::Ruby;

/// Locates the Ruby on PATH as a `Ruby` for execution tests.
/// Returns None when no interpreter is available.
fn test_ruby() -> Option<Ruby> {
    let output = std::process::Command::new("ruby")
        .args(["-e", "print RbConfig.ruby"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let exe = String::from_utf8(output.stdout).ok()?;
    let exe = camino::Utf8Path::new(exe.trim());
    let dir = exe.parent()?.parent()?.to_path_buf();
    Ruby::from_dir(dir, false).ok()
}

#[tokio::test]
async fn passes_simple_assertion_via_injection() {
    let Some(ruby) = test_ruby() else { return };
    let mut checker = crate::AssertionChecker::new(ruby);
    let code = "require \"minitest\"\nassert_equal 2, 1 + 1\n";
    assert!(crate::RubyChecker::check(&mut checker, code).await.is_ok());
}

#[tokio::test]
async fn reports_failing_assertion() {
    let Some(ruby) = test_ruby() else { return };
    let mut checker = crate::AssertionChecker::new(ruby);
    let code = "require \"minitest\"\nassert_equal 3, 1 + 1\n";
    match crate::RubyChecker::check(&mut checker, code).await {
        Err(crate::CheckError::Assertion { stderr, .. }) => {
            assert!(stderr.contains("Expected"), "stderr: {stderr}");
            assert!(stderr.contains("Actual"), "stderr: {stderr}");
        }
        other => panic!("expected CheckError::Assertion, got {other:?}"),
    }
}

#[tokio::test]
async fn manual_frame_still_passes() {
    let Some(ruby) = test_ruby() else { return };
    let mut checker = crate::AssertionChecker::new(ruby);
    let code = indoc! {"
        require \"minitest\"
        include Minitest::Assertions
        class << self; attr_accessor :assertions; end
        self.assertions = 0
        assert_equal 4, 2 + 2
    "};
    assert!(crate::RubyChecker::check(&mut checker, code).await.is_ok());
}

#[tokio::test]
async fn checker_enum_routes_minimest_to_assertion() {
    let Some(ruby) = test_ruby() else { return };
    let snippet = Snippet {
        item_name: String::new(),
        item_kind: ItemKind::Def,
        parent_path: String::new(),
        start_line: 1,
        code: "require \"minitest\"\nassert_equal 2, 1 + 1".into(),
    };
    let mut checker = crate::Checker::for_snippet(ruby, &snippet);
    assert!(matches!(checker, crate::Checker::Assert(_)));
    assert!(
        crate::RubyChecker::check(&mut checker, &snippet.code)
            .await
            .is_ok()
    );
}
