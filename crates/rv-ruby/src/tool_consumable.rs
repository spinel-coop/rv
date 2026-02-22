use crate::{
    request::{ReleasedRubyRequest, RubyRequest},
    version::{ReleasedRubyVersion, RubyVersion},
};
use std::fmt::Display;

pub trait ToolConsumable: Display {
    fn to_tool_consumable_string(&self) -> String {
        self.to_string().replace("ruby-", "")
    }
}

impl ToolConsumable for RubyRequest {}
impl ToolConsumable for ReleasedRubyRequest {}
impl ToolConsumable for RubyVersion {}
impl ToolConsumable for ReleasedRubyVersion {}
