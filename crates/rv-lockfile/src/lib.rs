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
    /// The Gemfile.lock contents
    #[source_code]
    lockfile_contents: String,

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
