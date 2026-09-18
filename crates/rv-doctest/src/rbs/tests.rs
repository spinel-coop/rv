use super::*;
use std::path::PathBuf;

fn write_rbs(dir: &Path, name: &str, content: &str) -> PathBuf {
    let path = dir.join(format!("{name}.rbs"));
    std::fs::write(&path, content).unwrap();
    path
}

#[test]
fn test_load_simple_class() {
    let dir = tempfile::tempdir().unwrap();
    write_rbs(
        dir.path(),
        "user",
        "class User\n  def initialize: (String name, Integer age) -> void\nend\n",
    );
    let env = RbsEnvironment::load(&[dir.path().to_str().unwrap()]).unwrap();
    let sig = env.lookup("User", "initialize").unwrap();
    assert_eq!(sig.required_args, 2);
    assert_eq!(sig.optional_args, 0);
    assert!(!sig.has_rest);
    assert!(!sig.has_block);
}

#[test]
fn test_load_module() {
    let dir = tempfile::tempdir().unwrap();
    write_rbs(
        dir.path(),
        "utils",
        "module Utils\n  def self.answer: () -> Integer\nend\n",
    );
    let env = RbsEnvironment::load(&[dir.path().to_str().unwrap()]).unwrap();
    assert!(env.lookup("Utils", "answer").is_some());
}

#[test]
fn test_load_nested_class() {
    let dir = tempfile::tempdir().unwrap();
    write_rbs(
        dir.path(),
        "outer",
        "class Outer\n  class Inner\n    def process: (String) -> void\n  end\nend\n",
    );
    let env = RbsEnvironment::load(&[dir.path().to_str().unwrap()]).unwrap();
    let sig = env.lookup("Outer::Inner", "process").unwrap();
    assert_eq!(sig.required_args, 1);
}

#[test]
fn test_optional_and_rest_args() {
    let dir = tempfile::tempdir().unwrap();
    write_rbs(
        dir.path(),
        "greeter",
        "class Greeter\n  def greet: (String name, ?Integer times, *String extras) -> void\nend\n",
    );
    let env = RbsEnvironment::load(&[dir.path().to_str().unwrap()]).unwrap();
    let sig = env.lookup("Greeter", "greet").unwrap();
    assert_eq!(sig.required_args, 1);
    assert_eq!(sig.optional_args, 1);
    assert!(sig.has_rest);
    assert!(!sig.has_block);
}

#[test]
fn test_arity_too_few() {
    let dir = tempfile::tempdir().unwrap();
    write_rbs(
        dir.path(),
        "user",
        "class User\n  def initialize: (String name, Integer age) -> void\nend\n",
    );
    let env = RbsEnvironment::load(&[dir.path().to_str().unwrap()]).unwrap();
    let result = env.check_arity("User", "initialize", 1, false);
    assert_eq!(
        result,
        ArityResult::TooFew {
            expected: 2,
            found: 1
        }
    );
}

#[test]
fn test_arity_too_many() {
    let dir = tempfile::tempdir().unwrap();
    write_rbs(
        dir.path(),
        "user",
        "class User\n  def initialize: (String name, Integer age) -> void\nend\n",
    );
    let env = RbsEnvironment::load(&[dir.path().to_str().unwrap()]).unwrap();
    let result = env.check_arity("User", "initialize", 3, false);
    assert_eq!(
        result,
        ArityResult::TooMany {
            expected: 2,
            found: 3
        }
    );
}

#[test]
fn test_arity_rest_allows_more() {
    let dir = tempfile::tempdir().unwrap();
    write_rbs(
        dir.path(),
        "greeter",
        "class Greeter\n  def greet: (String name, *String extras) -> void\nend\n",
    );
    let env = RbsEnvironment::load(&[dir.path().to_str().unwrap()]).unwrap();
    let result = env.check_arity("Greeter", "greet", 100, false);
    assert_eq!(result, ArityResult::Valid);
}

#[test]
fn test_arity_valid() {
    let dir = tempfile::tempdir().unwrap();
    write_rbs(
        dir.path(),
        "user",
        "class User\n  def initialize: (String name, Integer age) -> void\nend\n",
    );
    let env = RbsEnvironment::load(&[dir.path().to_str().unwrap()]).unwrap();
    let result = env.check_arity("User", "initialize", 2, false);
    assert_eq!(result, ArityResult::Valid);
}

#[test]
fn test_arity_optional_args() {
    let dir = tempfile::tempdir().unwrap();
    write_rbs(
        dir.path(),
        "greeter",
        "class Greeter\n  def greet: (String name, ?Integer times) -> void\nend\n",
    );
    let env = RbsEnvironment::load(&[dir.path().to_str().unwrap()]).unwrap();
    assert_eq!(
        env.check_arity("Greeter", "greet", 1, false),
        ArityResult::Valid
    );
    assert_eq!(
        env.check_arity("Greeter", "greet", 2, false),
        ArityResult::Valid
    );
    assert_eq!(
        env.check_arity("Greeter", "greet", 3, false),
        ArityResult::TooMany {
            expected: 2,
            found: 3
        }
    );
}

#[test]
fn test_unknown_method_skipped() {
    let dir = tempfile::tempdir().unwrap();
    write_rbs(
        dir.path(),
        "user",
        "class User\n  def initialize: (String name) -> void\nend\n",
    );
    let env = RbsEnvironment::load(&[dir.path().to_str().unwrap()]).unwrap();
    assert_eq!(
        env.check_arity("User", "nonexistent", 0, false),
        ArityResult::Valid
    );
}

#[test]
fn test_unknown_class_skipped() {
    let dir = tempfile::tempdir().unwrap();
    write_rbs(
        dir.path(),
        "user",
        "class User\n  def initialize: (String name) -> void\nend\n",
    );
    let env = RbsEnvironment::load(&[dir.path().to_str().unwrap()]).unwrap();
    assert_eq!(
        env.check_arity("Unknown", "initialize", 1, false),
        ArityResult::Valid
    );
}

#[test]
fn test_multiple_files() {
    let dir = tempfile::tempdir().unwrap();
    write_rbs(
        dir.path(),
        "user",
        "class User\n  def initialize: (String name) -> void\nend\n",
    );
    write_rbs(
        dir.path(),
        "post",
        "class Post\n  def create: (String title, String body) -> void\nend\n",
    );
    let env = RbsEnvironment::load(&[dir.path().to_str().unwrap()]).unwrap();
    assert!(env.lookup("User", "initialize").is_some());
    assert!(env.lookup("Post", "create").is_some());
}

#[test]
fn test_block_capable_method() {
    let dir = tempfile::tempdir().unwrap();
    write_rbs(
        dir.path(),
        "scanner",
        "class Scanner\n  def scan: (String pattern) { (Integer) -> void } -> void\nend\n",
    );
    let env = RbsEnvironment::load(&[dir.path().to_str().unwrap()]).unwrap();
    let sig = env.lookup("Scanner", "scan").unwrap();
    assert!(sig.has_block);
    assert_eq!(
        env.check_arity("Scanner", "scan", 1, true),
        ArityResult::Valid
    );
    assert_eq!(
        env.check_arity("Scanner", "scan", 1, false),
        ArityResult::Valid
    );
}

#[test]
fn test_attr_reader_arity_zero() {
    let dir = tempfile::tempdir().unwrap();
    write_rbs(
        dir.path(),
        "user",
        "class User\n  attr_reader name: String\nend\n",
    );
    let env = RbsEnvironment::load(&[dir.path().to_str().unwrap()]).unwrap();
    assert_eq!(
        env.check_arity("User", "name", 0, false),
        ArityResult::Valid
    );
    assert_eq!(
        env.check_arity("User", "name", 1, false),
        ArityResult::TooMany {
            expected: 0,
            found: 1
        }
    );
}
