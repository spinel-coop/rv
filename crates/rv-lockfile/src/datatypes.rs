//! Most of the types in this module borrow a string from their input,
//! so they have a lifetime 'i, which is short for 'input.

#[derive(Debug, Clone, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GemfileDotLock<'i> {
    /// Dependencies sourced from a Git repo.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub git: Vec<GitSection<'i>>,

    /// Dependencies sourced from a RubyGems server.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gem: Vec<GemSection<'i>>,

    /// Dependencies sourced from a filesystem path.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub path: Vec<PathSection<'i>>,

    /// Lists every triple that Bundler has resolved and included in this lockfile.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub platforms: Vec<&'i str>,

    /// Lists every gem that this lockfile has been resolved to include
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<GemRange<'i>>,

    /// Which version of Ruby this lockfile was built with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ruby_version: Option<&'i str>,

    /// Which version of Bundler this lockfile was built with.
    pub bundled_with: Option<&'i str>,

    /// Checksums for each dependency.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checksums: Option<Vec<Checksum<'i>>>,
}

/// Git source that gems could come from.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GitSection<'i> {
    /// Location of the Git repo.
    pub remote: &'i str,
    /// Commit used from the Git repo.
    pub revision: &'i str,
    /// Branch used from the Git repo.
    pub branch: Option<&'i str>,
    /// All gems which came from this source in particular.
    pub specs: Vec<Spec<'i>>,
}

/// Rubygems server source that gems could come from.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GemSection<'i> {
    /// Location of the RubyGems server.
    pub remote: &'i str,
    /// All gems which came from this source in particular.
    pub specs: Vec<Spec<'i>>,
}

/// Filesystem path that gems could come from.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PathSection<'i> {
    /// The filesystem path that sourced these dependencies.
    pub remote: &'i str,
    /// All gems which came from this source in particular.
    pub specs: Vec<Spec<'i>>,
}

/// A (gem, version) pair.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GemVersion<'i> {
    /// Name of the gem.
    pub name: &'i str,
    /// Version of the gem.
    pub version: &'i str,
}

/// A range of possible versions of a certain gem.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GemRange<'i> {
    pub name: &'i str,
    pub semver: Option<Vec<GemRangeSemver<'i>>>,
    /// Dependencies specified with a source other than the main Rubygems index (e.g., git dependencies, path-based, dependencies) have a ! which means they are "pinned" to that source.
    /// According to <https://stackoverflow.com/questions/7517524/understanding-the-gemfile-lock-file>.
    pub nonstandard: bool,
}

/// A range of possible versions of a gem.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GemRangeSemver<'i> {
    pub semver_constraint: SemverConstraint,
    #[serde(borrow)]
    pub version: &'i str,
}

/// Gem which has been locked and came from some particular source.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Spec<'i> {
    #[serde(borrow)]
    pub gem_version: GemVersion<'i>,
    #[serde(borrow)]
    pub deps: Vec<GemRange<'i>>,
}

/// Checksum of a particular gem version.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Checksum<'i> {
    #[serde(borrow)]
    pub gem_version: GemVersion<'i>,
    pub algorithm: ChecksumAlgorithm<'i>,
    pub value: Vec<u8>,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ChecksumAlgorithm<'i> {
    #[serde(borrow)]
    Unknown(&'i str),
    #[default]
    SHA256,
}

/// Constrains the range of possible versions of a gem which could be selected.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SemverConstraint {
    /// `=`
    Exact,
    /// `!=`
    NotEqual,
    /// `>`
    GreaterThan,
    /// `<`
    LessThan,
    /// `>=`
    GreaterThanOrEqual,
    /// `<=`
    LessThanOrEqual,
    /// `~>`
    Pessimistic,
}
