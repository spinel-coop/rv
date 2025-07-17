use crate::Version;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requirement {
    pub constraints: Vec<VersionConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionConstraint {
    pub operator: ComparisonOperator,
    pub version: Version,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComparisonOperator {
    Equal,
    NotEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
    Pessimistic,
}

impl Requirement {
    pub fn new(constraints: Vec<VersionConstraint>) -> Self {
        Self { constraints }
    }

    pub fn matches(&self, version: &Version) -> bool {
        // TODO: Implement requirement matching logic
        false
    }
}

impl std::fmt::Display for ComparisonOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComparisonOperator::Equal => write!(f, "="),
            ComparisonOperator::NotEqual => write!(f, "!="),
            ComparisonOperator::Greater => write!(f, ">"),
            ComparisonOperator::GreaterEqual => write!(f, ">="),
            ComparisonOperator::Less => write!(f, "<"),
            ComparisonOperator::LessEqual => write!(f, "<="),
            ComparisonOperator::Pessimistic => write!(f, "~>"),
        }
    }
}
