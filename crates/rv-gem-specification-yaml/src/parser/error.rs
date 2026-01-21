use miette::{Diagnostic, SourceSpan};

#[derive(Debug, thiserror::Error, Diagnostic)]
pub enum DeserializationError {
    #[error("YAML parsing error")]
    #[diagnostic(
        code(yaml::de::parse),
        help("Check YAML syntax - this typically indicates malformed YAML structure")
    )]
    Parse {
        #[source]
        source: saphyr_parser::ScanError,
        #[label("{source}")]
        bad_bit: SourceSpan,
    },
    #[error("Expected {expected}, found {found}")]
    #[diagnostic(
        code(yaml::de::expected_event),
        help("Check YAML structure for correct event types")
    )]
    ExpectedEvent {
        expected: String,
        found: String,

        #[label("expected {expected}")]
        bad_bit: SourceSpan,
    },
    #[error("Unexpected tag: expected {expected}, found {found}")]
    #[diagnostic(
        code(yaml::de::unexpected_tag),
        help("YAML tags must match the expected Ruby object types")
    )]
    UnexpectedTag {
        expected: String,
        found: String,

        #[label("unexpected tag here")]
        bad_bit: SourceSpan,
    },
    #[error("Unexpected end of input: {message}")]
    #[diagnostic(
        code(yaml::de::unexpected_end),
        help("YAML document ended unexpectedly")
    )]
    UnexpectedEnd {
        message: String,

        #[label("unexpected end here")]
        bad_bit: SourceSpan,
    },
    #[error("Missing required field: {field}")]
    #[diagnostic(
        code(yaml::de::missing_field),
        help("All Gem::Specification objects must have 'name' and 'version' fields")
    )]
    MissingField {
        field: String,

        #[label("field '{field}' is required but missing")]
        bad_bit: SourceSpan,
    },
    #[error("Invalid type for field {field}: expected {expected}")]
    #[diagnostic(
        code(yaml::de::invalid_type),
        help("Check the YAML field type matches the expected Ruby object structure")
    )]
    InvalidType {
        field: String,
        expected: String,

        #[label("expected {expected}")]
        bad_bit: SourceSpan,
    },
    #[error("Dependency error: {0}")]
    #[diagnostic(code(yaml::de::dependency))]
    Dependency(#[from] rv_gem_types::DependencyError),
    #[error("Version error: {0}")]
    #[diagnostic(code(yaml::de::version))]
    Version(#[from] rv_gem_types::VersionError),
    #[error("Requirement error: {0}")]
    #[diagnostic(code(yaml::de::requirement))]
    Requirement(#[from] rv_gem_types::requirement::RequirementError),
    #[error("Specification error: {0}")]
    #[diagnostic(code(yaml::de::specification))]
    Specification(#[from] rv_gem_types::SpecificationError),
    #[error("Platform error: {0}")]
    #[diagnostic(code(yaml::de::platform))]
    Platform(#[from] rv_gem_types::platform::PlatformError),
}
