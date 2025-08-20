use rv_cache::{CacheKey, CacheKeyHasher};
use std::{fmt::Display, str::FromStr};

use crate::{Ruby, engine::RubyEngine};
use serde_with::{DeserializeFromStr, SerializeDisplay};

type VersionPart = u32;

#[derive(Debug, Clone, PartialEq, Eq, DeserializeFromStr, SerializeDisplay)]
pub struct RubyRequest {
    pub engine: RubyEngine,
    pub major: Option<VersionPart>,
    pub minor: Option<VersionPart>,
    pub patch: Option<VersionPart>,
    pub tiny: Option<VersionPart>,
    pub prerelease: Option<String>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RequestError {
    #[error("Empty input")]
    EmptyInput,
    #[error("Could not parse version: {0}")]
    InvalidVersion(String),
    #[error("Could not parse version {0}, no more than 4 numbers are allowed")]
    TooManySegments(String),
    #[error("Could not parse {0}: {1}")]
    InvalidPart(&'static str, String),
}
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum MatchError {
    #[error("Ruby version {0} could not be found")]
    NotFound(String),
}

impl Default for RubyRequest {
    fn default() -> Self {
        RubyRequest {
            engine: RubyEngine::Ruby,
            major: None,
            minor: None,
            patch: None,
            tiny: None,
            prerelease: None,
        }
    }
}

impl RubyRequest {
    pub fn find_match_in(self, rubies: &[Ruby]) -> Result<&Ruby, MatchError> {
        rubies
            .iter()
            .find(|r| self.satisfied_by(r))
            .ok_or(MatchError::NotFound(self.to_string()))
    }

    pub fn satisfied_by(&self, ruby: &Ruby) -> bool {
        let version = &ruby.version;

        if self.engine != version.engine {
            return false;
        }
        if self.major.is_some() && self.major != version.major {
            return false;
        }
        if self.minor.is_some() && self.minor != version.minor {
            return false;
        }
        if self.patch.is_some() && self.patch != version.patch {
            return false;
        }
        if self.tiny.is_some() && self.tiny != version.tiny {
            return false;
        }
        if self.prerelease.is_some() && self.prerelease != version.prerelease {
            return false;
        }

        true
    }

    pub fn number(&self) -> String {
        let mut version = String::new();
        if let Some(major) = self.major {
            version.push_str(&major.to_string());
        }
        if let Some(minor) = self.minor {
            version.push('.');
            version.push_str(&minor.to_string());
        }
        if let Some(patch) = self.patch {
            version.push('.');
            version.push_str(&patch.to_string());
        }
        if let Some(tiny) = self.tiny {
            version.push('.');
            version.push_str(&tiny.to_string());
        }
        if let Some(ref prerelease) = self.prerelease {
            if self.major.is_some() {
                version.push('-');
            }
            version.push_str(prerelease);
        }
        version
    }
}

impl FromStr for RubyRequest {
    type Err = RequestError;
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let input = input.trim();
        let first_char = input.chars().next().ok_or(RequestError::EmptyInput)?;
        let (engine, version) = if first_char.is_alphabetic() {
            input.split_once('-').unwrap_or((input, ""))
        } else {
            ("ruby", input)
        };
        let mut segments: Vec<String> = vec![];
        let mut prerelease = None;

        let first_char = version.chars().next();
        if let Some(first_char) = first_char {
            let (numbers, pre) = if first_char.is_alphabetic() {
                if version == "dev" {
                    (None, Some(version.to_string()))
                } else {
                    Err(RequestError::InvalidVersion(input.to_string()))?
                }
            } else if let Some(pos) = version.find('-') {
                (
                    Some(version[..pos].to_string()),
                    Some(version[pos + 1..].to_string()),
                )
            } else {
                (Some(version.to_string()), None)
            };

            segments = if let Some(rest) = numbers {
                rest.split('.')
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>()
            } else {
                vec![]
            };

            if segments.len() > 4 {
                return Err(RequestError::TooManySegments(input.to_string()));
            }

            prerelease = pre;
        };

        let major = if !segments.is_empty() {
            Some(
                segments[0]
                    .parse::<u32>()
                    .map_err(|_| RequestError::InvalidPart("major version", input.to_string()))?,
            )
        } else {
            None
        };
        let minor = if segments.len() > 1 {
            Some(
                segments[1]
                    .parse::<u32>()
                    .map_err(|_| RequestError::InvalidPart("minor version", input.to_string()))?,
            )
        } else {
            None
        };
        let patch = if segments.len() > 2 {
            Some(
                segments[2]
                    .parse::<u32>()
                    .map_err(|_| RequestError::InvalidPart("patch version", input.to_string()))?,
            )
        } else {
            None
        };
        let tiny = if segments.len() > 3 {
            Some(
                segments[3]
                    .parse::<u32>()
                    .map_err(|_| RequestError::InvalidPart("tiny version", input.to_string()))?,
            )
        } else {
            None
        };

        Ok(RubyRequest {
            engine: engine.into(),
            major,
            minor,
            patch,
            tiny,
            prerelease,
        })
    }
}

impl From<String> for RubyRequest {
    fn from(val: String) -> Self {
        Self::from_str(&val).expect("Failed to parse string: {val}")
    }
}

impl From<&str> for RubyRequest {
    fn from(val: &str) -> Self {
        Self::from_str(val).expect("Failed to parse string: {val}")
    }
}

impl Display for RubyRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.engine)?;

        if let Some(major) = self.major {
            write!(f, "-{major}")?;
            if let Some(minor) = self.minor {
                write!(f, ".{minor}")?;
                if let Some(patch) = self.patch {
                    write!(f, ".{patch}")?;
                    if let Some(tiny) = self.tiny {
                        write!(f, ".{tiny}")?;
                    }
                }
            }
        }

        if let Some(ref pre_release) = self.prerelease {
            write!(f, "-{pre_release}")?;
        };

        Ok(())
    }
}

impl PartialOrd for RubyRequest {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RubyRequest {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (
            &self.engine,
            &self.major,
            &self.minor,
            &self.patch,
            &self.tiny,
            &self.prerelease,
        )
            .cmp(&(
                &other.engine,
                &other.major,
                &other.minor,
                &other.patch,
                &other.tiny,
                &other.prerelease,
            ))
    }
}

impl CacheKey for RubyRequest {
    fn cache_key(&self, state: &mut CacheKeyHasher) {
        // Cache key includes all version components that define a unique Ruby request
        self.engine.cache_key(state);
        self.major.cache_key(state);
        self.minor.cache_key(state);
        self.patch.cache_key(state);
        self.tiny.cache_key(state);
        self.prerelease.cache_key(state);
    }
}

#[test]
fn test_empty_version() {
    let request = RubyRequest::from_str("").expect_err("Expected error for empty version");
    assert_eq!(request, RequestError::EmptyInput);
}

#[test]
fn test_invalid_version_format() {
    let request = RubyRequest::from_str("ruby-invalid")
        .expect_err("Expected error for invalid version format");
    assert_eq!(request, RequestError::InvalidVersion("ruby-invalid".into()));
}

#[test]
fn test_adding_ruby_engine() {
    let request = RubyRequest::from_str("3.0.0").expect("Failed to parse version");
    assert_eq!(request.engine, "ruby".into());
    assert_eq!(request.major, Some(3));
    assert_eq!(request.minor, Some(0));
    assert_eq!(request.patch, Some(0));
    assert_eq!(request.tiny, None);
    assert_eq!(request.prerelease, None);
}

#[test]
fn test_major_only() {
    let request = RubyRequest::from_str("3").expect("Failed to parse version");
    assert_eq!(request.engine, "ruby".into());
    assert_eq!(request.major, Some(3));
    assert_eq!(request.minor, None);
    assert_eq!(request.patch, None);
    assert_eq!(request.tiny, None);
    assert_eq!(request.prerelease, None);
}

#[test]
fn test_parsing_supported_ruby_versions() {
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
        let request = RubyRequest::from_str(version).expect("Failed to parse version");
        let output = request.to_string();
        assert_eq!(
            output, version,
            "Parsed output does not match input for {version}"
        );
    }
}

#[test]
fn test_parsing_partial_requests() {
    let versions = [
        "ruby-3",
        "ruby-3.2-preview1",
        "ruby-3-rc",
        "jruby-9.4",
        "truffleruby-24.1",
        "mruby-3.2",
        "artichoke",
        "jruby-9",
        "jruby",
    ];
    for version in versions {
        let request = RubyRequest::from_str(version).expect("Failed to parse version");
        let output = request.to_string();
        assert_eq!(
            output, version,
            "Parsed output does not match input for {version}"
        );
    }
}

#[test]
fn test_parsing_engine_without_version() {
    let request = RubyRequest::from_str("jruby-").unwrap();
    assert_eq!(request.engine, "jruby".into());
    assert_eq!(request.major, None);
    assert_eq!(request.minor, None);
    assert_eq!(request.patch, None);
    assert_eq!(request.tiny, None);
    assert_eq!(request.prerelease, None);
}

#[test]
fn test_parsing_ruby_version_files() {
    let versions = [
        "ruby\n",
        "ruby-3\n",
        "ruby-3.2-preview1\n",
        "ruby-3-rc\n",
        "jruby-9.4\n",
        "truffleruby-24.1\n",
        "mruby-3.2\n",
        "artichoke\n",
        "jruby-9\n",
        "jruby\n",
        "truffleruby+graalvm-24.1.0\n",
    ];
    for version in versions {
        let request = RubyRequest::from_str(version).expect("Failed to parse version");
        let output = request.to_string();
        assert_eq!(
            output,
            version.trim(),
            "Parsed output does not match input for {version}"
        );
    }
}

#[test]
fn test_parsing_ruby_version_files_without_engine() {
    let versions = ["3\n", "3.4\n", "3.4.5\n", "3.4.5-rc1\n", "3.4-dev\n"];
    for version in versions {
        let request = RubyRequest::from_str(version).expect("Failed to parse version");
        let output = request.to_string();
        assert_eq!(
            output,
            format!("ruby-{}", version.trim()),
            "Parsed output does not match input for {version}"
        );
    }
}
