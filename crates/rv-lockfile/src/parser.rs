use crate::datatypes::*;
use winnow::{
    LocatingSlice, ModalResult, Parser,
    ascii::{line_ending, space0, space1},
    combinator::{alt, delimited, dispatch, opt, peek, preceded, repeat, separated, terminated},
    error::ContextError,
    stream::AsChar,
    token::{take_until, take_while},
};

pub type Input<'a> = LocatingSlice<&'a str>;

type Res<T> = ModalResult<T, ContextError>;

#[derive(Debug)]
enum Section<'i> {
    Git(GitSection<'i>),
    Gem(GemSection<'i>),
    Path(PathSection<'i>),
    Platforms(Vec<&'i str>),
    Dependencies(Vec<GemRange<'i>>),
    RubyVersion(&'i str),
    BundledWith(&'i str),
    Checksums(Vec<Checksum<'i>>),
}

pub fn parse<'i>(file: &'i str) -> crate::Result<GemfileDotLock<'i>> {
    let input = LocatingSlice::new(file);
    parse_winnow.parse(input).map_err(|e| {
        let char_offset = todo!();
        let msg = e.to_string();
        crate::Error::CouldNotParse { char_offset, msg }
    })
}

fn parse_winnow<'i>(i: &mut Input<'i>) -> Res<GemfileDotLock<'i>> {
    let mut parsed = GemfileDotLock::default();

    let sections: Vec<_> = repeat(
        1..,
        dispatch! {peek(parse_section_header);
            "GIT" => paragraph(parse_git_section).map(Section::Git),
            "GEM" => paragraph(parse_gem).map(Section::Gem),
            "PATH" => paragraph(parse_path).map(Section::Path),
            "PLATFORMS" => paragraph(parse_platforms).map(Section::Platforms),
            "DEPENDENCIES" => paragraph(parse_dependencies).map(Section::Dependencies),
            "CHECKSUMS" => paragraph(parse_checksums).map(Section::Checksums),
            "RUBY VERSION" => paragraph(parse_ruby_version).map(Section::RubyVersion),
            "BUNDLED WITH" => paragraph(parse_bundled_with).map(Section::BundledWith),
            _ => winnow::combinator::fail::<_,Section,_>,
        },
    )
    .parse_next(i)?;

    for section in sections {
        match section {
            Section::Git(section) => {
                parsed.git.push(section);
            }
            Section::Gem(section) => {
                parsed.gem.push(section);
            }
            Section::Path(section) => {
                parsed.path.push(section);
            }
            Section::Platforms(section) => {
                parsed.platforms = section;
            }
            Section::Dependencies(section) => {
                parsed.dependencies = section;
            }
            Section::RubyVersion(section) => {
                parsed.ruby_version = Some(section);
            }
            Section::BundledWith(section) => {
                parsed.bundled_with = Some(section);
            }
            Section::Checksums(section) => {
                parsed.checksums = Some(section);
            }
        }
    }

    Ok(parsed)
}

/// Parse a paragraph, i.e. something ending in a new line.
fn paragraph<'i, O, F>(parser: F) -> impl Parser<Input<'i>, O, ContextError>
where
    F: Parser<Input<'i>, O, ContextError>,
{
    terminated(parser, parse_empty_lines)
}

fn parse_section_header<'i>(i: &mut Input<'i>) -> Res<&'i str> {
    terminated(
        take_while(1.., |c: char| c.is_ascii_uppercase() || c == ' '),
        terminated(space0, line_ending),
    )
    .parse_next(i)
}

fn parse_empty_lines<'i>(i: &mut Input<'i>) -> Res<()> {
    let _ = space0.parse_next(i)?;
    let _: Vec<_> = repeat(0.., line_ending).parse_next(i)?;
    Ok(())
}

fn parse_spec<'i>(i: &mut Input<'i>) -> Res<Spec<'i>> {
    "    ".parse_next(i)?;
    let spec = parse_spec_no_delimiters.parse_next(i)?;
    Ok(spec)
}

fn parse_spec_no_delimiters<'i>(i: &mut Input<'i>) -> Res<Spec<'i>> {
    let name = parse_gem_name.parse_next(i)?;
    space1.parse_next(i)?;
    let version = delimited('(', parse_version, ")\n").parse_next(i)?;
    let gem_version = GemVersion { name, version };
    let deps = repeat(0.., parse_spec_dep).parse_next(i)?;
    Ok(Spec { gem_version, deps })
}

fn parse_spec_dep<'i>(i: &mut Input<'i>) -> Res<GemRange<'i>> {
    "      ".parse_next(i)?;
    let out = parse_dependency.parse_next(i)?;
    line_ending.parse_next(i)?;
    Ok(out)
}

fn parse_dependency<'i>(i: &mut Input<'i>) -> Res<GemRange<'i>> {
    let name = parse_gem_name.parse_next(i)?;
    let semver = opt(spec_dep_semver).parse_next(i)?;
    let nonstandard = opt('!').parse_next(i)?;
    Ok(GemRange {
        name,
        semver,
        nonstandard: nonstandard.is_some(),
    })
}

fn spec_dep_semver<'i>(i: &mut Input<'i>) -> Res<Vec<GemRangeSemver<'i>>> {
    space1.parse_next(i)?;
    '('.parse_next(i)?;
    let out = separated(1.., parse_semver_constraints, terminated(',', space0)).parse_next(i)?;
    ')'.parse_next(i)?;
    Ok(out)
}

fn parse_semver_constraints<'i>(i: &mut Input<'i>) -> Res<GemRangeSemver<'i>> {
    let semver_constraint = parse_semver_constraint.parse_next(i)?;
    space1.parse_next(i)?;
    let version = parse_version.parse_next(i)?;
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

fn parse_gem_name<'i>(i: &mut Input<'i>) -> Res<&'i str> {
    take_while(0.., |c: char| {
        c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' || c == '/'
    })
    .parse_next(i)
}

fn parse_bundled_with_contents<'i>(i: &mut Input<'i>) -> Res<&'i str> {
    take_while(0.., |c: char| {
        c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.'
    })
    .parse_next(i)
}

fn parse_ruby_version_contents<'i>(i: &mut Input<'i>) -> Res<&'i str> {
    take_while(0.., |c: char| {
        c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' || c == ' '
    })
    .parse_next(i)
}

fn alphanumdash<'i>(i: &mut Input<'i>) -> Res<&'i str> {
    take_while(1.., |c: char| c.is_alphanumeric() || c == '-' || c == '_').parse_next(i)
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

fn parse_hex_string<'i>(i: &mut Input<'i>) -> Res<&'i str> {
    take_while(1.., |c: char| c.is_hex_digit()).parse_next(i)
}

fn parse_checksum<'i>(i: &mut Input<'i>) -> Res<Checksum<'i>> {
    // nokogiri (1.18.10-arm-linux-gnu) sha256=51f4f25ab5d5ba1012d6b16aad96b840a10b067b93f35af6a55a2c104a7ee322
    let name = parse_gem_name.parse_next(i)?;
    space1.parse_next(i)?;
    '('.parse_next(i)?;
    let version = parse_version.parse_next(i)?;
    ')'.parse_next(i)?;
    space1.parse_next(i)?;
    "sha256=".parse_next(i)?;
    let sha256 = parse_hex_string.try_map(hex::decode).parse_next(i)?;
    Ok(Checksum {
        gem_version: GemVersion { name, version },
        sha256,
    })
}

fn parse_num(i: &mut Input<'_>) -> Res<u32> {
    take_while(1.., |c: char| c.is_ascii_digit())
        .try_map(|digits: &str| digits.parse::<u32>())
        .parse_next(i)
}

fn parse_git_section<'i>(i: &mut Input<'i>) -> Res<GitSection<'i>> {
    "GIT\n".parse_next(i)?;
    let remote = delimited("  remote: ", parse_remote, line_ending).parse_next(i)?;
    let revision = delimited("  revision: ", parse_hex_string, line_ending).parse_next(i)?;
    "  specs:\n".parse_next(i)?;
    let specs = repeat(0.., parse_spec).parse_next(i)?;
    Ok(GitSection {
        remote,
        revision,
        specs,
    })
}

fn parse_platforms<'i>(i: &mut Input<'i>) -> Res<Vec<&'i str>> {
    "PLATFORMS\n".parse_next(i)?;
    repeat(1.., delimited(space1, parse_gem_name, line_ending)).parse_next(i)
}

fn parse_dependencies<'i>(i: &mut Input<'i>) -> Res<Vec<GemRange<'i>>> {
    "DEPENDENCIES\n".parse_next(i)?;
    repeat(0.., delimited(space1, parse_dependency, line_ending)).parse_next(i)
}

fn parse_checksums<'i>(i: &mut Input<'i>) -> Res<Vec<Checksum<'i>>> {
    "CHECKSUMS\n".parse_next(i)?;
    repeat(0.., delimited(space1, parse_checksum, line_ending)).parse_next(i)
}

fn parse_bundled_with<'i>(i: &mut Input<'i>) -> Res<&'i str> {
    "BUNDLED WITH".parse_next(i)?;
    space0.parse_next(i)?;
    "\n".parse_next(i)?;
    preceded(space1, parse_bundled_with_contents).parse_next(i)
}

fn parse_ruby_version<'i>(i: &mut Input<'i>) -> Res<&'i str> {
    "RUBY VERSION".parse_next(i)?;
    space0.parse_next(i)?;
    "\n".parse_next(i)?;
    preceded(space1, terminated(parse_ruby_version_contents, line_ending)).parse_next(i)
}

fn parse_gem<'i>(i: &mut Input<'i>) -> Res<GemSection<'i>> {
    "GEM\n".parse_next(i)?;
    let remote = delimited("  remote: ", parse_remote, line_ending).parse_next(i)?;
    "  specs:\n".parse_next(i)?;
    let specs = repeat(0.., parse_spec).parse_next(i)?;
    Ok(GemSection { remote, specs })
}

fn parse_path<'i>(i: &mut Input<'i>) -> Res<PathSection<'i>> {
    "PATH\n".parse_next(i)?;
    let remote = delimited("  remote: ", parse_remote, line_ending).parse_next(i)?;
    "  specs:\n".parse_next(i)?;
    let specs = repeat(0.., parse_spec).parse_next(i)?;
    Ok(PathSection { remote, specs })
}

fn parse_remote<'i>(i: &mut Input<'i>) -> Res<&'i str> {
    take_until(0.., '\n').parse_next(i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_gem() {
        let input = "\
GEM
  remote: https://rubygems.org/
  specs:
    erubi (1.13.1)
    netrc (0.11.0)
    parallel (1.26.3)
    prism (1.3.0)
    rbi (0.2.2)
      prism (~> 1.0)
      sorbet-runtime (>= 0.5.9204)
    sorbet (0.5.11725)
      sorbet-static (= 0.5.11725)
    sorbet-runtime (0.5.11725)
    sorbet-static (0.5.11725-aarch64-linux)
    sorbet-static (0.5.11725-universal-darwin)
    sorbet-static (0.5.11725-x86_64-linux)
    sorbet-static-and-runtime (0.5.11725)
      sorbet (= 0.5.11725)
      sorbet-runtime (= 0.5.11725)
    spoom (1.5.0)
      erubi (>= 1.10.0)
      prism (>= 0.28.0)
      sorbet-static-and-runtime (>= 0.5.10187)
      thor (>= 0.19.2)
    tapioca (0.16.6)
      bundler (>= 2.2.25)
      netrc (>= 0.11.0)
      parallel (>= 1.21.0)
      rbi (~> 0.2)
      sorbet-static-and-runtime (>= 0.5.11087)
      spoom (>= 1.2.0)
      thor (>= 1.2.0)
      yard-sorbet
    thor (1.4.0)
    yard (0.9.37)
    yard-sorbet (0.9.0)
      sorbet-runtime
      yard
";
        let mut input = LocatingSlice::new(input);
        let out = parse_gem.parse_next(&mut input).unwrap();
        assert_eq!(out.specs.len(), 16);
        assert_eq!(out.specs[15].deps.len(), 2);
        assert!(input.is_empty());
    }

    #[test]
    fn basic_spec_dep() {
        for input in [
            "      prism (~> 1.0)\n",
            "      sorbet-runtime\n",
            "      sorbet-runtime (>= 0.5.9204)\n",
        ] {
            let mut input = LocatingSlice::new(input);
            let out = parse_spec_dep.parse_next(&mut input).unwrap();
            println!("{out:#?}");
            println!("Remainder:");
            println!("{input}");
        }
    }

    #[test]
    fn test_ranges() {
        let input = " (>= 1.15.7, != 1.16.7, != 1.16.6, != 1.16.5, != 1.16.4, != 1.16.3, != 1.16.2, != 1.16.1, != 1.16.0.rc1, != 1.16.0)";
        let input = LocatingSlice::new(input);
        let out = spec_dep_semver.parse(input).unwrap();
        assert_eq!(out.len(), 10);
    }

    #[test]
    fn test_git_section() {
        for (test_num, input) in [
            "\
GIT
  remote: https://github.com/Driversnote-Dev/guard-erb_lint.git
  revision: 2ba3c5d21f5e891df97a3b7c03e56d7d19bf15a2
  specs:
    guard-erb_lint (1.0.0)
      activesupport
      erb_lint
      guard-compat (>= 1)
",
            "GIT
  remote: https://github.com/arthurnn/code-scanning-rubocop.git
  revision: 3077502361b66fd7e73b056a917649e40f87eb03
  specs:
    code-scanning-rubocop (0.6.1)
      rubocop (~> 1.0)
",
            "GIT
  remote: https://github.com/indirect/cloudflare.git
  revision: 82641303470f1de68d6b9ad25636e53e1e0325f9
  specs:
    cloudflare (4.4.0)
      async-rest (~> 0.18)
",
            "GIT
  remote: https://github.com/oldmoe/litestack.git
  revision: e598e1b1f0d46f45df1e2c6213ff9b136b63d9bf
  specs:
    litestack (0.4.5)
      erubi (~> 1)
      oj (~> 3)
      rack (~> 3)
      rackup (~> 2)
      nokogiri (>= 1.15.7, != 1.16.7, != 1.16.6, != 1.16.5, != 1.16.4, != 1.16.3, != 1.16.2, != 1.16.1, != 1.16.0.rc1, != 1.16.0)
      tilt (~> 2)
",
        ]
        .into_iter()
        .enumerate()
        {
            let i = LocatingSlice::new(input);
            let git_section = parse_git_section.parse(i).unwrap();
            if test_num == 3 {
                println!("{git_section:#?}");
            }
        }
    }

    #[test]
    fn test_parse_path() {
        let input = "\
PATH
  remote: pathgem
  specs:
    pathgem (0.1.0)
";
        let i = LocatingSlice::new(input);
        parse_path.parse(i).unwrap();
    }

    #[test]
    fn test_parse_section_header() {
        let input = "\
PATH
  remote: pathgem
  specs:
    pathgem (0.1.0)
";
        let mut i = LocatingSlice::new(input);
        let actual = parse_section_header.parse_next(&mut i).unwrap();
        assert_eq!(actual, "PATH");
    }

    #[test]
    fn test_parse_version() {
        for input in [
            "1",
            "1.0",
            "2.3a.4B",
            "1.0.0-beta",
            "1.2.3-beta.2-release-1",
            "1.0-rc.1",
        ] {
            let i = LocatingSlice::new(input);
            let _out = parse_version.parse(i).unwrap();
        }
    }
}
