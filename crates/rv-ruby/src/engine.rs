use std::{
    fmt::{self, Display},
    str::FromStr,
};

use rv_cache::{CacheKey, CacheKeyHasher};
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
        let engine = match s {
            "ruby" => Self::Ruby,
            "jruby" => Self::JRuby,
            "truffleruby" => Self::TruffleRuby,
            "mruby" => Self::MRuby,
            "artichoke" => Self::Artichoke,
            _ => Self::Unknown(s.to_string()),
        };
        Ok(engine)
    }
}

impl From<&str> for RubyEngine {
    fn from(val: &str) -> Self {
        match RubyEngine::from_str(val) {
            Ok(engine) => engine,
            Err(e) => match e {},
        }
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

impl CacheKey for RubyEngine {
    fn cache_key(&self, state: &mut CacheKeyHasher) {
        self.name().cache_key(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_ruby_engine_from_str() {
        assert_eq!(RubyEngine::from_str("ruby").unwrap(), RubyEngine::Ruby);
        assert_eq!(RubyEngine::from_str("jruby").unwrap(), RubyEngine::JRuby);
        assert_eq!(
            RubyEngine::from_str("truffleruby").unwrap(),
            RubyEngine::TruffleRuby
        );
        assert_eq!(RubyEngine::from_str("mruby").unwrap(), RubyEngine::MRuby);
        assert_eq!(
            RubyEngine::from_str("artichoke").unwrap(),
            RubyEngine::Artichoke
        );
        assert_eq!(
            RubyEngine::from_str("custom-ruby").unwrap(),
            RubyEngine::Unknown("custom-ruby".to_string())
        );
    }

    #[test]
    fn test_ruby_engine_name() {
        assert_eq!(RubyEngine::Ruby.name(), "ruby");
        assert_eq!(RubyEngine::JRuby.name(), "jruby");
        assert_eq!(
            RubyEngine::Unknown("custom-ruby".to_string()).name(),
            "custom-ruby"
        );
    }

    #[test]
    fn test_engine_ordering() {
        let ruby = RubyEngine::Ruby;
        let jruby = RubyEngine::JRuby;
        let truffleruby = RubyEngine::TruffleRuby;
        let unknown = RubyEngine::Unknown("custom-ruby".to_string());

        // Ruby comes first
        assert!(ruby < jruby);
        assert!(ruby < truffleruby);
        assert!(ruby < unknown);

        // Known implementations come before unknown
        assert!(jruby < unknown);
        assert!(truffleruby < unknown);

        // Known implementations are sorted alphabetically
        assert!(jruby < truffleruby); // "jruby" < "truffleruby"
    }
}
