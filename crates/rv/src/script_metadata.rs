use rv_ruby::request::RubyRequest;
use std::str::FromStr;
use tracing::warn;

#[derive(Debug, Default)]
pub struct ScriptMetadata {
    pub requires_ruby: Option<RubyRequest>,
}

pub fn parse(content: &str) -> Option<ScriptMetadata> {
    let mut in_block = false;
    let mut metadata = ScriptMetadata::default();
    let mut found_block = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed == "# /// script" {
            in_block = true;
            found_block = true;
            continue;
        }

        if in_block && trimmed == "# ///" {
            break;
        }

        if in_block {
            let content_line = if let Some(stripped) = trimmed.strip_prefix("# ") {
                stripped
            } else if trimmed == "#" {
                continue;
            } else {
                warn!("Script metadata line missing '# ' prefix: {}", line);
                continue;
            };

            if let Some(("requires-ruby", value)) = parse_key_value(content_line) {
                match RubyRequest::from_str(value) {
                    Ok(request) => metadata.requires_ruby = Some(request),
                    Err(e) => warn!("Invalid Ruby version '{}': {}", value, e),
                }
            }
        }
    }

    if found_block { Some(metadata) } else { None }
}

fn parse_key_value(line: &str) -> Option<(&str, &str)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }

    let (key, rest) = line.split_once('=')?;
    let key = key.trim();
    let value = rest.trim();
    let value = value.strip_prefix('"')?.strip_suffix('"')?;

    Some((key, value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_metadata() {
        let content = r#"# /// script
# requires-ruby = "3.4"
# ///

puts "Hello"
"#;

        let metadata = parse(content).expect("should parse metadata");
        let ruby = metadata.requires_ruby.expect("should have ruby version");
        let RubyRequest::Released(ruby) = ruby else {
            panic!("Unexpected ruby version {ruby:?}")
        };
        assert_eq!(ruby.major, Some(3));
        assert_eq!(ruby.minor, Some(4));
        assert_eq!(ruby.patch, None);
    }

    #[test]
    fn test_parse_with_shebang() {
        let content = r#"#!/usr/bin/env rv run
# /// script
# requires-ruby = "3.4.1"
# ///

puts RUBY_VERSION
"#;

        let metadata = parse(content).expect("should parse metadata");
        let ruby = metadata.requires_ruby.expect("should have ruby version");
        let RubyRequest::Released(ruby) = ruby else {
            panic!("Unexpected ruby version {ruby:?}")
        };
        assert_eq!(ruby.major, Some(3));
        assert_eq!(ruby.minor, Some(4));
        assert_eq!(ruby.patch, Some(1));
    }

    #[test]
    fn test_parse_no_metadata() {
        let content = r#"puts "Hello, World!"
"#;

        assert!(parse(content).is_none());
    }

    #[test]
    fn test_parse_empty_block() {
        let content = r#"# /// script
# ///

puts "Hello"
"#;

        let metadata = parse(content).expect("should parse empty block");
        assert!(metadata.requires_ruby.is_none());
    }

    #[test]
    fn test_parse_full_version() {
        let content = r#"# /// script
# requires-ruby = "ruby-3.4.0-preview1"
# ///
"#;

        let metadata = parse(content).expect("should parse metadata");
        let ruby = metadata.requires_ruby.expect("should have ruby version");
        let RubyRequest::Released(ruby) = ruby else {
            panic!("Unexpected ruby version {ruby:?}")
        };
        assert_eq!(ruby.major, Some(3));
        assert_eq!(ruby.minor, Some(4));
        assert_eq!(ruby.patch, Some(0));
        assert_eq!(ruby.prerelease, Some("preview1".to_string()));
    }

    #[test]
    fn test_parse_with_extra_whitespace() {
        let content = r#"# /// script
#   requires-ruby   =   "3.3"
# ///
"#;

        let metadata = parse(content).expect("should parse metadata");
        let ruby = metadata.requires_ruby.expect("should have ruby version");
        let RubyRequest::Released(ruby) = ruby else {
            panic!("Unexpected ruby version {ruby:?}")
        };
        assert_eq!(ruby.major, Some(3));
        assert_eq!(ruby.minor, Some(3));
    }

    #[test]
    fn test_parse_unknown_keys_ignored() {
        let content = r#"# /// script
# requires-ruby = "3.4"
# unknown-key = "some-value"
# dependencies = ["rake", "rspec"]
# ///
"#;

        let metadata = parse(content).expect("should parse metadata");
        assert!(metadata.requires_ruby.is_some());
    }

    #[test]
    fn test_parse_stops_at_end_marker() {
        let content = r#"# /// script
# requires-ruby = "3.4"
# ///
# requires-ruby = "3.3"
"#;

        let metadata = parse(content).expect("should parse metadata");
        let ruby = metadata.requires_ruby.expect("should have ruby version");
        let RubyRequest::Released(ruby) = ruby else {
            panic!("Unexpected ruby version {ruby:?}")
        };
        assert_eq!(ruby.minor, Some(4)); // Should be 3.4, not 3.3
    }

    #[test]
    fn test_parse_jruby_version() {
        let content = r#"# /// script
# requires-ruby = "jruby-9.4"
# ///
"#;

        let metadata = parse(content).expect("should parse metadata");
        let ruby = metadata.requires_ruby.expect("should have ruby version");
        let RubyRequest::Released(ruby) = ruby else {
            panic!("Unexpected ruby version {ruby:?}")
        };
        assert_eq!(ruby.engine.to_string(), "jruby");
        assert_eq!(ruby.major, Some(9));
        assert_eq!(ruby.minor, Some(4));
    }
}
