mod datatypes;
pub mod parser;
#[cfg(test)]
mod tests;

use miette::Diagnostic;
pub use parser::parse;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    CouldNotParse(ParseErrors),
}

#[derive(Debug, thiserror::Error, Diagnostic)]
#[error("Could not parse")]
pub struct ParseErrors {
    pub first: ParseError,
    #[related]
    pub others: Vec<ParseError>,
}

#[derive(Debug, thiserror::Error, Diagnostic)]
#[error("Could not parse")]
pub struct ParseError {
    char_offset: usize,
    msg: String,
}

pub type Result<T> = std::result::Result<T, Error>;
