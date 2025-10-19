mod datatypes;
pub mod parser;
#[cfg(test)]
mod tests;

pub use parser::parse;

#[derive(thiserror::Error, Debug)]
pub enum Error {}
