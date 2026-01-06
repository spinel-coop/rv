use crate::{ParseError, ParseErrors, datatypes::*};
use miette::SourceSpan;
use winnow::{
    LocatingSlice, ModalResult, Parser,
    ascii::{line_ending, space0, space1},
    combinator::{alt, delimited, dispatch, opt, peek, preceded, repeat, separated, terminated},
    error::{ContextError, ErrMode},
    stream::{AsChar, Location, Stream},
    token::{take_until, take_while},
};

pub type Input<'a> = LocatingSlice<&'a str>;

type Res<T> = ModalResult<T, ContextError>;

pub fn parse<'i>(file: &'i str) -> Result<Gemfile<'i>, ParseErrors> {
    let mut input = LocatingSlice::new(file);
    let i = &mut input;
    let mut parsed = Gemfile::default();
    let mut error: Option<ParseErrors> = None;

    let error = todo!();

    match error {
        None => Ok(parsed),
        Some(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        todo!()
    }
}
