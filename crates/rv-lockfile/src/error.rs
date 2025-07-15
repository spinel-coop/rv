use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

/// Top-level error type for lockfile operations
#[derive(Debug, Error, Diagnostic)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    Parse(#[from] ParseError),
}

/// Detailed parsing errors with position information
#[derive(Debug, Error, Diagnostic)]
pub enum ParseError {
    #[error("Invalid indentation: expected {expected} spaces, found {found}")]
    #[diagnostic(
        code(lockfile::invalid_indentation),
        help("Lockfile indentation must be consistent with 2 or 4 spaces")
    )]
    InvalidIndentation {
        line: usize,
        expected: usize,
        found: usize,
        #[source_code]
        source_code: String,
        #[label("here")]
        span: SourceSpan,
    },

    #[error("Merge conflict detected")]
    #[diagnostic(
        code(lockfile::merge_conflict),
        help("Resolve the merge conflict and regenerate the lockfile")
    )]
    MergeConflict {
        line: usize,
        #[source_code]
        source_code: String,
        #[label("merge conflict markers found here")]
        span: SourceSpan,
    },

    #[error("Invalid version format: {version}")]
    #[diagnostic(
        code(lockfile::invalid_version),
        help("Version must follow semantic versioning format (e.g., 1.2.3)")
    )]
    InvalidVersion {
        line: usize,
        version: String,
        #[source_code]
        source_code: String,
        #[label("invalid version")]
        span: SourceSpan,
    },

    #[error("Unknown source type: {source_type}")]
    #[diagnostic(
        code(lockfile::unknown_source),
        help("Valid source types are: GEM, GIT, PATH, PLUGIN")
    )]
    UnknownSourceType {
        line: usize,
        source_type: String,
        #[source_code]
        source_code: String,
        #[label("unknown source type")]
        span: SourceSpan,
    },

    #[error("Unexpected section: {section}")]
    #[diagnostic(
        code(lockfile::unexpected_section),
        help("Valid sections are: GEM, GIT, PATH, PLUGIN, PLATFORMS, DEPENDENCIES, RUBY VERSION, BUNDLED WITH, CHECKSUMS")
    )]
    UnexpectedSection {
        line: usize,
        section: String,
        #[source_code]
        source_code: String,
        #[label("unexpected section")]
        span: SourceSpan,
    },

    #[error("Invalid gem specification: {spec}")]
    #[diagnostic(
        code(lockfile::invalid_spec),
        help("Gem specifications should follow format: 'name (version)' or 'name (version-platform)'")
    )]
    InvalidSpecification {
        line: usize,
        spec: String,
        #[source_code]
        source_code: String,
        #[label("invalid specification")]
        span: SourceSpan,
    },

    #[error("Invalid dependency format: {dependency}")]
    #[diagnostic(
        code(lockfile::invalid_dependency),
        help("Dependencies should follow format: 'name' or 'name (constraints)'")
    )]
    InvalidDependency {
        line: usize,
        dependency: String,
        #[source_code]
        source_code: String,
        #[label("invalid dependency")]
        span: SourceSpan,
    },

    #[error("Invalid platform: {platform}")]
    #[diagnostic(
        code(lockfile::invalid_platform),
        help(
            "Platforms should be valid Ruby platform identifiers like 'ruby', 'x86_64-linux', etc."
        )
    )]
    InvalidPlatform {
        line: usize,
        platform: String,
        #[source_code]
        source_code: String,
        #[label("invalid platform")]
        span: SourceSpan,
    },

    #[error("Missing required field: {field}")]
    #[diagnostic(
        code(lockfile::missing_field),
        help("This field is required for proper lockfile parsing")
    )]
    MissingField {
        line: usize,
        field: String,
        #[source_code]
        source_code: String,
        #[label("missing field")]
        span: SourceSpan,
    },

    #[error("Unexpected end of file while parsing {section}")]
    #[diagnostic(
        code(lockfile::unexpected_eof),
        help("The lockfile appears to be truncated or malformed")
    )]
    UnexpectedEof { section: String },

    #[error("Invalid checksum format: {checksum}")]
    #[diagnostic(
        code(lockfile::invalid_checksum),
        help("Checksums should be in format: 'name (version) algorithm=hash'")
    )]
    InvalidChecksum {
        line: usize,
        checksum: String,
        #[source_code]
        source_code: String,
        #[label("invalid checksum")]
        span: SourceSpan,
    },
}

impl ParseError {
    pub fn line(&self) -> Option<usize> {
        match self {
            ParseError::InvalidIndentation { line, .. } => Some(*line),
            ParseError::MergeConflict { line, .. } => Some(*line),
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

    /// Create an InvalidIndentation error with default values
    pub fn invalid_indentation(line: usize, expected: usize, found: usize) -> Self {
        ParseError::InvalidIndentation {
            line,
            expected,
            found,
            source_code: String::new(),
            span: SourceSpan::new(0.into(), 0),
        }
    }

    /// Create a MergeConflict error with default values
    pub fn merge_conflict(line: usize) -> Self {
        ParseError::MergeConflict {
            line,
            source_code: String::new(),
            span: SourceSpan::new(0.into(), 0),
        }
    }

    /// Create an InvalidVersion error with default values
    pub fn invalid_version(line: usize, version: String) -> Self {
        ParseError::InvalidVersion {
            line,
            version,
            source_code: String::new(),
            span: SourceSpan::new(0.into(), 0),
        }
    }

    /// Create an UnknownSourceType error with default values
    pub fn unknown_source_type(line: usize, source_type: String) -> Self {
        ParseError::UnknownSourceType {
            line,
            source_type,
            source_code: String::new(),
            span: SourceSpan::new(0.into(), 0),
        }
    }

    /// Create an UnexpectedSection error with default values
    pub fn unexpected_section(line: usize, section: String) -> Self {
        ParseError::UnexpectedSection {
            line,
            section,
            source_code: String::new(),
            span: SourceSpan::new(0.into(), 0),
        }
    }

    /// Create an InvalidSpecification error with default values
    pub fn invalid_specification(line: usize, spec: String) -> Self {
        ParseError::InvalidSpecification {
            line,
            spec,
            source_code: String::new(),
            span: SourceSpan::new(0.into(), 0),
        }
    }

    /// Create an InvalidDependency error with default values
    pub fn invalid_dependency(line: usize, dependency: String) -> Self {
        ParseError::InvalidDependency {
            line,
            dependency,
            source_code: String::new(),
            span: SourceSpan::new(0.into(), 0),
        }
    }

    /// Create an InvalidPlatform error with default values
    pub fn invalid_platform(line: usize, platform: String) -> Self {
        ParseError::InvalidPlatform {
            line,
            platform,
            source_code: String::new(),
            span: SourceSpan::new(0.into(), 0),
        }
    }

    /// Create a MissingField error with default values
    pub fn missing_field(line: usize, field: String) -> Self {
        ParseError::MissingField {
            line,
            field,
            source_code: String::new(),
            span: SourceSpan::new(0.into(), 0),
        }
    }

    /// Create an InvalidChecksum error with default values
    pub fn invalid_checksum(line: usize, checksum: String) -> Self {
        ParseError::InvalidChecksum {
            line,
            checksum,
            source_code: String::new(),
            span: SourceSpan::new(0.into(), 0),
        }
    }

    /// Create a new error with source context for better diagnostics
    pub fn with_source_context(
        mut self,
        source_code: impl Into<String>,
        line_start: usize,
        line_content: &str,
    ) -> Self {
        let source = source_code.into();
        let line_offset = source
            .lines()
            .take(line_start)
            .map(|l| l.len() + 1) // +1 for newline
            .sum::<usize>();
        let span = SourceSpan::new(line_offset.into(), line_content.len());

        match &mut self {
            ParseError::InvalidIndentation {
                source_code: sc,
                span: sp,
                ..
            } => {
                *sc = source;
                *sp = span;
            }
            ParseError::MergeConflict {
                source_code: sc,
                span: sp,
                ..
            } => {
                *sc = source;
                *sp = span;
            }
            ParseError::InvalidVersion {
                source_code: sc,
                span: sp,
                ..
            } => {
                *sc = source;
                *sp = span;
            }
            ParseError::UnknownSourceType {
                source_code: sc,
                span: sp,
                ..
            } => {
                *sc = source;
                *sp = span;
            }
            ParseError::UnexpectedSection {
                source_code: sc,
                span: sp,
                ..
            } => {
                *sc = source;
                *sp = span;
            }
            ParseError::InvalidSpecification {
                source_code: sc,
                span: sp,
                ..
            } => {
                *sc = source;
                *sp = span;
            }
            ParseError::InvalidDependency {
                source_code: sc,
                span: sp,
                ..
            } => {
                *sc = source;
                *sp = span;
            }
            ParseError::InvalidPlatform {
                source_code: sc,
                span: sp,
                ..
            } => {
                *sc = source;
                *sp = span;
            }
            ParseError::MissingField {
                source_code: sc,
                span: sp,
                ..
            } => {
                *sc = source;
                *sp = span;
            }
            ParseError::InvalidChecksum {
                source_code: sc,
                span: sp,
                ..
            } => {
                *sc = source;
                *sp = span;
            }
            ParseError::UnexpectedEof { .. } => {
                // This error doesn't have source context
            }
        }

        self
    }
}
