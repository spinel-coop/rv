use miette::Diagnostic;
use std::io;
use thiserror::Error;

pub type Result<T> = miette::Result<T, Error>;

#[derive(Error, Debug, Diagnostic)]
pub enum Error {
    #[error("gem format error")]
    #[diagnostic(code(rv_gem_package::format_error))]
    FormatError(#[from] FormatErrorKind),

    #[error("checksum error")]
    #[diagnostic(code(rv_gem_package::checksum_error))]
    ChecksumError(#[from] ChecksumErrorKind),

    #[error("unsupported old gem format")]
    #[diagnostic(
        code(rv_gem_package::old_format_error),
        help("This gem uses the pre-2007 format which is not yet supported")
    )]
    OldFormatError,

    #[error("tar archive error")]
    #[diagnostic(code(rv_gem_package::tar_error))]
    TarError(#[from] TarErrorKind),

    #[error("IO error")]
    #[diagnostic(code(rv_gem_package::io_error))]
    IoError(#[from] io::Error),

    #[error("YAML parsing error")]
    #[diagnostic(code(rv_gem_package::yaml_error))]
    YamlParsing(#[diagnostic_source] miette::Report),
}

#[derive(Error, Debug, Diagnostic)]
pub enum FormatErrorKind {
    #[error("missing required file '{expected_file}' in gem")]
    #[diagnostic(help("The gem archive must contain this file for proper operation"))]
    MissingFile { expected_file: String },

    #[error("invalid UTF-8 content in '{file_name}'")]
    #[diagnostic(help("Gem files must contain valid UTF-8 text"))]
    InvalidUtf8 {
        file_name: String,
        #[source]
        source: std::string::FromUtf8Error,
    },

    #[error("invalid YAML content in '{file_name}'")]
    #[diagnostic(help("YAML content could not be parsed"))]
    InvalidYaml {
        file_name: String,
        #[source]
        source: saphyr::ScanError,
    },

    #[error("empty YAML document in '{file_name}'")]
    #[diagnostic(help("YAML file exists but contains no valid documents"))]
    EmptyYaml { file_name: String },
}

#[derive(Error, Debug, Diagnostic)]
pub enum ChecksumErrorKind {
    #[error("unsupported checksum algorithm '{algorithm}'")]
    #[diagnostic(help("Supported algorithms are: SHA1, SHA256, SHA512"))]
    UnsupportedAlgorithm { algorithm: String },

    #[error("checksum mismatch for '{file_path}' using {algorithm}")]
    #[diagnostic(help("Expected: {expected}, Got: {actual}"))]
    Mismatch {
        file_path: String,
        algorithm: String,
        expected: String,
        actual: String,
    },

    #[error("file '{file_path}' not found but listed in checksums")]
    #[diagnostic(help("The checksums file references a file that doesn't exist in the gem"))]
    MissingFile { file_path: String },
}

#[derive(Error, Debug, Diagnostic)]
pub enum TarErrorKind {
    #[error("failed to read tar archive")]
    #[diagnostic(help("The tar archive may be corrupted or incomplete"))]
    ArchiveRead(#[from] std::io::Error),

    #[error("unsupported tar entry type: {entry_type:?}")]
    #[diagnostic(help("Only regular files, directories, and symlinks are supported"))]
    UnsupportedEntryType { entry_type: String },

    #[error("symlink entry missing target")]
    #[diagnostic(help("Symlink entries must specify a target path"))]
    MissingSymlinkTarget,
}

impl Error {
    // Format error constructors
    pub fn missing_file(expected_file: impl Into<String>) -> Self {
        FormatErrorKind::MissingFile {
            expected_file: expected_file.into(),
        }
        .into()
    }

    pub fn invalid_utf8(file_name: impl Into<String>, source: std::string::FromUtf8Error) -> Self {
        FormatErrorKind::InvalidUtf8 {
            file_name: file_name.into(),
            source,
        }
        .into()
    }

    pub fn invalid_yaml(file_name: impl Into<String>, source: saphyr::ScanError) -> Self {
        FormatErrorKind::InvalidYaml {
            file_name: file_name.into(),
            source,
        }
        .into()
    }

    pub fn empty_yaml(file_name: impl Into<String>) -> Self {
        FormatErrorKind::EmptyYaml {
            file_name: file_name.into(),
        }
        .into()
    }

    // Checksum error constructors
    pub fn unsupported_algorithm(algorithm: impl Into<String>) -> Self {
        ChecksumErrorKind::UnsupportedAlgorithm {
            algorithm: algorithm.into(),
        }
        .into()
    }

    pub fn checksum_mismatch(
        file_path: impl Into<String>,
        algorithm: impl Into<String>,
        expected: impl Into<String>,
        actual: impl Into<String>,
    ) -> Self {
        ChecksumErrorKind::Mismatch {
            file_path: file_path.into(),
            algorithm: algorithm.into(),
            expected: expected.into(),
            actual: actual.into(),
        }
        .into()
    }

    pub fn checksum_missing_file(file_path: impl Into<String>) -> Self {
        ChecksumErrorKind::MissingFile {
            file_path: file_path.into(),
        }
        .into()
    }

    // Tar error constructors (for cases without #[from])
    pub fn tar_unsupported_entry_type(entry_type: impl Into<String>) -> Self {
        TarErrorKind::UnsupportedEntryType {
            entry_type: entry_type.into(),
        }
        .into()
    }

    pub fn tar_missing_symlink_target() -> Self {
        TarErrorKind::MissingSymlinkTarget.into()
    }
}
