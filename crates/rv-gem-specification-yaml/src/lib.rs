use miette::Diagnostic;
use rv_gem_types::requirement::RequirementError;

pub mod parser;
pub mod serialize;

use saphyr::EmitError;
pub use serialize::serialize_specification_to_yaml;

#[derive(Debug, thiserror::Error, Diagnostic)]
pub enum SerializationError {
    #[error("YAML serialization error: {emit_error}")]
    #[diagnostic(
        code(yaml::ser::emit),
        help("Failed to generate YAML output - this typically indicates an internal error")
    )]
    Emit {
        #[source]
        emit_error: EmitError,
    },
    #[error("Invalid structure for serialization: {message}")]
    #[diagnostic(
        code(yaml::ser::structure),
        help("The specification structure is invalid for YAML serialization")
    )]
    Structure { message: String },
    #[error("Version error: {0}")]
    #[diagnostic(code(yaml::ser::version))]
    Version(#[from] rv_gem_types::VersionError),
    #[error("Requirement error: {0}")]
    #[diagnostic(code(yaml::ser::requirement))]
    Requirement(#[from] RequirementError),
    #[error("Dependency error: {0}")]
    #[diagnostic(code(yaml::ser::dependency))]
    Dependency(#[from] rv_gem_types::DependencyError),
}

pub use parser::parse;
