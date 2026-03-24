use crate::{Platform, Version, VersionPlatform};
use pubgrub::Ranges;
use rv_ruby::Versioned;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RequirementError {
    #[error("Empty requirement string")]
    Empty,
    #[error("Invalid requirement operator: {operator}")]
    InvalidOperator { operator: String },
    #[error("Invalid version in requirement: {version}")]
    InvalidVersion { version: String },
    #[error("Malformed requirement string: {requirement}")]
    Malformed { requirement: String },
}

#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct Requirement {
    pub constraints: Vec<VersionConstraint>,
}

impl Default for Requirement {
    fn default() -> Self {
        Self {
            constraints: vec![VersionConstraint::default()],
        }
    }
}

impl std::fmt::Debug for Requirement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.constraints.fmt(f)
    }
}

impl From<Vec<VersionConstraint>> for Requirement {
    fn from(constraints: Vec<VersionConstraint>) -> Self {
        Self { constraints }
    }
}

impl From<Requirement> for Vec<VersionConstraint> {
    fn from(constraints: Requirement) -> Self {
        constraints.constraints
    }
}

impl From<Requirement> for Ranges<VersionPlatform> {
    fn from(constraints: Requirement) -> Self {
        // Convert the RubyGems constraints into PubGrub ranges.
        let ranges = constraints.constraints.into_iter().map(Ranges::from);

        // Now, join all those ranges together using &, because that's what multiple RubyGems
        // constraints are actually listed as.
        let mut overall_range = Ranges::full();
        for r in ranges {
            overall_range = overall_range.intersection(&r);
        }
        overall_range
    }
}

// Defaults to ">= 0"
#[derive(Default, Clone, Ord, PartialOrd, Serialize, Deserialize)]
pub struct VersionConstraint {
    pub operator: ComparisonOperator,
    pub version: Version,
}

impl std::fmt::Debug for VersionConstraint {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{} {}", self.operator, self.version)
    }
}

impl VersionConstraint {
    pub fn version_from(str: &str, prefix: &str) -> Result<Version, RequirementError> {
        let version = str.strip_prefix(prefix).unwrap_or(str).trim();

        Version::new(version).map_err(|_| RequirementError::InvalidVersion {
            version: version.to_string(),
        })
    }

    pub fn is_latest(&self) -> bool {
        matches!(self.operator, ComparisonOperator::GreaterThanOrEqual)
            && self.version.version == "0"
    }
}

impl TryFrom<&str> for VersionConstraint {
    type Error = RequirementError;

    fn try_from(str: &str) -> Result<Self, RequirementError> {
        let str = str.trim();

        if str.is_empty() {
            return Err(RequirementError::Empty);
        }

        // Try to match operator and version
        let operator = ComparisonOperator::try_from(str)?;
        let version = VersionConstraint::version_from(str, operator.as_ref())?;

        Ok(Self { operator, version })
    }
}

impl From<VersionConstraint> for Ranges<VersionPlatform> {
    fn from(constraint: VersionConstraint) -> Self {
        let v = constraint.version;
        let min_v = VersionPlatform {
            version: v.clone(),
            platform: Platform::Ruby,
        };

        let max_v = VersionPlatform {
            version: v.clone(),
            platform: Platform::Current,
        };

        match constraint.operator {
            ComparisonOperator::Equal => {
                Ranges::intersection(&Ranges::higher_than(min_v), &Ranges::lower_than(max_v))
            }
            ComparisonOperator::NotEqual => Ranges::union(
                &Ranges::strictly_lower_than(min_v),
                &Ranges::strictly_higher_than(max_v),
            ),

            // These 4 are easy:
            ComparisonOperator::GreaterThan => Ranges::strictly_higher_than(max_v),
            ComparisonOperator::LessThan => Ranges::strictly_lower_than(min_v),
            ComparisonOperator::GreaterThanOrEqual => Ranges::higher_than(min_v),
            ComparisonOperator::LessThanOrEqual => Ranges::lower_than(max_v),
            // This one is weird, but at least it's encapsulated into a `bump` method.
            ComparisonOperator::Pessimistic => {
                let bump_v = VersionPlatform {
                    version: Version::new(format!("{}.A", v.bump())).unwrap(),
                    platform: Platform::Ruby,
                };

                Ranges::intersection(
                    &Ranges::higher_than(min_v),
                    &Ranges::strictly_lower_than(bump_v),
                )
            }
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum ComparisonOperator {
    Equal,
    NotEqual,
    GreaterThan,
    #[default]
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
    Pessimistic,
}

impl TryFrom<&str> for ComparisonOperator {
    type Error = RequirementError;

    fn try_from(str: &str) -> Result<Self, RequirementError> {
        match str {
            s if s.starts_with(">=") => Ok(Self::GreaterThanOrEqual),
            s if s.starts_with("<=") => Ok(Self::LessThanOrEqual),
            s if s.starts_with("!=") => Ok(Self::NotEqual),
            s if s.starts_with("~>") => Ok(Self::Pessimistic),
            s if s.starts_with(">") => Ok(Self::GreaterThan),
            s if s.starts_with("<") => Ok(Self::LessThan),
            s if s.starts_with("!") => Err(RequirementError::InvalidOperator {
                operator: str.chars().take(2).collect(),
            }),
            _ => Ok(Self::Equal), // Default to "=" if no operator specified
        }
    }
}

impl FromStr for ComparisonOperator {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "!=" => Ok(ComparisonOperator::NotEqual),
            ">=" => Ok(ComparisonOperator::GreaterThanOrEqual),
            "<=" => Ok(ComparisonOperator::LessThanOrEqual),
            ">" => Ok(ComparisonOperator::GreaterThan),
            "<" => Ok(ComparisonOperator::LessThan),
            "~>" => Ok(ComparisonOperator::Pessimistic),
            "=" => Ok(ComparisonOperator::Equal),
            other => Err(other.to_owned()),
        }
    }
}

impl AsRef<str> for ComparisonOperator {
    fn as_ref(&self) -> &str {
        match self {
            Self::GreaterThanOrEqual => ">=",
            Self::LessThanOrEqual => "<=",
            Self::NotEqual => "!=",
            Self::Pessimistic => "~>",
            Self::GreaterThan => ">",
            Self::LessThan => "<",
            Self::Equal => "=",
        }
    }
}

impl Requirement {
    pub fn new(requirements: Vec<impl AsRef<str>>) -> Result<Self, RequirementError> {
        if requirements.is_empty() {
            Ok(Self::default())
        } else {
            let mut constraints = Vec::new();

            for req in requirements {
                constraints.push(VersionConstraint::try_from(req.as_ref())?);
            }

            Ok(Self { constraints })
        }
    }

    pub fn parse(requirement: &str) -> Result<Self, RequirementError> {
        Self::new(vec![requirement])
    }

    /// Resolve the Ruby request to a specific version of ruby, chosen from
    /// the given list.
    pub fn find_match_in<T: Versioned + Clone>(
        &self,
        rubies: &[T],
        allow_prerelease: bool,
    ) -> Option<T> {
        rubies
            .iter()
            .rev()
            .find(|r| self.matches(&Version::from(r.version()), allow_prerelease))
            .cloned()
    }

    pub fn satisfied_by(&self, version: &Version) -> bool {
        self.constraints
            .iter()
            .all(|constraint| constraint.matches(version))
    }

    pub fn matches(&self, version: &Version, allow_prerelease: bool) -> bool {
        // Check prerelease logic
        if version.is_prerelease() && !allow_prerelease && !self.is_prerelease() {
            return false;
        }

        self.satisfied_by(version)
    }

    pub fn is_latest_version(&self) -> bool {
        self.as_sole_constraint()
            .is_some_and(|constraint| constraint.is_latest())
    }

    pub fn is_prerelease(&self) -> bool {
        // A requirement is prerelease if any of its constraint versions are prerelease
        self.constraints
            .iter()
            .any(|constraint| constraint.version.is_prerelease())
    }

    pub fn to_ruby(&self) -> String {
        match self.as_sole_constraint() {
            Some(constraint) => format!("Gem::Requirement.new(\"{}\".freeze)", constraint),
            None => {
                let constraints = self
                    .constraints
                    .iter()
                    .map(|c| format!("\"{}\".freeze", c))
                    .collect::<Vec<_>>()
                    .join(", ");

                format!("Gem::Requirement.new([{}])", constraints)
            }
        }
    }

    /// If this has exactly 1 constraint, return it.
    fn as_sole_constraint(&self) -> Option<&VersionConstraint> {
        (self.constraints.len() == 1).then(|| self.constraints.first())?
    }
}

impl PartialEq for VersionConstraint {
    fn eq(&self, other: &Self) -> bool {
        self.operator == other.operator && self.version.version == other.version.version
    }
}

impl Eq for VersionConstraint {}

impl Hash for VersionConstraint {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.operator.hash(state);
        self.version.version.hash(state);
    }
}

impl VersionConstraint {
    pub fn new(operator: ComparisonOperator, version: Version) -> Self {
        Self { operator, version }
    }

    pub fn matches(&self, version: &Version) -> bool {
        match self.operator {
            ComparisonOperator::Equal => version == &self.version,
            ComparisonOperator::NotEqual => version != &self.version,
            ComparisonOperator::GreaterThan => version > &self.version,
            ComparisonOperator::GreaterThanOrEqual => version >= &self.version,
            ComparisonOperator::LessThan => version < &self.version,
            ComparisonOperator::LessThanOrEqual => version <= &self.version,
            ComparisonOperator::Pessimistic => {
                version >= &self.version && version < &self.version.bump()
            }
        }
    }
}

impl std::fmt::Display for ComparisonOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

impl std::fmt::Display for VersionConstraint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.operator, self.version)
    }
}

impl std::fmt::Display for Requirement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let constraints: Vec<String> = self.constraints.iter().map(|c| c.to_string()).collect();
        write!(f, "{}", constraints.join(", "))
    }
}

impl FromStr for Requirement {
    type Err = RequirementError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Requirement::parse(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rv_ruby::version::RubyVersion;

    #[track_caller]
    fn v(version: &str) -> Version {
        Version::new(version).unwrap()
    }

    #[track_caller]
    fn req(requirement: &str) -> Requirement {
        Requirement::parse(requirement).unwrap()
    }

    #[test]
    fn test_requirement_parsing() {
        // Basic parsing
        insta::assert_debug_snapshot!(req("1.0"), @r"
        [
            = 1.0,
        ]
        ");

        insta::assert_debug_snapshot!(req("= 1.0"), @r"
        [
            = 1.0,
        ]
        ");

        insta::assert_debug_snapshot!(req("> 1.0"), @r"
        [
            > 1.0,
        ]
        ");

        insta::assert_debug_snapshot!(req("~> 1.2"), @r"
        [
            ~> 1.2,
        ]
        ");
    }

    #[test]
    fn test_requirement_matching() {
        // Basic matching
        assert!(req("1.0").satisfied_by(&v("1.0")));
        assert!(req("= 1.0").satisfied_by(&v("1.0")));
        assert!(!req("= 1.0").satisfied_by(&v("1.1")));

        // Greater than
        assert!(req("> 1.0").satisfied_by(&v("1.1")));
        assert!(!req("> 1.0").satisfied_by(&v("1.0")));

        // Greater than or equal
        assert!(req(">= 1.0").satisfied_by(&v("1.0")));
        assert!(req(">= 1.0").satisfied_by(&v("1.1")));
        assert!(!req(">= 1.0").satisfied_by(&v("0.9")));

        // Less than
        assert!(req("< 1.0").satisfied_by(&v("0.9")));
        assert!(!req("< 1.0").satisfied_by(&v("1.0")));

        // Less than or equal
        assert!(req("<= 1.0").satisfied_by(&v("1.0")));
        assert!(req("<= 1.0").satisfied_by(&v("0.9")));
        assert!(!req("<= 1.0").satisfied_by(&v("1.1")));

        // Not equal
        assert!(req("!= 1.0").satisfied_by(&v("1.1")));
        assert!(!req("!= 1.0").satisfied_by(&v("1.0")));
    }

    #[test]
    fn test_pessimistic_operator() {
        // ~> 1.4 matches 1.4, 1.5, 1.9 but not 2.0
        assert!(req("~> 1.4").satisfied_by(&v("1.4")));
        assert!(req("~> 1.4").satisfied_by(&v("1.5")));
        assert!(req("~> 1.4").satisfied_by(&v("1.9")));
        assert!(!req("~> 1.4").satisfied_by(&v("2.0")));
        assert!(!req("~> 1.4").satisfied_by(&v("1.3")));

        // ~> 1.4.4 matches 1.4.4, 1.4.5 but not 1.5.0
        assert!(req("~> 1.4.4").satisfied_by(&v("1.4.4")));
        assert!(req("~> 1.4.4").satisfied_by(&v("1.4.5")));
        assert!(!req("~> 1.4.4").satisfied_by(&v("1.5.0")));
        assert!(!req("~> 1.4.4").satisfied_by(&v("1.4.3")));

        assert_ne!(req("~> 1.0.0"), req("~> 1.0"));
        assert!(req("~> 1.0.0").satisfied_by(&v("1.0.1")));
        assert!(req("~> 1.0.0").satisfied_by(&v("1")));
        assert!(!req("~> 1.0.0").satisfied_by(&v("1.1")));
    }

    #[test]
    fn test_multiple_constraints() {
        let req = Requirement::new(vec![">= 1.4", "<= 1.6", "!= 1.5"]).unwrap();

        assert!(req.satisfied_by(&v("1.4")));
        assert!(req.satisfied_by(&v("1.6")));
        assert!(!req.satisfied_by(&v("1.3")));
        assert!(!req.satisfied_by(&v("1.5")));
        assert!(!req.satisfied_by(&v("1.7")));
    }

    #[test]
    fn test_default_requirement() {
        let req = Requirement::new(vec![""; 0]).unwrap();
        assert_eq!(req.constraints.len(), 1);
        assert_eq!(
            req.constraints[0].operator,
            ComparisonOperator::GreaterThanOrEqual
        );
        assert_eq!(req.constraints[0].version, v("0"));
    }

    #[test]
    fn test_prerelease_versions() {
        assert!(req(">= 1.0.0").satisfied_by(&v("1.0.0")));
        assert!(req(">= 1.0.0").satisfied_by(&v("1.0.1")));
        assert!(!req(">= 1.0.0").satisfied_by(&v("1.0.0.a")));
        assert!(req(">= 1.0.0.a").satisfied_by(&v("1.0.0.a")));
        assert!(req(">= 1.0.0.a").satisfied_by(&v("1.0.0")));
    }

    #[test]
    fn test_invalid_requirements() {
        assert!(Requirement::parse("").is_err());
        assert!(Requirement::parse("! 1").is_err());
        assert!(Requirement::parse("= junk").is_err());
        assert!(Requirement::parse("1..2").is_err());
    }

    #[test]
    fn test_is_prerelease() {
        // Regular release versions are not prerelease
        assert!(!req("1.0").is_prerelease());
        assert!(!req("= 1.0.0").is_prerelease());
        assert!(!req("> 1.2.3").is_prerelease());
        assert!(!req(">= 2.0").is_prerelease());
        assert!(!req("< 3.0.0").is_prerelease());
        assert!(!req("<= 1.9.9").is_prerelease());
        assert!(!req("~> 1.4").is_prerelease());

        // Prerelease versions are prerelease
        assert!(req("1.0.alpha").is_prerelease());
        assert!(req("= 1.0.0.beta").is_prerelease());
        assert!(req("> 1.2.3.rc1").is_prerelease());
        assert!(req(">= 2.0.pre").is_prerelease());
        assert!(req("< 3.0.0.dev").is_prerelease());
        assert!(req("<= 1.9.9.a").is_prerelease());
        assert!(req("~> 1.4.alpha.1").is_prerelease());

        // Mixed constraints - prerelease if ANY constraint has prerelease version
        let mixed_req = Requirement::new(vec![">= 1.0", "< 2.0.alpha"]).unwrap();
        assert!(mixed_req.is_prerelease());

        let all_release_req = Requirement::new(vec![">= 1.0", "< 2.0"]).unwrap();
        assert!(!all_release_req.is_prerelease());

        let all_prerelease_req = Requirement::new(vec![">= 1.0.alpha", "< 2.0.beta"]).unwrap();
        assert!(all_prerelease_req.is_prerelease());

        // Default requirement (>= 0) is not prerelease
        let default_req = Requirement::new(vec![""; 0]).unwrap();
        assert!(!default_req.is_prerelease());
    }

    fn ruby(version: &str) -> rv_ruby::Ruby {
        let version = RubyVersion::from_str(version).unwrap();
        let version_str = version.to_string();
        rv_ruby::Ruby {
            key: format!("{version_str}-macos-aarch64"),
            version,
            path: Default::default(),
            managed: false,
            symlink: None,
            arch: "aarch64".into(),
            os: "macos".into(),
            gem_root: None,
        }
    }

    #[test]
    fn test_select_ruby_version_for() {
        let constraints = vec![VersionConstraint {
            operator: ComparisonOperator::LessThan,
            version: "3.4".parse().unwrap(),
        }];
        let requirement: Requirement = constraints.into();

        let rubies = vec![ruby("ruby-3.2.10"), ruby("ruby-3.3.10"), ruby("ruby-3.4.8")];

        let expected = RubyVersion::from_str("ruby-3.3.10").unwrap();
        let selected_ruby = requirement.find_match_in(&rubies, false).unwrap();

        assert_eq!(expected, selected_ruby.version);
    }

    #[test]
    fn test_select_ruby_version_for_prereleases() {
        let constraints = vec![VersionConstraint {
            operator: ComparisonOperator::LessThan,
            version: "3.5".parse().unwrap(),
        }];
        let requirement: Requirement = constraints.into();

        let rubies = vec![
            ruby("ruby-3.2.10"),
            ruby("ruby-3.3.10"),
            ruby("ruby-3.4.8"),
            ruby("3.5.0-preview1"),
        ];

        let expected = RubyVersion::from_str("ruby-3.5.0-preview1").unwrap();
        let match_prereleases = true;
        let selected_ruby = requirement
            .find_match_in(&rubies, match_prereleases)
            .unwrap();
        assert_eq!(expected, selected_ruby.version);

        let expected = RubyVersion::from_str("ruby-3.4.8").unwrap();
        let match_prereleases = false;
        let selected_ruby = requirement
            .find_match_in(&rubies, match_prereleases)
            .unwrap();
        assert_eq!(expected, selected_ruby.version);
    }
}
