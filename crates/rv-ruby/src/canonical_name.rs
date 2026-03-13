use crate::{
    request::{ReleasedRubyRequest, RubyRequest},
    version::RubyVersion,
};
use std::fmt::Display;

pub trait CanonicalName: Display {
    fn canonical_name(&self) -> String {
        self.to_string().replace("ruby-", "")
    }
}

impl CanonicalName for RubyRequest {}
impl CanonicalName for ReleasedRubyRequest {}
impl CanonicalName for RubyVersion {}
