use thiserror::Error;

/// Top-level error type for lockfile operations
#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Parse error: {0}")]
    Parse(#[from] ParseError),
}

/// Detailed parsing errors with position information
#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Invalid indentation at line {line}: expected {expected} spaces, found {found}")]
    InvalidIndentation { 
        line: usize, 
        expected: usize, 
        found: usize 
    },
    
    #[error("Merge conflict detected at line {line}")]
    MergeConflict { 
        line: usize 
    },
    
    #[error("Invalid version format at line {line}: {version}")]
    InvalidVersion { 
        line: usize, 
        version: String 
    },
    
    #[error("Unknown source type at line {line}: {source_type}")]
    UnknownSourceType { 
        line: usize, 
        source_type: String 
    },
    
    #[error("Unexpected section at line {line}: {section}")]
    UnexpectedSection { 
        line: usize, 
        section: String 
    },
    
    #[error("Invalid gem specification at line {line}: {spec}")]
    InvalidSpecification { 
        line: usize, 
        spec: String 
    },
    
    #[error("Invalid dependency format at line {line}: {dependency}")]
    InvalidDependency { 
        line: usize, 
        dependency: String 
    },
    
    #[error("Invalid platform at line {line}: {platform}")]
    InvalidPlatform { 
        line: usize, 
        platform: String 
    },
    
    #[error("Missing required field at line {line}: {field}")]
    MissingField { 
        line: usize, 
        field: String 
    },
    
    #[error("Unexpected end of file while parsing {section}")]
    UnexpectedEof { 
        section: String 
    },
    
    #[error("Invalid checksum format at line {line}: {checksum}")]
    InvalidChecksum { 
        line: usize, 
        checksum: String 
    },
}

impl ParseError {
    pub fn line(&self) -> Option<usize> {
        match self {
            ParseError::InvalidIndentation { line, .. } => Some(*line),
            ParseError::MergeConflict { line } => Some(*line),
            ParseError::InvalidVersion { line, .. } => Some(*line),
            ParseError::UnknownSourceType { line, .. } => Some(*line),
            ParseError::UnexpectedSection { line, .. } => Some(*line),
            ParseError::InvalidSpecification { line, .. } => Some(*line),
            ParseError::InvalidDependency { line, .. } => Some(*line),
            ParseError::InvalidPlatform { line, .. } => Some(*line),
            ParseError::MissingField { line, .. } => Some(*line),
            ParseError::InvalidChecksum { line, .. } => Some(*line),
            ParseError::UnexpectedEof { .. } => None,
        }
    }
}