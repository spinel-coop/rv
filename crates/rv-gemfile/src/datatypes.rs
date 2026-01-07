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
    Gem(Gem<'i>),
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Gem<'i> {
    #[serde(borrow)]
    pub name: &'i str,
    pub constraint: SemverConstraint,
    #[serde(borrow)]
    pub version: &'i str,
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
