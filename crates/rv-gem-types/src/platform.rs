#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Platform {
    Ruby,
    Current,
    Specific {
        cpu: Option<CPU>,
        os: String,
        version: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CPU {
    X86,
    X64,
    Arm64,
    Other(String),
}

impl Platform {
    pub fn matches(&self, other: &Platform) -> bool {
        // TODO: Implement platform matching logic
        false
    }
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Platform::Ruby => write!(f, "ruby"),
            Platform::Current => write!(f, "current"),
            Platform::Specific { cpu, os, version } => {
                // TODO: Implement proper platform string formatting
                write!(f, "{os}")
            }
        }
    }
}
