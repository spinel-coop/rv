use crate::{ParseError, ParseErrors, datatypes::*};
use miette::SourceSpan;
use winnow::{
    LocatingSlice, ModalResult, Parser,
    ascii::{newline, space0, space1},
    combinator::{alt, delimited, opt, preceded, repeat, terminated},
    error::{ContextError, ErrMode},
    stream::Location,
    token::{take_until, take_while},
};

pub type Input<'a> = LocatingSlice<&'a str>;

type Res<T> = ModalResult<T, ContextError>;

pub fn parse<'i>(file: &'i str) -> Result<Gemfile<'i>, ParseErrors> {
    let mut input = LocatingSlice::new(file);
    let i = &mut input;
    match repeat(1.., terminated(parse_item, newline)).parse_next(i) {
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
    alt((parse_source, parse_ruby_file, parse_gem)).parse_next(i)
}

/// e.g.
/// ruby file: ".ruby-version"
fn parse_ruby_file<'i>(i: &mut Input<'i>) -> Res<Item<'i>> {
    preceded("ruby file: ", parse_ruby_string)
        .map(Item::RubyFile)
        .parse_next(i)
}

/// Parse some string inside quotes, e.g. "foo"
fn parse_ruby_string<'i>(i: &mut Input<'i>) -> Res<&'i str> {
    delimited('"', parse_inside_string, '"').parse_next(i)
}

/// e.g.
/// gem "rails", "~> 7.2"
// gem "rack", ">= 2.2", "< 3.0"
fn parse_gem<'i>(i: &mut Input<'i>) -> Res<Item<'i>> {
    let _ = "gem".parse_next(i)?;
    let _ = space1.parse_next(i)?;
    let gem_name = parse_ruby_string.parse_next(i)?;
    let semver: Vec<_> =
        repeat(0.., preceded((',', space0), parse_semver_constraints)).parse_next(i)?;
    Ok(Item::Gem(GemRange {
        name: gem_name,
        semver,
        nonstandard: false,
    }))
}

/// e.g.
/// source "https://rubygems.org"
fn parse_source<'i>(i: &mut Input<'i>) -> Res<Item<'i>> {
    preceded("source ", parse_ruby_string)
        .map(Item::Source)
        .parse_next(i)
}

fn parse_inside_string<'i>(i: &mut Input<'i>) -> Res<&'i str> {
    take_until(0.., '"').parse_next(i)
}

fn parse_semver_constraints<'i>(i: &mut Input<'i>) -> Res<GemRangeSemver<'i>> {
    let _ = '"'.parse_next(i)?;
    let semver_constraint = parse_semver_constraint.parse_next(i)?;
    space1.parse_next(i)?;
    let version = parse_version.parse_next(i)?;
    let _ = '"'.parse_next(i)?;
    Ok(GemRangeSemver {
        semver_constraint,
        version,
    })
}

fn parse_semver_constraint<'i>(i: &mut Input<'i>) -> Res<SemverConstraint> {
    // Order matters here somewhat,
    // e.g. must parse >= before > otherwise >= would never get parsed,
    // because > is a substring of >=.
    alt((
        "!=".map(|_| SemverConstraint::NotEqual),
        ">=".map(|_| SemverConstraint::GreaterThanOrEqual),
        "<=".map(|_| SemverConstraint::LessThanOrEqual),
        ">".map(|_| SemverConstraint::GreaterThan),
        "<".map(|_| SemverConstraint::LessThan),
        "~>".map(|_| SemverConstraint::Pessimistic),
        "=".map(|_| SemverConstraint::Exact),
    ))
    .parse_next(i)
}

fn parse_version<'i>(i: &mut Input<'i>) -> Res<&'i str> {
    parse_version_inner.take().parse_next(i)
}

/// Equivalent to the regex
/// `[0-9]+(?>\.[0-9a-zA-Z]+)*(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?`,
/// except that we allow underscores in the last section where
/// prereleases or version architectures might go.
fn parse_version_inner<'i>(i: &mut Input<'i>) -> Res<()> {
    // [0-9]+
    let _major = parse_num.parse_next(i)?;

    // (?>\.[0-9a-zA-Z]+)*
    let _minor: Vec<_> =
        repeat(0.., ('.', take_while(1.., |c: char| c.is_alphanumeric()))).parse_next(i)?;

    // (-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?
    let _prerelease: Option<(char, &str, Vec<_>)> =
        opt(('-', alphanumdash, repeat(0.., ('.', alphanumdash)))).parse_next(i)?;

    Ok(())
}

fn parse_num(i: &mut Input<'_>) -> Res<u32> {
    take_while(1.., |c: char| c.is_ascii_digit())
        .try_map(|digits: &str| digits.parse::<u32>())
        .parse_next(i)
}

fn alphanumdash<'i>(i: &mut Input<'i>) -> Res<&'i str> {
    take_while(1.., |c: char| c.is_alphanumeric() || c == '-' || c == '_').parse_next(i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_source() {
        let input = r#"source "https://rubygems.org"
"#;
        let out = parse(input).unwrap();
        assert_eq!(out.items, vec![Item::Source("https://rubygems.org")]);
    }

    #[test]
    fn basic_rubyfile() {
        let input = r#"ruby file: ".ruby-version"
"#;
        let out = parse(input).unwrap();
        assert_eq!(out.items, vec![Item::RubyFile(".ruby-version")]);
    }

    #[test]
    fn basic_gem() {
        let input = r#"gem "rails", "~> 7.2"
"#;
        let out = parse(input).unwrap();
        assert_eq!(
            out.items,
            vec![Item::Gem(GemRange {
                name: "rails",
                semver: vec![GemRangeSemver {
                    semver_constraint: SemverConstraint::Pessimistic,
                    version: "7.2"
                }],
                nonstandard: false
            })]
        );
    }

    #[test]
    fn gemfile_with_no_sections() {
        let input = r#"source "https://rubygems.org"
ruby file: ".ruby-version"
gem "rails", "~> 7.2"
gem "mail", "< 2.9.0"
"#;
        let out = parse(input).unwrap();
        assert_eq!(
            out.items,
            vec![
                Item::Source("https://rubygems.org"),
                Item::RubyFile(".ruby-version"),
                Item::Gem(GemRange {
                    name: "rails",
                    semver: vec![GemRangeSemver {
                        semver_constraint: SemverConstraint::Pessimistic,
                        version: "7.2"
                    }],
                    nonstandard: false
                }),
                Item::Gem(GemRange {
                    name: "mail",
                    semver: vec![GemRangeSemver {
                        semver_constraint: SemverConstraint::LessThan,
                        version: "2.9.0"
                    }],
                    nonstandard: false
                })
            ]
        );
    }
}
