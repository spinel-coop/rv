use crate::{ParseError, ParseErrors, datatypes::*};
use miette::SourceSpan;
use winnow::{
    LocatingSlice, ModalResult, Parser,
    combinator::{delimited, preceded, repeat},
    error::{ContextError, ErrMode},
    stream::Location,
    token::take_until,
};

pub type Input<'a> = LocatingSlice<&'a str>;

type Res<T> = ModalResult<T, ContextError>;

pub fn parse<'i>(file: &'i str) -> Result<Gemfile<'i>, ParseErrors> {
    let mut input = LocatingSlice::new(file.trim());
    let i = &mut input;
    match repeat(1.., parse_item).parse_next(i) {
        Ok(items) => Ok(Gemfile { items }),
        Err(e) => {
            // OK, there was an error. Let's figure out where, to highlight it.
            let byte_offset = i.location();
            let char_offset = file[..byte_offset.min(file.len())].chars().count();
            // Then find the error message.
            let msg = match &e {
                ErrMode::Incomplete(_) => "unexpected end of input".to_string(),
                ErrMode::Backtrack(err) | ErrMode::Cut(err) => err.to_string(),
            };
            let parse_err = ParseError {
                char_offset: SourceSpan::new(char_offset.into(), 1),
                msg,
            };
            Err(ParseErrors {
                gemfile_contents: file.to_owned(),
                others: vec![parse_err],
            })
        }
    }
}

fn parse_item<'i>(i: &mut Input<'i>) -> Res<Item<'i>> {
    parse_source.parse_next(i)
}

// e.g.
// source "https://rubygems.org"
fn parse_source<'i>(i: &mut Input<'i>) -> Res<Item<'i>> {
    preceded("source ", delimited('"', parse_source_url, '"')).parse_next(i)
}

fn parse_source_url<'i>(i: &mut Input<'i>) -> Res<Item<'i>> {
    take_until(0.., '"').map(Item::Source).parse_next(i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        let input = r#"source "https://rubygems.org"
"#;
        let out = parse(input).unwrap();
        assert_eq!(out.items, vec![Item::Source("https://rubygems.org")]);
    }
}
