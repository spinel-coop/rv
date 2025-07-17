use either::Either;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub version: String,
    pub segments: Vec<Either<String, u32>>,
}

impl Version {
    pub fn new(version: String) -> Self {
        let segments = Self::parse_segments(&version);
        Self { version, segments }
    }

    fn parse_segments(version: &str) -> Vec<Either<String, u32>> {
        // TODO: Implement proper version parsing
        vec![]
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.version)
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // TODO: Implement proper version comparison
        self.version.cmp(&other.version)
    }
}
