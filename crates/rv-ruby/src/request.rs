use camino::Utf8PathBuf;
use rv_cache::{CacheKey, CacheKeyHasher};
use std::{fmt::Display, str::FromStr};

use crate::{Ruby, engine::RubyEngine};
use serde_with::{DeserializeFromStr, SerializeDisplay};

pub type VersionPart = u32;

/// A range of possible Ruby versions. E.g. "3.4" spans the range 3.4.0, 3.4.1, etc.
/// This is different to a RubyVersion, which is one specific version in this requested range.
#[derive(Debug, Clone, PartialEq, Eq, DeserializeFromStr, SerializeDisplay)]
pub struct RubyRequest {
    pub engine: RubyEngine,
    pub major: Option<VersionPart>,
    pub minor: Option<VersionPart>,
    pub patch: Option<VersionPart>,
    pub tiny: Option<VersionPart>,
    pub prerelease: Option<String>,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Source {
    DotToolVersions(Utf8PathBuf),
    DotRubyVersion(Utf8PathBuf),
    GemfileLock(Utf8PathBuf),
}

impl std::fmt::Debug for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DotToolVersions(arg0) => f.debug_tuple("DotToolVersions").field(arg0).finish(),
            Self::DotRubyVersion(arg0) => f.debug_tuple("DotRubyVersion").field(arg0).finish(),
            Self::GemfileLock(arg0) => f.debug_tuple("GemfileLock").field(arg0).finish(),
        }
    }
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

impl PartialOrd for RubyRequest {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RubyRequest {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;

        if self.major != other.major {
            self.major.cmp(&other.major)
        } else if self.minor != other.minor {
            self.minor.cmp(&other.minor)
        } else if self.patch != other.patch {
            self.patch.cmp(&other.patch)
        } else if self.tiny != other.tiny {
            self.tiny.cmp(&other.tiny)
        } else {
            match (&self.prerelease, &other.prerelease) {
                (None, None) => Ordering::Equal,
                (None, Some(_prerelease)) => Ordering::Greater,
                (Some(_prerelease), None) => Ordering::Less,
                (prerelease, other_prerelease) => prerelease.cmp(other_prerelease),
            }
        }
    }
}

impl RubyRequest {
    /// Resolve the Ruby request to a specific version of ruby, chosen from
    /// the given list.
    pub fn find_match_in(&self, rubies: &[Ruby]) -> Option<Ruby> {
        rubies
            .iter()
            .rev()
            .find(|r| r.version.satisfies(self))
            .cloned()
    }

    /// A version that toolfiles like .tool-version/.ruby-version/Gemfile/Gemfile.lock knows how to read.
    pub fn to_tool_consumable_version(&self) -> String {
        self.to_string().replace("ruby-", "")
    }
}

impl FromStr for RubyRequest {
    type Err = RequestError;
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let input = input.trim();
        let first_char = input.chars().next().ok_or(RequestError::EmptyInput)?;
        let (engine, version) = if input == "latest" {
            ("ruby", "")
        } else if first_char.is_alphabetic() {
            input.split_once('-').unwrap_or((input, ""))
        } else {
            ("ruby", input)
        };
        let mut segments: Option<_> = None;
        let mut prerelease = None;

        let first_char = version.chars().next();
        if let Some(first_char) = first_char {
            let (numbers, pre) = if first_char.is_alphabetic() {
                if version == "dev" {
                    (None, Some(version))
                } else {
                    Err(RequestError::InvalidVersion(input.to_string()))?
                }
            } else if let Some(pos) = version.find('-') {
                (Some(&version[..pos]), Some(&version[pos + 1..]))
            } else if let Some(pos) = version.find(char::is_alphabetic) {
                // Handle both "3.3.0-preview2" and "3.3.0.preview2" formats
                // If preceded by a dot, exclude it from the numbers portion
                let num_end = if pos > 0 && version.as_bytes()[pos - 1] == b'.' {
                    pos - 1
                } else {
                    pos
                };
                (Some(&version[..num_end]), Some(&version[pos..]))
            } else {
                (Some(version), None)
            };

            segments = numbers.map(|rest| rest.split('.'));
            prerelease = pre;
        };

        let Some(mut segments) = segments else {
            return Ok(RubyRequest {
                engine: engine.into(),
                major: None,
                minor: None,
                patch: None,
                tiny: None,
                prerelease: prerelease.map(ToString::to_string),
            });
        };

        let major = segments
            .next()
            .map(|segment| {
                segment
                    .parse::<u32>()
                    .map_err(|_| RequestError::InvalidPart("major version", input.to_string()))
            })
            .transpose()?;
        let minor = segments
            .next()
            .map(|segment| {
                segment
                    .parse::<u32>()
                    .map_err(|_| RequestError::InvalidPart("minor version", input.to_string()))
            })
            .transpose()?;
        let patch = segments
            .next()
            .map(|segment| {
                segment
                    .parse::<u32>()
                    .map_err(|_| RequestError::InvalidPart("patch version", input.to_string()))
            })
            .transpose()?;
        let tiny = segments
            .next()
            .map(|segment| {
                segment
                    .parse::<u32>()
                    .map_err(|_| RequestError::InvalidPart("tiny version", input.to_string()))
            })
            .transpose()?;

        if segments.next().is_some() {
            return Err(RequestError::TooManySegments(input.to_string()));
        }

        Ok(RubyRequest {
            engine: engine.into(),
            major,
            minor,
            patch,
            tiny,
            prerelease: prerelease.map(ToString::to_string),
        })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RubyVersion;

    #[track_caller]
    fn v(version: &str) -> RubyVersion {
        RubyVersion::from_str(version).unwrap()
    }

    #[track_caller]
    fn r(version: &str) -> RubyRequest {
        RubyRequest::from_str(version).unwrap()
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
        let request = r("3.0.0");
        assert_eq!(request.engine, "ruby".into());
        assert_eq!(request.major, Some(3));
        assert_eq!(request.minor, Some(0));
        assert_eq!(request.patch, Some(0));
        assert_eq!(request.tiny, None);
        assert_eq!(request.prerelease, None);
    }

    #[test]
    fn test_adding_ruby_engine_version() {
        let request = v("3.0.0");
        assert_eq!(request.engine, "ruby".into());
        assert_eq!(request.major, (3));
        assert_eq!(request.minor, (0));
        assert_eq!(request.patch, (0));
        assert_eq!(request.tiny, None);
        assert_eq!(request.prerelease, None);
    }

    #[test]
    fn test_major_only() {
        let request = r("3");
        assert_eq!(request.engine, "ruby".into());
        assert_eq!(request.major, Some(3));
        assert_eq!(request.minor, None);
        assert_eq!(request.patch, None);
        assert_eq!(request.tiny, None);
        assert_eq!(request.prerelease, None);
    }

    #[test]
    fn test_major_only_version() {
        let request = RubyVersion::from_str("3");
        let _err = request.unwrap_err();
    }

    #[test]
    fn test_parsing_supported_ruby_versions() {
        let versions = [
            "ruby-0.49",
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
            "ruby-4.0.0-preview2",
            "ruby-dev",
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
            let request = r(version);
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
            let request = r(version);
            let output = request.to_string();
            assert_eq!(
                output, version,
                "Parsed output does not match input for {version}"
            );
        }
    }

    #[test]
    fn test_parsing_ruby_description_versions() {
        // in the RUBY_DESCRIPTION constant, printed for `ruby --version`, the version number for some reason
        // does not include a dash before the prerelease version number.
        let request = v("ruby-4.0.0preview2");
        assert_eq!(request.to_string(), "ruby-4.0.0-preview2");
        let request = v("ruby-3.5.0preview1");
        assert_eq!(request.to_string(), "ruby-3.5.0-preview1");
        let request = v("ruby-3.4.0rc1");
        assert_eq!(request.to_string(), "ruby-3.4.0-rc1");
    }

    #[test]
    fn test_parsing_engine_without_version() {
        let request = r("jruby-");
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
            let request = r(version);
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
            let request = r(version);
            let output = request.to_string();
            assert_eq!(
                output,
                format!("ruby-{}", version.trim()),
                "Parsed output does not match input for {version}"
            );
        }
    }

    #[test]
    fn test_ruby_request_from_str() {
        let version = "3.0.0";
        let request = v(version);
        let output = request.to_string();
        assert_eq!(
            output,
            format!("ruby-{}", version.trim()),
            "Parsed output does not match input for {version}"
        );
    }

    #[test]
    fn test_ruby_request_from_str_with_latest_word() {
        let request = r("latest");
        assert_eq!(request.engine, "ruby".into());
        assert_eq!(request.major, None);
        assert_eq!(request.minor, None);
        assert_eq!(request.patch, None);
        assert_eq!(request.tiny, None);
        assert_eq!(request.prerelease, None);
    }

    #[test]
    fn test_version_comparisons() {
        assert!(v("3.3.9") < v("3.3.10"));
        assert!(v("4.0.0-preview3") < v("4.0.0"));
    }
}
