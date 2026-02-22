use std::str::FromStr;

use crate::{
    engine::RubyEngine,
    request::{ReleasedRubyRequest, RequestError, RubyRequest, VersionPart},
    tool_consumable::ToolConsumable,
};
use serde_with::{DeserializeFromStr, SerializeDisplay};

/// A specific version of Ruby, which can be run and downloaded.
/// This is different from a RubyRequest, which represents a range of possible
/// Ruby versions.
#[derive(Debug, Clone, PartialEq, Eq, DeserializeFromStr, SerializeDisplay)]
pub enum RubyVersion {
    /// The daily dev builds from rv-ruby-dev
    Dev,
    /// A proper released version like 4.0.1
    Released(ReleasedRubyVersion),
}

impl RubyVersion {
    pub fn is_dev(&self) -> bool {
        matches!(self, Self::Dev)
    }

    pub fn number(&self) -> String {
        match self {
            RubyVersion::Dev => "dev".to_string(),
            RubyVersion::Released(v) => v.number(),
        }
    }

    pub fn satisfies(&self, request: &RubyRequest) -> bool {
        match (self, request) {
            (RubyVersion::Dev, RubyRequest::Dev) => true,
            (RubyVersion::Dev, RubyRequest::Released(_)) => false,
            (RubyVersion::Released(version), request) => version.satisfies(request),
        }
    }

    pub fn to_tool_consumable_version(&self) -> String {
        match self {
            RubyVersion::Dev => "dev".to_string(),
            RubyVersion::Released(v) => v.to_tool_consumable_string(),
        }
    }
}

impl FromStr for RubyVersion {
    type Err = ParseVersionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "dev" || s == "ruby-dev" {
            return Ok(Self::Dev);
        }
        ReleasedRubyVersion::from_str(s).map(Self::Released)
    }
}

impl std::fmt::Display for RubyVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RubyVersion::Dev => write!(f, "ruby-dev"),
            RubyVersion::Released(version) => version.fmt(f),
        }
    }
}

impl TryFrom<RubyRequest> for RubyVersion {
    type Error = ParseVersionError;

    fn try_from(request: RubyRequest) -> Result<Self, Self::Error> {
        match request {
            RubyRequest::Dev => Ok(Self::Dev),
            RubyRequest::Released(released) => {
                ReleasedRubyVersion::try_from(released).map(Self::Released)
            }
        }
    }
}

/// A concrete, released version of Ruby that can be downloaded.
#[derive(Debug, Clone, PartialEq, Eq, DeserializeFromStr, SerializeDisplay)]
pub struct ReleasedRubyVersion {
    pub engine: RubyEngine,
    pub major: VersionPart,
    pub minor: VersionPart,
    pub patch: VersionPart,
    pub tiny: Option<VersionPart>,
    pub prerelease: Option<String>,
}

impl Ord for ReleasedRubyVersion {
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

impl PartialOrd for ReleasedRubyVersion {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Ways a Ruby version can fail to be parsed.
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
    #[error("Cannot use the dev version of Ruby here")]
    CannotUseDev,
}

impl FromStr for ReleasedRubyVersion {
    type Err = ParseVersionError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        RubyRequest::from_str(input)?.try_into()
    }
}

/// If the Ruby request is very specific, it can be made into a specific Ruby version.
impl TryFrom<RubyRequest> for ReleasedRubyVersion {
    type Error = ParseVersionError;

    fn try_from(request: RubyRequest) -> Result<Self, Self::Error> {
        match request {
            RubyRequest::Dev => Err(ParseVersionError::CannotUseDev),
            RubyRequest::Released(request) => Self::try_from(request),
        }
    }
}

/// If the Ruby request is very specific, it can be made into a specific Ruby version.
impl TryFrom<ReleasedRubyRequest> for ReleasedRubyVersion {
    type Error = ParseVersionError;

    fn try_from(request: ReleasedRubyRequest) -> Result<Self, Self::Error> {
        let major = request.major.ok_or(ParseVersionError::MissingMajor)?;
        let minor = request.minor.ok_or(ParseVersionError::MissingMinor)?;
        let patch = request.patch.ok_or(ParseVersionError::MissingPatch)?;

        Ok(Self {
            engine: request.engine,
            major,
            minor,
            patch,
            tiny: request.tiny,
            prerelease: request.prerelease,
        })
    }
}

impl From<ReleasedRubyVersion> for RubyRequest {
    fn from(version: ReleasedRubyVersion) -> Self {
        Self::Released(ReleasedRubyRequest {
            engine: version.engine,
            major: Some(version.major),
            minor: Some(version.minor),
            patch: Some(version.patch),
            tiny: version.tiny,
            prerelease: version.prerelease,
        })
    }
}

impl ReleasedRubyVersion {
    /// Does this version satisfy the given Ruby requested range?
    pub fn satisfies(&self, request: &RubyRequest) -> bool {
        let request = match request {
            RubyRequest::Dev => return false,
            RubyRequest::Released(request) => request,
        };
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

    /// ABI compatibility version for this ruby version.
    pub fn abi(&self) -> String {
        format!("{}.{}.0", self.major, self.minor)
    }

    /// Is this ruby version a prerelease.
    pub fn is_prerelease(&self) -> bool {
        self.prerelease.is_some()
    }

    /// Parse a Ruby version from Gemfile.lock format.
    ///
    /// Gemfile.lock uses the format "ruby X.Y.ZpNN" where:
    /// - "ruby " is a prefix (with a space, not a dash)
    /// - "pNN" is an optional patchlevel suffix that should be ignored
    ///
    /// Examples:
    /// - "ruby 3.3.1p55" -> RubyVersion for 3.3.1
    /// - "ruby 4.0.0" -> RubyVersion for 4.0.0
    pub fn from_gemfile_lock(input: &str) -> Result<Self, ParseVersionError> {
        // Strip "ruby " prefix
        let version = input.strip_prefix("ruby ").unwrap_or(input);

        // Strip patchlevel suffix (e.g., "p55" from "3.3.1p55")
        // Only strip if 'p' is followed by all digits
        let version = if let Some(p_pos) = version.find('p') {
            if version[p_pos + 1..].chars().all(|c| c.is_ascii_digit()) {
                &version[..p_pos]
            } else {
                version
            }
        } else {
            version
        };

        version.parse()
    }
}

impl std::fmt::Display for ReleasedRubyVersion {
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_parsing_supported_ruby_versions() {
        use std::str::FromStr as _;

        let versions = [
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
            "ruby-3.4.0",
            "ruby-3.4.0-preview1",
            "ruby-3.4.0-preview2",
            "ruby-3.4.0-rc1",
            "ruby-3.4.1",
            "ruby-3.4.2",
            "ruby-3.4.3",
            "ruby-3.4.4",
            "ruby-3.4.5",
            "ruby-3.5.0-preview1",
            "ruby-4.0.0",
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
            "mruby-3.2.0",
            "mruby-3.3.0",
            "mruby-3.4.0",
            "picoruby-3.0.0",
            "truffleruby-24.1.0",
            "truffleruby-24.1.1",
            "truffleruby-24.1.2",
            "truffleruby-24.2.0",
            "truffleruby-24.2.1",
            "truffleruby+graalvm-24.1.0",
            "truffleruby+graalvm-24.1.1",
            "truffleruby+graalvm-24.1.2",
            "truffleruby+graalvm-24.2.0",
            "truffleruby+graalvm-24.2.1",
        ];

        for version_str in versions {
            // Invariant: all these strings should be valid Ruby versions.
            let version = ReleasedRubyVersion::from_str(version_str)
                .unwrap_or_else(|_| panic!("Failed to parse version in {version_str}"));
            let output = version.to_string();
            assert_eq!(
                output, version_str,
                "Parsed output does not match input for {version_str}"
            );
            // Invariant: all Ruby versions should be convertible into RubyRequest
            // and back to RubyVersion, unchanged.
            let request = RubyRequest::from(version.clone());
            let version_out = ReleasedRubyVersion::try_from(request).unwrap();
            assert_eq!(
                version_out, version,
                "Version did not survive the roundtrip to/from RubyRequest"
            );
            // Invariant: the number should appear in the version.
            let num = version.number();
            assert!(version_str.contains(&num));
        }
    }

    #[test]
    fn test_from_gemfile_lock_with_patchlevel() {
        // Gemfile.lock format: "ruby 3.3.1p55"
        let version = ReleasedRubyVersion::from_gemfile_lock("ruby 3.3.1p55").unwrap();
        assert_eq!(version.major, 3);
        assert_eq!(version.minor, 3);
        assert_eq!(version.patch, 1);
        assert_eq!(version.prerelease, None);
    }

    #[test]
    fn test_from_gemfile_lock_without_patchlevel() {
        // Gemfile.lock format: "ruby 4.0.0" (no patchlevel)
        let version = ReleasedRubyVersion::from_gemfile_lock("ruby 4.0.0").unwrap();
        assert_eq!(version.major, 4);
        assert_eq!(version.minor, 0);
        assert_eq!(version.patch, 0);
        assert_eq!(version.prerelease, None);
    }

    #[test]
    fn test_from_gemfile_lock_with_p0() {
        // Gemfile.lock format: "ruby 3.2.0p0"
        let version = ReleasedRubyVersion::from_gemfile_lock("ruby 3.2.0p0").unwrap();
        assert_eq!(version.major, 3);
        assert_eq!(version.minor, 2);
        assert_eq!(version.patch, 0);
    }

    #[test]
    fn test_from_gemfile_lock_preserves_preview() {
        // Real format from GitHub: "ruby 3.3.0.preview2" (dot, not dash)
        // https://github.com/akitaonrails/rinhabackend-rails-api/blob/main/Gemfile.lock
        let version = ReleasedRubyVersion::from_gemfile_lock("ruby 3.3.0.preview2").unwrap();
        assert_eq!(version.major, 3);
        assert_eq!(version.minor, 3);
        assert_eq!(version.patch, 0);
        assert_eq!(version.prerelease, Some("preview2".to_string()));
    }

    #[test]
    fn test_from_gemfile_lock_preserves_rc() {
        // Real format from GitHub: "ruby 3.3.0.rc1" (dot, not dash)
        // https://github.com/pbstriker38/is_ruby_dead/blob/main/Gemfile.lock
        let version = ReleasedRubyVersion::from_gemfile_lock("ruby 3.3.0.rc1").unwrap();
        assert_eq!(version.major, 3);
        assert_eq!(version.minor, 3);
        assert_eq!(version.patch, 0);
        assert_eq!(version.prerelease, Some("rc1".to_string()));
    }
}
