use std::{
    fmt::{self, Display},
    str::FromStr,
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RubyEngine {
    /// Standard Ruby (MRI/CRuby)
    Ruby,
    /// JRuby (Java implementation)
    #[serde(rename = "jruby")]
    JRuby,
    /// TruffleRuby (GraalVM implementation)
    #[serde(rename = "truffleruby")]
    TruffleRuby,
    /// mruby (minimal Ruby)
    #[serde(rename = "mruby")]
    MRuby,
    /// Artichoke Ruby (Rust implementation)
    #[serde(rename = "artichoke")]
    Artichoke,
    /// Unknown implementation with the original name
    #[serde(untagged)]
    Unknown(String),
}

impl RubyEngine {
    /// Get the display name for this implementation
    pub fn name(&self) -> &str {
        match self {
            Self::Ruby => "ruby",
            Self::JRuby => "jruby",
            Self::TruffleRuby => "truffleruby",
            Self::MRuby => "mruby",
            Self::Artichoke => "artichoke",
            Self::Unknown(name) => name,
        }
    }
}

impl Display for RubyEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl FromStr for RubyEngine {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let implementation = match s {
            "ruby" => Self::Ruby,
            "jruby" => Self::JRuby,
            "truffleruby" => Self::TruffleRuby,
            "mruby" => Self::MRuby,
            "artichoke" => Self::Artichoke,
            _ => Self::Unknown(s.to_string()),
        };
        Ok(implementation)
    }
}

impl PartialOrd for RubyEngine {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RubyEngine {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;

        // Get priority for each implementation
        let self_priority = match self {
            Self::Ruby => 0, // Ruby always comes first
            Self::JRuby | Self::TruffleRuby | Self::MRuby | Self::Artichoke => 1, // Known implementations second
            Self::Unknown(_) => 2, // Unknown implementations last
        };

        let other_priority = match other {
            Self::Ruby => 0,
            Self::JRuby | Self::TruffleRuby | Self::MRuby | Self::Artichoke => 1,
            Self::Unknown(_) => 2,
        };

        // First compare by priority
        match self_priority.cmp(&other_priority) {
            Ordering::Equal => {
                // Same priority, sort alphabetically by name
                self.name().cmp(other.name())
            }
            other => other,
        }
    }
}
