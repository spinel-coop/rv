use std::str::FromStr;

use crate::{
    engine::RubyEngine,
    request::{RequestError, RubyRequest, VersionPart},
};
use serde_with::{DeserializeFromStr, SerializeDisplay};

/// A specific version of Ruby, which can be run and downloaded.
/// This is different from a RubyRequest, which represents a range of possible
/// Ruby versions.
#[derive(Debug, Clone, PartialEq, Eq, DeserializeFromStr, SerializeDisplay, Ord, PartialOrd)]
pub struct RubyVersion {
    pub engine: RubyEngine,
    pub major: VersionPart,
    pub minor: VersionPart,
    pub patch: VersionPart,
    pub tiny: Option<VersionPart>,
    pub prerelease: Option<String>,
}

/// If the Ruby request is very specific, it can be made into a specific Ruby version.
impl TryFrom<RubyRequest> for RubyVersion {
    type Error = String;

    fn try_from(request: RubyRequest) -> Result<Self, Self::Error> {
        if let Some(major) = request.major
            && let Some(minor) = request.minor
            && let Some(patch) = request.patch
        {
            Ok(RubyVersion {
                major,
                minor,
                patch,
                engine: request.engine,
                tiny: request.tiny,
                prerelease: request.prerelease,
            })
        } else {
            Err(format!(
                "The range {request} was not specific enough to pick a specific Ruby version"
            ))
        }
    }
}

impl RubyVersion {
    /// Does this version satisfy the given Ruby requested range?
    pub fn satisfies(&self, request: &RubyRequest) -> bool {
        if self.engine != request.engine {
            return false;
        }
        if let Some(major) = request.major
            && self.major != major
        {
            return false;
        }
        if let Some(minor) = request.minor
            && self.minor != minor
        {
            return false;
        }
        if let Some(patch) = request.patch
            && self.patch != patch
        {
            return false;
        }
        if request.tiny.is_some() && self.tiny != request.tiny {
            return false;
        }
        if self.prerelease != request.prerelease {
            return false;
        }

        true
    }

    /// Get the Ruby number. Basically like calling `.to_string()` except without the Ruby engine.
    pub fn number(&self) -> String {
        use std::fmt::Write;
        let mut version = format!("{}.{}.{}", self.major, self.minor, self.patch);

        if let Some(tiny) = self.tiny {
            version.push('.');
            write!(&mut version, "{}", tiny).unwrap();
        }
        if let Some(ref prerelease) = self.prerelease {
            version.push('-');
            version.push_str(prerelease);
        }
        version
    }
}

impl std::fmt::Display for RubyVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.engine)?;
        write!(f, "-{}", self.major)?;
        write!(f, ".{}", self.minor)?;
        write!(f, ".{}", self.patch)?;

        if let Some(tiny) = self.tiny {
            write!(f, ".{tiny}")?;
        }

        if let Some(ref pre_release) = self.prerelease {
            write!(f, "-{pre_release}")?;
        };

        Ok(())
    }
}

/// Ways that a ruby version could fail to be parsed.
#[derive(thiserror::Error, Debug)]
pub enum ParseVersionError {
    #[error(transparent)]
    Invalid(#[from] RequestError),
    #[error("Missing major version")]
    MissingMajor,
    #[error("Missing minor version")]
    MissingMinor,
    #[error("Missing patch version")]
    MissingPatch,
}

impl FromStr for RubyVersion {
    type Err = ParseVersionError;
    fn from_str(input: &str) -> Result<Self, ParseVersionError> {
        let req = RubyRequest::from_str(input)?;
        let major = req.major.ok_or(ParseVersionError::MissingMajor)?;
        let minor = req.minor.ok_or(ParseVersionError::MissingMinor)?;
        let patch = req.patch.ok_or(ParseVersionError::MissingPatch)?;
        Ok(Self {
            engine: req.engine,
            major,
            minor,
            patch,
            tiny: req.tiny,
            prerelease: req.prerelease,
        })
    }
}

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
