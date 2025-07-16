use insta::assert_debug_snapshot;
use rv_lockfile::{parse_lockfile, ParseError};

#[test]
fn test_git_source_lockfile() {
    let git_content = r#"
GIT
  remote: https://github.com/rails/rails.git
  revision: 1234567890abcdef1234567890abcdef12345678
  ref: v7.0.0
  specs:
    rails (7.0.0)
      actioncable (= 7.0.0)
      actionmailbox (= 7.0.0)

PLATFORMS
  ruby

DEPENDENCIES
  rails!

BUNDLED WITH
   2.3.0
"#;

    let parser = parse_lockfile(git_content).unwrap();
    assert_debug_snapshot!(parser);
}

#[test]
fn test_gem_source_lockfile() {
    let gem_content = r#"
GEM
  remote: https://rubygems.org/
  specs:
    activerecord (7.0.4)
      activemodel (= 7.0.4)
      activesupport (= 7.0.4)
    activesupport (7.0.4)
      concurrent-ruby (~> 1.0, >= 1.0.2)
      i18n (>= 1.6, < 2)
    concurrent-ruby (1.1.10)
    i18n (1.12.0)
      concurrent-ruby (~> 1.0)

PLATFORMS
  ruby
  x86_64-linux

DEPENDENCIES
  activerecord (~> 7.0)
  concurrent-ruby

RUBY VERSION
   ruby 3.1.0p0

BUNDLED WITH
   2.3.0
"#;

    let parser = parse_lockfile(gem_content).unwrap();
    assert_debug_snapshot!(parser);
}

#[test]
fn test_path_source_lockfile() {
    let path_content = r#"
PATH
  remote: ../local-gem
  specs:
    local-gem (1.0.0)
      some-dependency (~> 2.0)

GEM
  remote: https://rubygems.org/
  specs:
    some-dependency (2.1.0)

PLATFORMS
  ruby

DEPENDENCIES
  local-gem!
  some-dependency

BUNDLED WITH
   2.3.0
"#;

    let parser = parse_lockfile(path_content).unwrap();
    assert_debug_snapshot!(parser);
}

#[test]
fn test_multiple_platforms_lockfile() {
    let platforms_content = r#"
GEM
  remote: https://rubygems.org/
  specs:
    ffi (1.15.5)
    ffi (1.15.5-x64-mingw32)
    ffi (1.15.5-x86_64-linux)

PLATFORMS
  ruby
  x64-mingw32
  x86_64-linux

DEPENDENCIES
  ffi

BUNDLED WITH
   2.3.0
"#;

    let parser = parse_lockfile(platforms_content).unwrap();
    assert_debug_snapshot!(parser);
}

#[test]
fn test_checksum_enabled_lockfile() {
    let checksum_content = r#"
GEM
  remote: https://rubygems.org/
  specs:
    minitest (5.16.3)

PLATFORMS
  ruby

DEPENDENCIES
  minitest

CHECKSUMS
  minitest (5.16.3) sha256=3c8fb073e5353d086d12af3a8822ef7e4dc1df3a5de1a1b1b48c6ed59d1de7fc

BUNDLED WITH
   2.5.0
"#;

    let parser = parse_lockfile(checksum_content).unwrap();
    assert_debug_snapshot!(parser);
}

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

    if let Err(ParseError::MergeConflict { line, .. }) = result {
        assert_eq!(line, 5); // Line with <<<<<<< HEAD
    } else {
        panic!("Expected MergeConflict error");
    }
}

#[test]
fn test_malformed_gem_spec_strict_mode() {
    let malformed_content = r#"
GEM
  remote: https://rubygems.org/
  specs:
    invalid-gem-spec without version

PLATFORMS
  ruby
"#;

    let result = parse_lockfile(malformed_content);
    assert!(result.is_err());
    assert_debug_snapshot!(result.unwrap_err());
}


#[test]
fn test_plugin_source_lockfile() {
    let plugin_content = r#"
PLUGIN SOURCE
  custom_option: value
  another_option: another_value
  specs:
    custom-gem (1.0.0)

PLATFORMS
  ruby

DEPENDENCIES
  custom-gem

BUNDLED WITH
   2.3.0
"#;

    let parser = parse_lockfile(plugin_content).unwrap();
    assert_debug_snapshot!(parser);
}
