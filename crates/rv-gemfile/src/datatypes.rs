//! Most of the types in this module borrow a string from their input,
//! so they have a lifetime 'i, which is short for 'input.

#[derive(Debug, Clone, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Gemfile<'i> {
    foo: &'i str,
}
