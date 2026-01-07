//! Most of the types in this module borrow a string from their input,
//! so they have a lifetime 'i, which is short for 'input.

#[derive(Debug, Clone, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Gemfile<'i> {
    #[serde(borrow)]
    pub items: Vec<Item<'i>>,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Item<'i> {
    Source(&'i str),
    RubyFile(&'i str),
    Gem(GemRange<'i>),
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

/// A range of possible versions of a certain gem.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GemRange<'i> {
    pub name: &'i str,
    pub semver: Vec<GemRangeSemver<'i>>,
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
