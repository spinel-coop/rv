use super::*;
use crate::{ItemKind, Snippet};
use tempfile::TempDir;

const PATH: &str = "test.rb";

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
    let report = checker.check(&make_snippet("User.initialize(\"alice\", 30)"), PATH);
    assert!(report.violations.is_empty());
    assert_eq!(report.stats.checked, 1);
}

#[test]
fn test_too_few_args() {
    let (checker, _dir) = checker_with_env();
    let report = checker.check(&make_snippet("User.initialize(\"alice\")"), PATH);
    assert_eq!(report.violations.len(), 1);
    assert_eq!(report.violations[0].class_name, "User");
    assert_eq!(report.violations[0].method_name, "initialize");
    assert!(report.violations[0].message.contains("expected at least"));
    assert_eq!(report.stats.checked, 1);
}

#[test]
fn test_too_many_args() {
    let (checker, _dir) = checker_with_env();
    let report = checker.check(
        &make_snippet("User.initialize(\"alice\", 30, \"extra\")"),
        PATH,
    );
    assert_eq!(report.violations.len(), 1);
    assert!(report.violations[0].message.contains("expected at most"));
    assert_eq!(report.stats.checked, 1);
}

#[test]
fn test_unknown_class_counted() {
    let (checker, _dir) = checker_with_env();
    let report = checker.check(&make_snippet("Other::Thing.create(\"x\")"), PATH);
    assert!(report.violations.is_empty());
    let entry = &report.stats.unknown_classes["Other::Thing"];
    assert_eq!(entry.count, 1);
    assert_eq!(entry.locations, vec!["test.rb:1"]);
}

#[test]
fn test_unknown_method_counted() {
    let (checker, _dir) = checker_with_env();
    let report = checker.check(&make_snippet("User.nonexistent(1, 2)"), PATH);
    assert!(report.violations.is_empty());
    let entry = &report.stats.unknown_methods["User.nonexistent"];
    assert_eq!(entry.count, 1);
    assert_eq!(entry.locations, vec!["test.rb:1"]);
}

#[test]
fn test_instance_receiver_counted_as_unknown_receiver() {
    let (checker, _dir) = checker_with_env();
    // `user` is a variable — recorded under its raw, greppable text.
    let report = checker.check(&make_snippet("user.upcase(1, 2, 3, 4, 5)"), PATH);
    assert!(report.violations.is_empty());
    let entry = &report.stats.unknown_receivers["user.upcase"];
    assert_eq!(entry.count, 1);
    assert_eq!(entry.locations, vec!["test.rb:1"]);
}

#[test]
fn test_self_receiver_counted_as_unknown_receiver() {
    let (checker, _dir) = checker_with_env();
    let report = checker.check(&make_snippet("self.missing(1)"), PATH);
    assert!(report.violations.is_empty());
    let entry = &report.stats.unknown_receivers["self.missing"];
    assert_eq!(entry.count, 1);
}

#[test]
fn test_ivar_receiver_counted_as_unknown_receiver() {
    let (checker, _dir) = checker_with_env();
    let report = checker.check(&make_snippet("@cache.fetch(:key)"), PATH);
    assert!(report.violations.is_empty());
    let entry = &report.stats.unknown_receivers["@cache.fetch"];
    assert_eq!(entry.count, 1);
}

#[test]
fn test_multiple_calls_checked_independently() {
    let (checker, _dir) = checker_with_env();
    let report = checker.check(
        &make_snippet("User.initialize(\"alice\", 30)\nUser.initialize(30)"),
        PATH,
    );
    assert_eq!(report.violations.len(), 1);
    assert!(report.violations[0].message.contains("expected at least"));
    assert_eq!(report.stats.checked, 2);
}

#[test]
fn test_bare_call_uses_parent_path() {
    let (checker, _dir) = checker_with_env();
    let report = checker.check(&make_snippet("initialize(\"alice\")"), PATH);
    assert_eq!(report.violations.len(), 1);
    assert_eq!(report.violations[0].class_name, "User");
    assert_eq!(report.stats.checked, 1);
}

#[test]
fn test_bare_call_without_parent_path_counted_as_unknown_receiver() {
    let (checker, _dir) = checker_with_env();
    let snippet = Snippet {
        item_name: "greet".to_string(),
        item_kind: ItemKind::Def,
        parent_path: String::new(),
        start_line: 1,
        code: "greet(\"hello\")".to_string(),
    };
    let report = checker.check(&snippet, PATH);
    assert!(report.violations.is_empty());
    let entry = &report.stats.unknown_receivers["greet"];
    assert_eq!(entry.count, 1);
    assert_eq!(entry.locations, vec!["test.rb:1"]);
}

#[test]
fn test_comments_ignored() {
    let (checker, _dir) = checker_with_env();
    let report = checker.check(
        &make_snippet("# User.initialize(1)\n# User.initialize()"),
        PATH,
    );
    assert!(report.violations.is_empty());
    assert_eq!(report.stats.checked, 0);
    assert_eq!(report.stats.skipped(), 0);
}

#[test]
fn test_multiline_call_valid() {
    let (checker, _dir) = checker_with_env();
    let report = checker.check(
        &make_snippet("User.initialize(\n  \"alice\",\n  30\n)"),
        PATH,
    );
    assert!(report.violations.is_empty());
    assert_eq!(report.stats.checked, 1);
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

    let good = checker.check(&make_snippet("Outer::Inner.process(String.new)"), PATH);
    assert!(good.violations.is_empty());
    assert_eq!(good.stats.checked, 1);

    let bad = checker.check(&make_snippet("Outer::Inner.process(1, 2)"), PATH);
    assert_eq!(bad.violations.len(), 1);
}

#[test]
fn test_equivalence_block_no_false_positive() {
    let (checker, _dir) = checker_with_env();
    // Typical "what the method replaces" snippet, e.g. Rails `blank?` docs.
    // No parens anywhere, so no calls are extracted at all.
    let report = checker.check(&make_snippet("!address || address.empty?"), PATH);
    assert!(report.violations.is_empty());
    assert_eq!(report.stats.checked, 0);
    assert_eq!(report.stats.skipped(), 0);
}

#[test]
fn test_args_with_nested_commas() {
    let (checker, _dir) = checker_with_env();
    // Two args: a hash and an integer. Should pass arity of 2.
    let report = checker.check(&make_snippet("User.initialize({a: 1, b: 2}, 30)"), PATH);
    assert!(report.violations.is_empty());
}

#[test]
fn test_args_with_string_commas() {
    let (checker, _dir) = checker_with_env();
    let report = checker.check(&make_snippet("User.initialize(\"a, b, c\", 30)"), PATH);
    assert!(report.violations.is_empty());
}

#[test]
fn test_stats_merge() {
    let mut a = CheckStats {
        checked: 1,
        ..Default::default()
    };
    a.unknown_classes.insert(
        "Foo".to_string(),
        UnknownEntry {
            count: 2,
            locations: vec!["a.rb:1".to_string()],
        },
    );
    a.unknown_receivers.insert(
        "user.name".to_string(),
        UnknownEntry {
            count: 1,
            locations: vec!["a.rb:2".to_string()],
        },
    );

    let mut b = CheckStats {
        checked: 3,
        ..Default::default()
    };
    b.unknown_classes.insert(
        "Foo".to_string(),
        UnknownEntry {
            count: 1,
            locations: vec!["b.rb:1".to_string()],
        },
    );
    b.unknown_classes.insert(
        "Bar".to_string(),
        UnknownEntry {
            count: 4,
            locations: vec![
                "b.rb:2".to_string(),
                "b.rb:5".to_string(),
                "a.rb:1".to_string(), // already present on a
            ],
        },
    );
    b.unknown_methods.insert(
        "Foo.m".to_string(),
        UnknownEntry {
            count: 2,
            locations: vec!["b.rb:9".to_string()],
        },
    );

    a.merge(&b);
    assert_eq!(a.checked, 4);

    let foo = &a.unknown_classes["Foo"];
    assert_eq!(foo.count, 3);
    assert_eq!(foo.locations, vec!["a.rb:1", "b.rb:1"]);

    let bar = &a.unknown_classes["Bar"];
    assert_eq!(bar.count, 4);
    assert_eq!(bar.locations, vec!["b.rb:2", "b.rb:5", "a.rb:1"]);

    assert_eq!(a.unknown_methods["Foo.m"].count, 2);
    assert_eq!(a.unknown_receivers["user.name"].count, 1);
    assert_eq!(a.skipped(), 1 + 3 + 4 + 2);
}

#[test]
fn test_per_call_lines_recorded_exactly() {
    let (checker, _dir) = checker_with_env();
    let report = checker.check(&make_snippet("user.save(1)\nuser.save(2)"), PATH);
    let entry = &report.stats.unknown_receivers["user.save"];
    assert_eq!(entry.count, 2);
    // Each call gets its exact line within the file.
    assert_eq!(entry.locations, vec!["test.rb:1", "test.rb:2"]);
}

#[test]
fn test_call_line_tracks_snippet_start_line() {
    let (checker, _dir) = checker_with_env();
    // Snippet that starts at line 10: first call → 10, third line → 12.
    let mut snippet = make_snippet("user.save(1)\nuser.load(2)");
    snippet.start_line = 10;
    snippet.code = "user.save(1)\n# comment\nuser.load(2)".to_string();
    let report = checker.check(&snippet, PATH);
    assert_eq!(
        report.stats.unknown_receivers["user.save"].locations,
        vec!["test.rb:10"]
    );
    assert_eq!(
        report.stats.unknown_receivers["user.load"].locations,
        vec!["test.rb:12"]
    );
}
