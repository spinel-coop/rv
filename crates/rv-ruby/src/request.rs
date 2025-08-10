use std::str::FromStr;

#[derive(Debug, Clone, PartialEq)]
pub struct VersionRequest {
    pub engine: String,
    pub major: Option<u32>,
    pub minor: Option<u32>,
    pub patch: Option<u32>,
    pub tiny: Option<u32>,
    pub pre_release: Option<String>,
}

impl VersionRequest {
    pub fn parse(input: &str) -> Result<Self, String> {
        let first_char = input.chars().next().ok_or("Empty input")?;
        let (engine, rest) = if first_char.is_alphabetic() {
            input.split_once('-').unwrap_or(("", input))
        } else {
            ("ruby", input)
        };

        let first_char = rest.chars().next().ok_or("Empty version part")?;
        let (rest, pre) = if first_char.is_alphabetic() {
            if rest == "dev" {
                (None, Some(rest.to_string()))
            } else {
                Err(format!("Invalid version format: {}", input))?
            }
        } else {
            if let Some(pos) = rest.find('-') {
                (
                    Some(rest[..pos].to_string()),
                    Some(rest[pos + 1..].to_string()),
                )
            } else {
                (Some(rest.to_string()), None)
            }
        };

        let segments = if let Some(rest) = rest {
            rest.split('.')
                .map(|s| s.to_string())
                .collect::<Vec<String>>()
        } else {
            vec![]
        };

        if segments.len() > 4 {
            return Err(format!(
                "Invalid version {}, no more than 4 numbers are allowed",
                input
            ));
        }

        let major = if segments.len() > 0 {
            Some(
                segments[0]
                    .parse::<u32>()
                    .map_err(|_| format!("Invalid major version in {}", input))?,
            )
        } else {
            None
        };
        let minor = if segments.len() > 1 {
            Some(
                segments[1]
                    .parse::<u32>()
                    .map_err(|_| format!("Invalid minor version in {}", input))?,
            )
        } else {
            None
        };
        let patch = if segments.len() > 2 {
            Some(
                segments[2]
                    .parse::<u32>()
                    .map_err(|_| format!("Invalid patch version in {}", input))?,
            )
        } else {
            None
        };
        let tiny = if segments.len() > 3 {
            Some(
                segments[3]
                    .parse::<u32>()
                    .map_err(|_| format!("Invalid tiny version in {}", input))?,
            )
        } else {
            None
        };

        Ok(VersionRequest {
            engine: engine.to_string(),
            major: major,
            minor: minor,
            patch: patch,
            tiny: tiny,
            pre_release: pre,
        })
    }

    pub fn to_string(&self) -> String {
        let mut version = format!("{}", self.engine);

        if let Some(major) = self.major {
            version.push_str(&format!("-{}", major));
            if let Some(minor) = self.minor {
                version.push_str(&format!(".{}", minor));
                if let Some(patch) = self.patch {
                    version.push_str(&format!(".{}", patch));
                    if let Some(tiny) = self.tiny {
                        version.push_str(&format!(".{}", tiny));
                    }
                }
            }
        }

        if let Some(ref pre_release) = self.pre_release {
            version.push_str(&format!("-{}", pre_release));
        }

        version
    }
}

impl FromStr for VersionRequest {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

#[test]
fn test_empty_version() {
    let request = VersionRequest::parse("").expect_err("Expected error for empty version");
    assert_eq!(request, "Empty input");
}

#[test]
fn test_invalid_version_format() {
    let request = VersionRequest::parse("ruby-invalid")
        .expect_err("Expected error for invalid version format");
    assert!(request.contains("Invalid version"));
}

#[test]
fn test_adding_ruby_engine() {
    let request = VersionRequest::parse("3.0.0").expect("Failed to parse version");
    assert_eq!(request.engine, "ruby");
    assert_eq!(request.major, Some(3));
    assert_eq!(request.minor, Some(0));
    assert_eq!(request.patch, Some(0));
    assert_eq!(request.tiny, None);
    assert_eq!(request.pre_release, None);
}

#[test]
fn test_major_only() {
    let request = VersionRequest::parse("3").expect("Failed to parse version");
    assert_eq!(request.engine, "ruby");
    assert_eq!(request.major, Some(3));
    assert_eq!(request.minor, None);
    assert_eq!(request.patch, None);
    assert_eq!(request.tiny, None);
    assert_eq!(request.pre_release, None);
}

#[test]
fn test_parsing_supported_ruby_versions() {
    let versions = [
        "ruby-3.0-dev",
        "ruby-3.0.0",
        "ruby-3.0.0-preview1",
        "ruby-3.0.0-preview2",
        "ruby-3.0.0-rc1",
        "ruby-3.0.1",
        "ruby-3.0.2",
        "ruby-3.0.3",
        "ruby-3.0.4",
        "ruby-3.0.5",
        "ruby-3.0.6",
        "ruby-3.0.7",
        "ruby-3.1-dev",
        "ruby-3.1.0",
        "ruby-3.1.0-preview1",
        "ruby-3.1.1",
        "ruby-3.1.2",
        "ruby-3.1.3",
        "ruby-3.1.4",
        "ruby-3.1.5",
        "ruby-3.1.6",
        "ruby-3.1.7",
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
        "jruby-9.3.0.0",
        "jruby-9.3.1.0",
        "jruby-9.3.10.0",
        "jruby-9.3.11.0",
        "jruby-9.3.13.0",
        "jruby-9.3.14.0",
        "jruby-9.3.15.0",
        "jruby-9.3.2.0",
        "jruby-9.3.3.0",
        "jruby-9.3.4.0",
        "jruby-9.3.6.0",
        "jruby-9.3.7.0",
        "jruby-9.3.8.0",
        "jruby-9.3.9.0",
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
        "mruby-3.0.0",
        "mruby-3.1.0",
        "mruby-3.2.0",
        "mruby-3.3.0",
        "mruby-3.4.0",
        "mruby-dev",
        "picoruby-3.0.0",
        "ruby-dev",
        "truffleruby-24.0.0",
        "truffleruby-24.0.1",
        "truffleruby-24.0.2",
        "truffleruby-24.1.0",
        "truffleruby-24.1.1",
        "truffleruby-24.1.2",
        "truffleruby-24.2.0",
        "truffleruby-24.2.1",
        "truffleruby-dev",
        "truffleruby+graalvm-24.0.0",
        "truffleruby+graalvm-24.0.1",
        "truffleruby+graalvm-24.0.2",
        "truffleruby+graalvm-24.1.0",
        "truffleruby+graalvm-24.1.1",
        "truffleruby+graalvm-24.1.2",
        "truffleruby+graalvm-24.2.0",
        "truffleruby+graalvm-24.2.1",
        "truffleruby+graalvm-dev",
    ];

    for version in versions {
        let request = VersionRequest::parse(version).expect("Failed to parse version");
        let output = request.to_string();
        assert_eq!(
            output, version,
            "Parsed output does not match input for {}",
            version
        );
    }
}
