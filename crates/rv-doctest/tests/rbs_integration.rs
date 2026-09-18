//! End-to-end test: extract fenced examples from a Ruby file and validate
//! their method calls against RBS signatures.

use indoc::indoc;
use rv_doctest::{RbsChecker, RbsEnvironment, extract};

#[test]
fn rbs_arity_check_on_real_ruby_source() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("user.rbs"),
        "class User\n  def initialize: (String name, Integer age) -> void\nend\n",
    )
    .unwrap();

    let ruby_source = indoc! {r#"
        class User
          # Create a user.
          #
          # ```ruby
          # User.initialize("alice", 30)
          # ```
          def initialize(name, age)
          end
        end
    "#};

    let parsed = rv_ruby_parser::parse(ruby_source.as_bytes());
    let snippets = extract(&parsed);
    assert_eq!(snippets.len(), 1);

    let env = RbsEnvironment::load(&[dir.path().to_str().unwrap()]).unwrap();
    let checker = RbsChecker::new(env);
    let report = checker.check(&snippets[0], "fixture.rb");
    assert!(report.violations.is_empty());
    assert_eq!(report.stats.checked, 1);
}

#[test]
fn rbs_arity_mismatch_is_detected() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("user.rbs"),
        "class User\n  def initialize: (String name, Integer age) -> void\nend\n",
    )
    .unwrap();

    let ruby_source = indoc! {r#"
        class User
          # Create a user.
          #
          # ```ruby
          # User.initialize("alice")
          # ```
          def initialize(name, age)
          end
        end
    "#};

    let parsed = rv_ruby_parser::parse(ruby_source.as_bytes());
    let snippets = extract(&parsed);
    assert_eq!(snippets.len(), 1);

    let env = RbsEnvironment::load(&[dir.path().to_str().unwrap()]).unwrap();
    let checker = RbsChecker::new(env);
    let report = checker.check(&snippets[0], "fixture.rb");
    assert_eq!(report.violations.len(), 1);
    assert_eq!(report.violations[0].class_name, "User");
    assert_eq!(report.violations[0].method_name, "initialize");
    assert!(report.violations[0].message.contains("expected at least"));
}

#[test]
fn equivalence_block_produces_no_violations() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("object.rbs"),
        "class Object\n  def blank?: () -> bool\nend\n",
    )
    .unwrap();

    // Rails-style equivalence explanation: shows what `blank?` replaces,
    // not a call to it. Must not be flagged.
    let ruby_source = indoc! {r##"
        class Object
          # An object is blank if it responds to `empty?`.
          #
          #   ```ruby
          #   !address || address.empty?
          #   ```
          def blank?
          end
        end
    "##};

    let parsed = rv_ruby_parser::parse(ruby_source.as_bytes());
    let snippets = extract(&parsed);
    assert_eq!(snippets.len(), 1);

    let env = RbsEnvironment::load(&[dir.path().to_str().unwrap()]).unwrap();
    let checker = RbsChecker::new(env);
    let report = checker.check(&snippets[0], "fixture.rb");
    assert!(report.violations.is_empty());
    assert_eq!(report.stats.checked, 0);
    assert_eq!(report.stats.skipped(), 0);
}

#[test]
fn missing_sig_dir_returns_empty_env() {
    // A directory that contains no .rbs files yields an empty environment,
    // which is permissive: nothing gets flagged, but the class is counted
    // as unknown so coverage is visible.
    let dir = tempfile::tempdir().unwrap();
    let env = RbsEnvironment::load(&[dir.path().to_str().unwrap()]).unwrap();
    let checker = RbsChecker::new(env);

    let ruby_source = indoc! {r#"
        class User
          # ```ruby
          # User.initialize(1, 2, 3, 4, 5)
          # ```
          def initialize
          end
        end
    "#};
    let parsed = rv_ruby_parser::parse(ruby_source.as_bytes());
    let snippets = extract(&parsed);
    assert_eq!(snippets.len(), 1);
    let report = checker.check(&snippets[0], "fixture.rb");
    assert!(report.violations.is_empty());
    assert_eq!(report.stats.checked, 0);
    let entry = &report.stats.unknown_classes["User"];
    assert_eq!(entry.count, 1);
    assert_eq!(
        entry.locations,
        vec![format!("fixture.rb:{}", snippets[0].start_line)]
    );
}

#[test]
fn mixed_calls_report_coverage_split() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("user.rbs"),
        "class User\n  def initialize: (String name, Integer age) -> void\nend\n",
    )
    .unwrap();

    let ruby_source = indoc! {r#"
        class User
          # ```ruby
          # User.initialize("alice", 30)
          # user.save
          # Admin.reset(1)
          # User.new()
          # ```
          def initialize(name, age)
          end
        end
    "#};

    let parsed = rv_ruby_parser::parse(ruby_source.as_bytes());
    let snippets = extract(&parsed);
    assert_eq!(snippets.len(), 1);

    let env = RbsEnvironment::load(&[dir.path().to_str().unwrap()]).unwrap();
    let checker = RbsChecker::new(env);
    let report = checker.check(&snippets[0], "fixture.rb");
    assert!(report.violations.is_empty());
    assert_eq!(report.stats.checked, 1);
    assert_eq!(report.stats.unknown_classes["Admin"].count, 1);
    assert_eq!(report.stats.unknown_methods["User.new"].count, 1);
}
