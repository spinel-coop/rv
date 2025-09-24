use crate::request::RubyRequest;

pub type RubyVersion = RubyRequest;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_parsing_supported_ruby_versions() {
        use std::str::FromStr as _;

        let versions = [
            "ruby-3.2-dev",
            "ruby-3.2.0",
            "ruby-3.2.0-preview1",
            "ruby-3.2.0-preview2",
            "ruby-3.2.0-preview3",
            "ruby-3.2.0-rc1",
            "ruby-3.2.1",
            "ruby-3.2.2",
            "ruby-3.2.3",
            "ruby-3.2.4",
            "ruby-3.2.5",
            "ruby-3.2.6",
            "ruby-3.2.7",
            "ruby-3.2.8",
            "ruby-3.2.9",
            "ruby-3.3-dev",
            "ruby-3.3.0",
            "ruby-3.3.0-preview1",
            "ruby-3.3.0-preview2",
            "ruby-3.3.0-preview3",
            "ruby-3.3.0-rc1",
            "ruby-3.3.1",
            "ruby-3.3.2",
            "ruby-3.3.3",
            "ruby-3.3.4",
            "ruby-3.3.5",
            "ruby-3.3.6",
            "ruby-3.3.7",
            "ruby-3.3.8",
            "ruby-3.3.9",
            "ruby-3.4-dev",
            "ruby-3.4.0",
            "ruby-3.4.0-preview1",
            "ruby-3.4.0-preview2",
            "ruby-3.4.0-rc1",
            "ruby-3.4.1",
            "ruby-3.4.2",
            "ruby-3.4.3",
            "ruby-3.4.4",
            "ruby-3.4.5",
            "ruby-3.5-dev",
            "ruby-3.5.0-preview1",
            "artichoke-dev",
            "jruby-9.4.0.0",
            "jruby-9.4.1.0",
            "jruby-9.4.10.0",
            "jruby-9.4.11.0",
            "jruby-9.4.12.0",
            "jruby-9.4.12.1",
            "jruby-9.4.13.0",
            "jruby-9.4.2.0",
            "jruby-9.4.3.0",
            "jruby-9.4.4.0",
            "jruby-9.4.5.0",
            "jruby-9.4.6.0",
            "jruby-9.4.7.0",
            "jruby-9.4.8.0",
            "jruby-9.4.9.0",
            "jruby-dev",
            "mruby-3.2.0",
            "mruby-3.3.0",
            "mruby-3.4.0",
            "mruby-dev",
            "picoruby-3.0.0",
            "ruby-dev",
            "truffleruby-24.1.0",
            "truffleruby-24.1.1",
            "truffleruby-24.1.2",
            "truffleruby-24.2.0",
            "truffleruby-24.2.1",
            "truffleruby-dev",
            "truffleruby+graalvm-24.1.0",
            "truffleruby+graalvm-24.1.1",
            "truffleruby+graalvm-24.1.2",
            "truffleruby+graalvm-24.2.0",
            "truffleruby+graalvm-24.2.1",
            "truffleruby+graalvm-dev",
        ];

        for version in versions {
            let request = RubyVersion::from_str(version).expect("Failed to parse version");
            let output = request.to_string();
            assert_eq!(
                output, version,
                "Parsed output does not match input for {version}"
            );
        }
    }
}
