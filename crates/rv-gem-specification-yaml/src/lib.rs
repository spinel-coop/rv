// https://github.com/zkat/miette/issues/458
#![expect(unused_assignments, reason = "miette macros trigger false positives")]

//! # rv-gem-specification-yaml
//!
//! A Rust library for parsing and serializing Ruby gem specification YAML files.
//!
//! This crate provides functionality to parse the `metadata.gz` files found in Ruby gem
//! packages (`.gem` files) and convert them to structured Rust types, as well as serialize
//! specifications back to YAML format.
//!
//! ## Features
//!
//! - **High Compatibility**: Successfully parses 99.8% of real-world Ruby gems (7584/7602 gems tested)
//! - **Round-trip Serialization**: Parse YAML specifications and serialize them back to YAML
//! - **Structured Error Types**: Detailed error reporting with diagnostic information
//! - **Ruby Compatibility**: Handles various Ruby gem specification formats and legacy patterns
//! - **Semantic Null Handling**: Properly preserves null values in authors and email arrays
//!
//! ## Basic Usage
//!
//! ```rust
//! use rv_gem_specification_yaml::parse;
//!
//! let yaml_content = r#"
//! --- !ruby/object:Gem::Specification
//! name: example-gem
//! version: !ruby/object:Gem::Version
//!   version: 1.0.0
//! summary: An example gem
//! authors:
//! - Test Author
//! dependencies: []
//! "#;
//!
//! let specification = parse(yaml_content)?;
//! println!("Gem: {} v{}", specification.name, specification.version);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Known Limitations
//!
//! The following Ruby gem patterns are **not yet supported** (affecting 0.2% of gems):
//!
//! ### YAML Folded Scalar Syntax
//! ```yaml
//! description: ! 'Multi-line text with
//!   folded scalar syntax...'
//! ```
//! **Example**: `bacon-1.2.0.gem` - Valid YAML syntax not supported by parser
//!
//! ### Gem::Version::Requirement Class
//! ```yaml
//! required_ruby_version: !ruby/object:Gem::Version::Requirement
//!   requirements:
//!   - - ">"
//!     - !ruby/object:Gem::Version
//!       version: 0.0.0
//! ```
//! **Example**: `terminal-table-1.4.5.gem` - Different class hierarchy than standard `Gem::Requirement`
//!
//! ### YAML Anchors and References
//! ```yaml
//! dependencies:
//! - !ruby/object:Gem::Dependency
//!   requirement: &id001 !ruby/object:Gem::Requirement
//!     # ... requirement definition
//!   version_requirements: *id001
//!   prerelease: false
//! ```
//! **Example**: `mocha-on-bacon-0.2.2.gem` - YAML anchors/references and dependency `prerelease` field not implemented

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
