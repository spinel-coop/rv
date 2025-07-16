use insta::assert_debug_snapshot;
use rv_lockfile::{parse_lockfile, parse_lockfile_strict};

#[test]
fn test_empty_lockfile() {
    let empty_content = "";
    let parser = parse_lockfile(empty_content).unwrap();

    assert_debug_snapshot!(parser, @r"
    LockfileParser {
        sources: [],
        specs: {},
        dependencies: {},
        platforms: [],
        bundler_version: None,
        ruby_version: None,
        checksums_enabled: false,
        checksums: {},
        strict: false,
    }
    ");
}

#[test]
fn test_whitespace_only_lockfile() {
    let whitespace_content = "   \n\n  \t  \n   ";
    let parser = parse_lockfile(whitespace_content).unwrap();

    assert_debug_snapshot!(parser, @r"
    LockfileParser {
        sources: [],
        specs: {},
        dependencies: {},
        platforms: [],
        bundler_version: None,
        ruby_version: None,
        checksums_enabled: false,
        checksums: {},
        strict: false,
    }
    ");
}

#[test]
fn test_empty_lockfile_strict_mode() {
    let empty_content = "";
    let parser = parse_lockfile_strict(empty_content).unwrap();

    assert_debug_snapshot!(parser, @r"
    LockfileParser {
        sources: [],
        specs: {},
        dependencies: {},
        platforms: [],
        bundler_version: None,
        ruby_version: None,
        checksums_enabled: false,
        checksums: {},
        strict: true,
    }
    ");
}

#[test]
fn test_comments_only_lockfile() {
    // Test that lines that don't match any section are ignored
    let comment_content = r#"
# This is a comment
# Another comment
"#;
    let parser = parse_lockfile(comment_content).unwrap();

    assert_debug_snapshot!(parser, @r"
    LockfileParser {
        sources: [],
        specs: {},
        dependencies: {},
        platforms: [],
        bundler_version: None,
        ruby_version: None,
        checksums_enabled: false,
        checksums: {},
        strict: false,
    }
    ");
}

#[test]
fn test_minimal_valid_lockfile() {
    let minimal_content = r#"
PLATFORMS
  ruby

BUNDLED WITH
   2.3.0
"#;
    let parser = parse_lockfile(minimal_content).unwrap();

    assert_debug_snapshot!(parser, @r"
    LockfileParser {
        sources: [],
        specs: {},
        dependencies: {},
        platforms: [
            Ruby,
        ],
        bundler_version: Some(
            Version {
                major: 2,
                minor: 3,
                patch: 0,
            },
        ),
        ruby_version: None,
        checksums_enabled: false,
        checksums: {},
        strict: false,
    }
    ");
}
