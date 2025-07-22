use miette::Diagnostic;
use std::io;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug, Diagnostic)]
pub enum Error {
    #[error("invalid gem format: {message}")]
    #[diagnostic(code(rv_gem_package::format_error))]
    FormatError { message: String },

    #[error("checksum mismatch: {message}")]
    #[diagnostic(code(rv_gem_package::checksum_error))]
    ChecksumError { message: String },

    #[error("unsupported old gem format")]
    #[diagnostic(
        code(rv_gem_package::old_format_error),
        help("This gem uses the pre-2007 format which is not yet supported")
    )]
    OldFormatError,

    #[error("tar archive error: {message}")]
    #[diagnostic(code(rv_gem_package::tar_error))]
    TarError { message: String },

    #[error("IO error")]
    #[diagnostic(code(rv_gem_package::io_error))]
    IoError(#[from] io::Error),

    #[error("YAML parsing error: {0}")]
    #[diagnostic(code(rv_gem_package::yaml_error))]
    YamlError(String),
}

impl Error {
    pub fn format_error(message: impl Into<String>) -> Self {
        Self::FormatError {
            message: message.into(),
        }
    }

    pub fn checksum_error(message: impl Into<String>) -> Self {
        Self::ChecksumError {
            message: message.into(),
        }
    }

    pub fn tar_error(message: impl Into<String>) -> Self {
        Self::TarError {
            message: message.into(),
        }
    }
}
