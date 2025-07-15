use rv_lockfile::{parse_lockfile, parse_lockfile_strict, ParseError};
use insta::assert_debug_snapshot;
use std::fs;
use std::path::PathBuf;

/// Helper function to load a test fixture
fn load_fixture(name: &str) -> String {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/fixtures");
    path.push(format!("{}.gemfile.lock", name));
    
    fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("Failed to read fixture: {}", path.display()))
}

/// Test a lockfile fixture and create a snapshot
fn test_fixture(name: &str) {
    let content = load_fixture(name);
    let parser = parse_lockfile(&content).unwrap();
    assert_debug_snapshot!(format!("fixture_{}", name), parser);
}

/// Test a lockfile fixture with strict mode and create a snapshot
fn test_fixture_strict(name: &str) {
    let content = load_fixture(name);
    let parser = parse_lockfile_strict(&content).unwrap();
    assert_debug_snapshot!(format!("fixture_{}_strict", name), parser);
}

/// Test a lockfile fixture that should fail parsing
fn test_fixture_error(name: &str) {
    let content = load_fixture(name);
    let result = parse_lockfile(&content);
    assert!(result.is_err());
    assert_debug_snapshot!(format!("fixture_{}_error", name), result.unwrap_err());
}

#[test]
fn test_empty_lockfile() {
    test_fixture("empty");
}

#[test]
fn test_minimal_lockfile() {
    test_fixture("minimal");
}

#[test]
fn test_git_source_lockfile() {
    test_fixture("git_source");
}

#[test]
fn test_gem_source_lockfile() {
    test_fixture("gem_source");
}

#[test]
fn test_multi_platform_lockfile() {
    test_fixture("multi_platform");
}

#[test]
fn test_path_source_lockfile() {
    test_fixture("path_source");
}

#[test]
fn test_checksum_enabled_lockfile() {
    test_fixture("checksum_enabled");
}

#[test]
fn test_rails_app_lockfile() {
    test_fixture("rails_app");
}

// Test the same fixtures in strict mode
#[test]
fn test_empty_lockfile_strict() {
    test_fixture_strict("empty");
}

#[test]
fn test_minimal_lockfile_strict() {
    test_fixture_strict("minimal");
}

#[test]
fn test_git_source_lockfile_strict() {
    test_fixture_strict("git_source");
}

// Create inline test for merge conflict since it can't be a file
#[test]
fn test_merge_conflict_detection() {
    let conflict_content = r#"
GEM
  remote: https://rubygems.org/
  specs:
<<<<<<< HEAD
    gem-a (1.0.0)
=======
    gem-b (2.0.0)
>>>>>>> feature-branch

PLATFORMS
  ruby
"#;
    
    let result = parse_lockfile(conflict_content);
    assert!(result.is_err());
    
    if let Err(ParseError::MergeConflict { line }) = result {
        assert_eq!(line, 5); // Line with <<<<<<< HEAD
    } else {
        panic!("Expected MergeConflict error, got: {:?}", result);
    }
}