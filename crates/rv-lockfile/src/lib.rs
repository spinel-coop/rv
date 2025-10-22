mod datatypes;
pub mod parser;
#[cfg(test)]
mod tests;

pub use parser::parse;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Could not parse the file, starting at char index {char_offset}: {msg}")]
    CouldNotParse { char_offset: usize, msg: String },
}

pub type Result<T> = std::result::Result<T, Error>;
