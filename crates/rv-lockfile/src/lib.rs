// https://github.com/zkat/miette/issues/458
#![expect(unused_assignments, reason = "miette macros trigger false positives")]

pub mod datatypes;
mod parser;
#[cfg(test)]
mod tests;

use std::borrow::Cow;

use miette::{Diagnostic, SourceSpan};
pub use parser::parse;

/// Normalize line endings in a lockfile string.
///
/// Converts Windows-style line endings (`\r\n`) to Unix-style (`\n`).
/// This should be called before passing the string to [`parse`], as the parser
/// expects Unix line endings.
///
/// Returns a [`Cow::Borrowed`] if no conversion is needed (the string already
/// uses Unix line endings), or a [`Cow::Owned`] with the normalized string.
pub fn normalize_line_endings(contents: &str) -> Cow<'_, str> {
    if contents.contains("\r\n") {
        Cow::Owned(line_ending::LineEnding::normalize(contents))
    } else {
        Cow::Borrowed(contents)
    }
}

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
