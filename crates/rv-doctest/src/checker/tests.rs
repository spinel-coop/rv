use super::*;
use crate::{ItemKind, Snippet};
use tempfile::TempDir;

fn make_snippet(code: &str) -> Snippet {
    Snippet {
        item_name: "initialize".to_string(),
        item_kind: ItemKind::Def,
        parent_path: "User".to_string(),
        start_line: 1,
        code: code.to_string(),
    }
}

fn checker_with_env() -> (RbsChecker, TempDir) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("user.rbs"),
        "class User\n  def initialize: (String name, Integer age) -> void\nend\n",
    )
    .unwrap();
    let env = RbsEnvironment::load(&[dir.path().to_str().unwrap()]).unwrap();
    (RbsChecker::new(env), dir)
}

#[test]
fn test_valid_arity() {
    let (checker, _dir) = checker_with_env();
    let snippet = make_snippet("User.initialize(\"alice\", 30)");
    assert!(checker.check(&snippet).is_empty());
}

#[test]
fn test_too_few_args() {
    let (checker, _dir) = checker_with_env();
    let snippet = make_snippet("User.initialize(\"alice\")");
    let violations = checker.check(&snippet);
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].class_name, "User");
    assert_eq!(violations[0].method_name, "initialize");
    assert!(violations[0].message.contains("expected at least"));
}

#[test]
fn test_too_many_args() {
    let (checker, _dir) = checker_with_env();
    let snippet = make_snippet("User.initialize(\"alice\", 30, \"extra\")");
    let violations = checker.check(&snippet);
    assert_eq!(violations.len(), 1);
    assert!(violations[0].message.contains("expected at most"));
}

#[test]
fn test_unknown_class_skipped() {
    let (checker, _dir) = checker_with_env();
    let snippet = make_snippet("Other::Thing.create(\"x\")");
    assert!(checker.check(&snippet).is_empty());
}

#[test]
fn test_unknown_method_skipped() {
    let (checker, _dir) = checker_with_env();
    let snippet = make_snippet("User.nonexistent(1, 2)");
    assert!(checker.check(&snippet).is_empty());
}

#[test]
fn test_instance_receiver_skipped() {
    let (checker, _dir) = checker_with_env();
    // `user` is lowercase — treated as a variable, not a class.
    let snippet = make_snippet("user.upcase(1, 2, 3, 4, 5)");
    assert!(checker.check(&snippet).is_empty());
}

#[test]
fn test_multiple_calls_checked_independently() {
    let (checker, _dir) = checker_with_env();
    let snippet = make_snippet("User.initialize(\"alice\", 30)\nUser.initialize(30)");
    let violations = checker.check(&snippet);
    assert_eq!(violations.len(), 1);
    assert!(violations[0].message.contains("expected at least"));
}

#[test]
fn test_bare_call_uses_parent_path() {
    let (checker, _dir) = checker_with_env();
    let snippet = make_snippet("initialize(\"alice\")");
    let violations = checker.check(&snippet);
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].class_name, "User");
}

#[test]
fn test_bare_call_without_parent_path_skipped() {
    let (checker, _dir) = checker_with_env();
    let snippet = Snippet {
        item_name: "greet".to_string(),
        item_kind: ItemKind::Def,
        parent_path: String::new(),
        start_line: 1,
        code: "greet(\"hello\")".to_string(),
    };
    assert!(checker.check(&snippet).is_empty());
}

#[test]
fn test_comments_ignored() {
    let (checker, _dir) = checker_with_env();
    let snippet = make_snippet("# User.initialize(1)\n# User.initialize()");
    assert!(checker.check(&snippet).is_empty());
}

#[test]
fn test_multiline_call_valid() {
    let (checker, _dir) = checker_with_env();
    let snippet = make_snippet("User.initialize(\n  \"alice\",\n  30\n)");
    assert!(checker.check(&snippet).is_empty());
}

#[test]
fn test_nested_class_resolution() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("outer.rbs"),
        "class Outer\n  class Inner\n    def process: (String) -> void\n  end\nend\n",
    )
    .unwrap();
    let env = RbsEnvironment::load(&[dir.path().to_str().unwrap()]).unwrap();
    let checker = RbsChecker::new(env);
    let snippet = make_snippet("Outer::Inner.process(String.new)");
    assert!(checker.check(&snippet).is_empty());

    let bad = make_snippet("Outer::Inner.process(1, 2)");
    assert_eq!(checker.check(&bad).len(), 1);
}

#[test]
fn test_equivalence_block_no_false_positive() {
    let (checker, _dir) = checker_with_env();
    // Typical "what the method replaces" snippet, e.g. Rails `blank?` docs.
    let snippet = make_snippet("!address || address.empty?");
    assert!(checker.check(&snippet).is_empty());
}

#[test]
fn test_args_with_nested_commas() {
    let (checker, _dir) = checker_with_env();
    // Two args: a hash and an integer. Should pass arity of 2.
    let snippet = make_snippet("User.initialize({a: 1, b: 2}, 30)");
    assert!(checker.check(&snippet).is_empty());
}

#[test]
fn test_args_with_string_commas() {
    let (checker, _dir) = checker_with_env();
    let snippet = make_snippet("User.initialize(\"a, b, c\", 30)");
    assert!(checker.check(&snippet).is_empty());
}
