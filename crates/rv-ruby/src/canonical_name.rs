use crate::{
    request::{ReleasedRubyRequest, RubyRequest},
    version::RubyVersion,
};
use std::fmt::Display;

pub trait CanonicalName: Display {
    /// The name users see, with the redundant `ruby-` prefix dropped for CRuby.
    ///
    /// Only CRuby is stripped: other engines carry their name as part of their
    /// identity, so `jruby-10.1.1.0` stays intact rather than becoming `j10.1.1.0`.
    fn canonical_name(&self) -> String {
        let name = self.to_string();
        match name.strip_prefix("ruby-") {
            Some(stripped) => stripped.to_string(),
            None => name,
        }
    }
}

impl CanonicalName for RubyRequest {}
impl CanonicalName for ReleasedRubyRequest {}
impl CanonicalName for RubyVersion {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[track_caller]
    fn name(version: &str) -> String {
        RubyVersion::from_str(version).unwrap().canonical_name()
    }

    #[test]
    fn strips_the_cruby_prefix() {
        assert_eq!(name("ruby-3.4.1"), "3.4.1");
        assert_eq!(name("ruby-3.5.0-preview1"), "3.5.0-preview1");
    }

    #[test]
    fn keeps_other_engine_names_intact() {
        assert_eq!(name("jruby-10.1.1.0"), "jruby-10.1.1.0");
        assert_eq!(name("jruby-9.4.15.0"), "jruby-9.4.15.0");
        assert_eq!(name("truffleruby-24.1.1"), "truffleruby-24.1.1");
    }
}
