use crate::{Snippet, assertion_applies_to, assertion_check, extract, syntax_check};
use indoc::indoc;
use rv_ruby_parser::ItemKind;
use rv_ruby_parser::parse;

fn snippets(source: &str) -> Vec<Snippet> {
    extract(&parse(source.as_bytes()))
}

macro_rules! assert_snippet {
    ($snips:expr, $idx:expr, name: $name:literal, kind: $kind:ident, code: $code:literal) => {
        assert_eq!($snips[$idx].item_name, $name);
        assert_eq!($snips[$idx].item_kind, ItemKind::$kind);
        assert_eq!($snips[$idx].parent_path, String::new());
        assert_eq!($snips[$idx].code, $code);
    };
}

fn def_with_fence(doc: &str, code: &str, body: &str) -> String {
    format!(
        indoc! {"
            # {}
            #
            #   ```ruby
            #   {}
            #   ```
            #
            {}
        "},
        doc, code, body
    )
}

fn def_with_two_fences(doc: &str, code1: &str, code2: &str, body: &str) -> String {
    format!(
        indoc! {"
            # {}
            #
            #   ```ruby
            #   {}
            #   ```
            #
            #   ```ruby
            #   {}
            #   ```
            #
            {}
        "},
        doc, code1, code2, body
    )
}

#[test]
fn strict_fence_is_extracted() {
    let source = def_with_fence(
        "Adds two numbers.",
        "add(1, 2)",
        "def add(a, b)\n  a + b\nend",
    );
    let snips = snippets(&source);
    assert_eq!(snips.len(), 1);
    assert_snippet!(snips, 0, name: "add", kind: Def, code: "  add(1, 2)");
}

#[test]
fn snippet_start_line_points_inside_fence() {
    let source = def_with_fence(
        "Adds two numbers.",
        "add(1, 2)",
        "def add(a, b)\n  a + b\nend",
    );
    let snips = snippets(&source);
    assert_eq!(snips.len(), 1);
    assert_eq!(snips[0].start_line, 4);
}

#[test]
fn ruby_fence_is_extracted() {
    let source = indoc! {"
        # Example:
        #
        #   ```ruby
        #   add(1, 2)
        #   ```
        #
        def add(a, b) = a + b
    "};
    let snips = snippets(source);
    assert_eq!(snips.len(), 1);
    assert_eq!(snips[0].code, "  add(1, 2)");
}

#[test]
fn ruby_fence_is_case_insensitive() {
    let source = indoc! {"
        #   ```RUBY
        #   1 + 1
        #   ```
        def foo = 1
    "};
    assert_eq!(snippets(source).len(), 1);
}

#[test]
fn non_ruby_fence_is_not_extracted() {
    let source = indoc! {"
        # Example:
        #
        #   ```text
        #   some output
        #   ```
    "};
    assert!(snippets(source).is_empty());
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
    let source = def_with_two_fences(
        "Adds numbers.",
        "add(1, 2)",
        "add(3, 4)",
        "def add(a, b)\n  a + b\nend",
    );
    let snips = snippets(&source);
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
    assert_snippet!(snips, 0, name: "Utils", kind: Module, code: "  Utils.answer");
    assert_snippet!(snips, 1, name: "noop", kind: Def, code: "  Utils.noop");
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
    assert_snippet!(snips, 0, name: "Foo", kind: Class, code: "  Foo.new");
}

#[test]
fn items_without_comments_yield_no_snippets() {
    let source = "def add(a, b)\n  a + b\nend\n";
    assert!(snippets(source).is_empty());
}

#[test]
fn assertion_checker_applies_to_minitest() {
    assert!(assertion_applies_to("require \"minitest\"\n"));
    assert!(assertion_applies_to("require 'minitest/autorun'\n"));
    assert!(assertion_applies_to(
        "require \"minitest\"\nrequire \"minitest/autorun\"\n"
    ));
    assert!(!assertion_applies_to("require \"test/unit\"\n"));
    assert!(!assertion_applies_to("1 + 1\n"));
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
    assert!(assertion_applies_to(&snips[0].code));
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
    assert!(!assertion_applies_to(&snips[0].code));
}

use rv_ruby::Ruby;

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
    let code = "require \"minitest\"\nassert_equal 2, 1 + 1\n";
    assert!(assertion_check(ruby, code).await.is_ok());
}

#[tokio::test]
async fn reports_failing_assertion() {
    let Some(ruby) = test_ruby() else { return };
    let code = "require \"minitest\"\nassert_equal 3, 1 + 1\n";
    match assertion_check(ruby, code).await {
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
    let code = indoc! {"
        require \"minitest\"
        include Minitest::Assertions
        class << self; attr_accessor :assertions; end
        self.assertions = 0
        assert_equal 4, 2 + 2
    "};
    assert!(assertion_check(ruby, code).await.is_ok());
}

#[tokio::test]
async fn syntax_check_passes_valid_code() {
    let Some(ruby) = test_ruby() else { return };
    let code = "def add(a, b); a + b; end\n";
    assert!(syntax_check(ruby, code).await.is_ok());
}

#[tokio::test]
async fn syntax_check_fails_invalid_code() {
    let Some(ruby) = test_ruby() else { return };
    let code = "def foo(\n";
    assert!(syntax_check(ruby, code).await.is_err());
}
