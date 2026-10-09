use crate::{Snippet, assertion_applies_to, assertion_check, extract, syntax_check};
use indoc::indoc;
use rv_ruby_parser::ItemKind;
use rv_ruby_parser::parse;

fn snippets(source: &str) -> Vec<Snippet> {
    extract(&parse(source.as_bytes()))
}

macro_rules! assert_snippet {
    ($snips:expr, $idx:expr, name: $name:literal, kind: $kind:ident, code: $code:literal) => {
        assert_snippet!($snips, $idx, name: $name, kind: $kind, parent: "", code: $code);
    };
    ($snips:expr, $idx:expr, name: $name:literal, kind: $kind:ident, parent: $parent:literal, code: $code:literal) => {
        assert_eq!($snips[$idx].item_name, $name);
        assert_eq!($snips[$idx].item_kind, ItemKind::$kind);
        assert_eq!($snips[$idx].parent_path, $parent);
        assert_eq!($snips[$idx].code, $code);
    };
}

/// A doc comment with one fenced `ruby` block per `codes` entry, attached to
/// `body` so `extract` has an item to hang it on.
fn documented_def(doc: &str, codes: &[&str], body: &str) -> String {
    let mut source = format!("# {doc}\n#\n");
    for code in codes {
        source.push_str("#   ```ruby\n");
        source.push_str(&format!("#   {code}\n"));
        source.push_str("#   ```\n#\n");
    }
    source.push_str(body);
    source.push('\n');
    source
}

/// The `add(1, 2)` doctest example shared by extraction tests.
fn add_example_doc() -> String {
    documented_def(
        "Adds two numbers.",
        &["add(1, 2)"],
        "def add(a, b)\n  a + b\nend",
    )
}

#[test]
fn strict_fence_is_extracted() {
    let snips = snippets(&add_example_doc());
    assert_eq!(snips.len(), 1);
    assert_snippet!(snips, 0, name: "add", kind: Def, code: "  add(1, 2)");
}

#[test]
fn snippet_start_line_points_inside_fence() {
    let snips = snippets(&add_example_doc());
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
fn unsnippetable_input_yields_nothing() {
    for source in [
        indoc! {"
            # Example:
            #
            #   ```text
            #   some output
            #   ```
        "},
        indoc! {"
            # Example:
            #
            #   ```ruby
            #   add(1, 2)
            #
            def add(a, b)
              a + b
            end
        "},
        indoc! {"
            # Adds two numbers.
            #
            # Returns the sum.
            #
            def add(a, b)
              a + b
            end
        "},
    ] {
        assert!(snippets(source).is_empty());
    }
}

#[test]
fn multiple_fences_in_one_comment() {
    let source = documented_def(
        "Adds numbers.",
        &["add(1, 2)", "add(3, 4)"],
        "def add(a, b)\n  a + b\nend",
    );
    let snips = snippets(&source);
    assert_eq!(snips.len(), 2);
    assert_snippet!(snips, 0, name: "add", kind: Def, code: "  add(1, 2)");
    assert_snippet!(snips, 1, name: "add", kind: Def, code: "  add(3, 4)");
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
    assert_snippet!(snips, 1, name: "noop", kind: Def, parent: "Utils", code: "  Utils.noop");
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
fn checker_dispatches_by_minitest_presence() {
    for (fence_code, item_body, expects_assertion) in [
        (r#"require "minitest""#, "def foo = 1", true),
        ("add(1, 2)", "def add(a, b) = a + b", false),
    ] {
        let source = format!(
            "# Docs.\n#\n#   ```ruby\n#   {fence_code}\n#   ```\n#\n{item_body}\n"
        );
        let snips = snippets(&source);
        assert_eq!(snips.len(), 1);
        assert_eq!(assertion_applies_to(&snips[0].code), expects_assertion);
    }
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

/// Run `code` as an assertion example the way `check_snippets` does.
async fn run_assertion(ruby: Ruby, code: &str) -> Result<(), crate::CheckError> {
    let analysis = crate::SnippetAnalysis::of(code);
    let minitest = analysis.minitest.expect("code requires minitest");
    assertion_check(ruby, code, minitest).await
}

#[tokio::test]
async fn passes_simple_assertion_via_injection() {
    let Some(ruby) = test_ruby() else { return };
    let code = "require \"minitest\"\nassert_equal 2, 1 + 1\n";
    assert!(run_assertion(ruby, code).await.is_ok());
}

#[tokio::test]
async fn reports_failing_assertion() {
    let Some(ruby) = test_ruby() else { return };
    let code = "require \"minitest\"\nassert_equal 3, 1 + 1\n";
    match run_assertion(ruby, code).await {
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
    assert!(run_assertion(ruby, code).await.is_ok());
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

#[test]
fn snippet_analysis_answers_both_checkers_from_one_parse() {
    let analysis = crate::SnippetAnalysis::of("require \"minitest\"\nassert_equal 2, 1 + 1\n");

    assert!(analysis.is_assertion(), "minitest require should be found");
    assert_eq!(analysis.minitest.map(|m| m.require_line), Some(1));

    let names: Vec<&str> = analysis.calls.iter().map(|c| c.name.as_str()).collect();
    assert!(names.contains(&"assert_equal"), "{names:?}");
}

#[test]
fn snippet_analysis_of_a_plain_snippet_is_not_an_assertion() {
    let analysis = crate::SnippetAnalysis::of("Foo.bar(1)\n");

    assert!(!analysis.is_assertion());
    assert_eq!(analysis.minitest, None);
    assert_eq!(analysis.calls.len(), 1, "{:?}", analysis.calls);
}

#[test]
fn extract_analyzed_pairs_each_snippet_with_its_analysis() {
    let source = indoc! {"
        # Adds.
        #
        #   ```ruby
        #   require \"minitest\"
        #   assert_equal 3, 1 + 2
        #   ```
        def add(a, b)
          a + b
        end
    "};

    let analyzed = crate::extract_analyzed(&parse(source.as_bytes()));
    assert_eq!(analyzed.len(), 1);

    let (snippet, analysis) = &analyzed[0];
    assert_eq!(snippet.item_name, "add");
    assert!(analysis.is_assertion(), "snippet requires minitest");
}
