// https://github.com/zkat/miette/issues/458
#![expect(unused_assignments, reason = "miette macros trigger false positives")]

pub mod datatypes;
mod parser;
#[cfg(test)]
mod tests;

use miette::{Diagnostic, SourceSpan};
pub use parser::parse;

#[derive(Debug, thiserror::Error, Diagnostic)]
#[error("Could not parse")]
#[diagnostic()]
pub struct ParseErrors {
    /// The Gemfile contents
    #[source_code]
    gemfile_contents: String,

    /// Any other errors.
    #[related]
    pub others: Vec<ParseError>,
}

#[derive(Debug, thiserror::Error, Diagnostic)]
#[error("Could not parse: {msg}")]
#[diagnostic()]
pub struct ParseError {
    /// Where parsing failed.
    #[label("Parsing failed here")]
    char_offset: SourceSpan,

    /// Error message
    msg: String,
}
