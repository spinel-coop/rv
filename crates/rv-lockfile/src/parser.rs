use crate::{ParseError, ParseErrors, datatypes::*};
use miette::SourceSpan;
use winnow::{
    LocatingSlice, ModalParser, ModalResult, Parser,
    ascii::{line_ending, space0, space1},
    combinator::{alt, delimited, dispatch, opt, peek, preceded, repeat, separated, terminated},
    error::{ContextError, ErrMode},
    stream::{AsChar, Location, Stream},
    token::{take_until, take_while},
};

use rv_gem_types::requirement::{ComparisonOperator, Requirement, VersionConstraint};
use rv_gem_types::{Platform, ProjectDependency, VersionPlatform};
use rv_ruby::version::RubyVersion;
use rv_version::{Version, VersionSegment};

const GIT: &str = "GIT";
const GEM: &str = "GEM";
const PATH: &str = "PATH";
const PLATFORMS: &str = "PLATFORMS";
const DEPENDENCIES: &str = "DEPENDENCIES";
const CHECKSUMS: &str = "CHECKSUMS";
const RUBY_VERSION: &str = "RUBY VERSION";
const BUNDLED_WITH: &str = "BUNDLED WITH";

pub type Input<'a> = LocatingSlice<&'a str>;

type Res<T> = ModalResult<T, ContextError>;

#[derive(Debug)]
enum Section<'i> {
    Git(GitSection<'i>),
    Gem(GemSection<'i>),
    Path(PathSection<'i>),
    Platforms(Vec<Platform>),
    Dependencies(Vec<GemRange<'i>>),
    RubyVersion(RubyVersionSection),
    BundledWith(BundledWithSection),
    Checksums(Vec<Checksum<'i>>),
}

fn parse_section<'i>(i: &mut Input<'i>) -> Res<Section<'i>> {
    (dispatch! {peek(parse_section_header);
        GIT => paragraph(parse_git_section).map(Section::Git),
        GEM => paragraph(parse_gem).map(Section::Gem),
        PATH => paragraph(parse_path).map(Section::Path),
        PLATFORMS => paragraph(parse_platforms).map(Section::Platforms),
        DEPENDENCIES => paragraph(parse_dependencies).map(Section::Dependencies),
        CHECKSUMS => paragraph(parse_checksums).map(Section::Checksums),
        RUBY_VERSION => paragraph(parse_ruby_version).map(Section::RubyVersion),
        BUNDLED_WITH => paragraph(parse_bundled_with).map(Section::BundledWith),
        _ => winnow::combinator::fail::<_,Section,_>,
    })
    .parse_next(i)
}

pub fn parse<'i>(file: &'i str) -> Result<GemfileDotLock<'i>, ParseErrors> {
    let mut input = LocatingSlice::new(file);
    let i = &mut input;
    let mut parsed = GemfileDotLock::default();
    let mut error: Option<ParseErrors> = None;

    while !i.is_empty() {
        let section = match parse_section.parse_next(i) {
            Ok(sec) => sec,
            Err(e) => {
                // OK, there was an error. Let's figure out where, to highlight it.
                let byte_offset = i.previous_token_end();
                let char_offset = file[..byte_offset.min(file.len())].chars().count();

                // Then find the error message.
                let msg = match &e {
                    ErrMode::Incomplete(_) => "unexpected end of input".to_string(),
                    ErrMode::Backtrack(err) | ErrMode::Cut(err) => err.to_string(),
                };

                // Now we can add the error to the list.
                let parse_err = ParseError {
                    char_offset: SourceSpan::new(char_offset.into(), 1),
                    msg,
                };
                if let Some(err) = error.as_mut() {
                    err.others.push(parse_err);
                } else {
                    error = Some(ParseErrors {
                        lockfile_contents: file.to_owned(),
                        others: vec![parse_err],
                    })
                }

                // Consume input until the next new line which starts with a non-whitespace character.
                // If we reach the end of input, stop parsing.
                let remainder = *i.as_ref();
                if remainder.is_empty() {
                    break;
                }

                let mut skip_bytes = remainder.len();
                let mut found_boundary = false;
                let mut iter = remainder.char_indices().peekable();
                while let Some((idx, ch)) = iter.next() {
                    if ch == '\n' {
                        if let Some(&(_, next_ch)) = iter.peek() {
                            if !next_ch.is_whitespace() {
                                skip_bytes = idx + ch.len_utf8();
                                found_boundary = true;
                                break;
                            }
                        } else {
                            skip_bytes = remainder.len();
                            found_boundary = true;
                            break;
                        }
                    }
                }

                if skip_bytes == 0 {
                    break;
                }

                i.next_slice(skip_bytes);
                if !found_boundary || i.is_empty() {
                    break;
                }
                continue;
            }
        };
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

    match error {
        None => Ok(parsed),
        Some(error) => Err(error),
    }
}

/// Parse a paragraph, i.e. something ending in a new line.
fn paragraph<'i, O, F>(parser: F) -> impl ModalParser<Input<'i>, O, ContextError>
where
    F: Parser<Input<'i>, O, ErrMode<ContextError>>,
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
    let version_platform = delimited('(', parse_version_platform, ")\n").parse_next(i)?;
    let gem_version = GemVersion {
        name,
        version_platform,
    };
    let deps = repeat(0.., parse_spec_dep).parse_next(i)?;
    Ok(Spec { gem_version, deps })
}

fn parse_spec_dep<'i>(i: &mut Input<'i>) -> Res<ProjectDependency> {
    "      ".parse_next(i)?;
    let name = parse_gem_name.parse_next(i)?.to_string();
    let requirement = parse_requirement.parse_next(i)?;
    line_ending.parse_next(i)?;
    Ok(ProjectDependency { name, requirement })
}

fn parse_dependency<'i>(i: &mut Input<'i>) -> Res<GemRange<'i>> {
    let name = parse_gem_name.parse_next(i)?;
    let requirement = parse_requirement.parse_next(i)?;
    let nonstandard = opt('!').parse_next(i)?;
    Ok(GemRange {
        name,
        requirement,
        nonstandard: nonstandard.is_some(),
    })
}

fn parse_requirement<'i>(i: &mut Input<'i>) -> Res<Requirement> {
    let requirement = opt(spec_dep_semver)
        .parse_next(i)?
        .map_or(Requirement::default(), Requirement::from);

    Ok(requirement)
}

fn spec_dep_semver<'i>(i: &mut Input<'i>) -> Res<Vec<VersionConstraint>> {
    space1.parse_next(i)?;
    '('.parse_next(i)?;
    let out = separated(1.., parse_version_constraint, terminated(',', space0)).parse_next(i)?;
    ')'.parse_next(i)?;
    Ok(out)
}

fn parse_version_constraint<'i>(i: &mut Input<'i>) -> Res<VersionConstraint> {
    let operator = parse_operator.parse_next(i)?;
    space1.parse_next(i)?;
    let version = parse_version.parse_next(i)?;
    Ok(VersionConstraint { operator, version })
}

fn parse_operator<'i>(i: &mut Input<'i>) -> Res<ComparisonOperator> {
    // Order matters here somewhat,
    // e.g. must parse >= before > otherwise >= would never get parsed,
    // because > is a substring of >=.
    alt((
        "!=".map(|_| ComparisonOperator::NotEqual),
        ">=".map(|_| ComparisonOperator::GreaterThanOrEqual),
        "<=".map(|_| ComparisonOperator::LessThanOrEqual),
        ">".map(|_| ComparisonOperator::GreaterThan),
        "<".map(|_| ComparisonOperator::LessThan),
        "~>".map(|_| ComparisonOperator::Pessimistic),
        "=".map(|_| ComparisonOperator::Equal),
    ))
    .parse_next(i)
}

fn parse_gem_name<'i>(i: &mut Input<'i>) -> Res<&'i str> {
    take_while(0.., |c: char| {
        c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.'
    })
    .parse_next(i)
}

fn parse_platform<'i>(i: &mut Input<'i>) -> Res<Platform> {
    let cpu = take_while(1.., |c: char| c.is_ascii_alphanumeric() || c == '_').parse_next(i)?;

    let os = opt(preceded(
        '-',
        take_while(1.., |c: char| {
            c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-'
        }),
    ))
    .parse_next(i)?;

    let platform = Platform::from_lockfile(cpu, os);

    Ok(platform)
}

fn parse_ruby_version_inner<'i>(i: &mut Input<'i>) -> Res<RubyVersion> {
    let engine = take_while(1.., |c: char| c.is_ascii_alphabetic()).parse_next(i)?;

    ' '.parse_next(i)?;
    let major = parse_num.parse_next(i)?;

    '.'.parse_next(i)?;
    let minor = parse_num.parse_next(i)?;

    '.'.parse_next(i)?;
    let patch = parse_num.parse_next(i)?;

    let tiny = opt(preceded('.', parse_num)).parse_next(i)?;

    let patchlevel = opt(preceded('p', parse_num)).parse_next(i)?;

    let prerelease = opt(preceded(
        '.',
        take_while(1.., |c: char| c.is_ascii_alphanumeric()),
    ))
    .parse_next(i)?;

    Ok(RubyVersion {
        engine: engine.into(),
        major,
        minor,
        patch,
        patchlevel,
        tiny,
        prerelease: prerelease.map(|p| p.to_string()),
    })
}

fn parse_version<'i>(i: &mut Input<'i>) -> Res<Version> {
    let segments = peek(parse_segments).parse_next(i)?;
    let version = take_while(1.., |c: char| c.is_alphanumeric() || c == '.')
        .parse_next(i)?
        .to_string();

    Ok(Version { version, segments })
}

fn parse_segments<'i>(i: &mut Input<'i>) -> Res<Vec<VersionSegment>> {
    // [0-9]+
    let major = parse_num.parse_next(i)?;
    let mut segments = vec![VersionSegment::Number(major)];

    // (?>\.[0-9a-zA-Z]+)*
    let other_segments: Vec<_> = repeat(0.., preceded('.', parse_alphanum)).parse_next(i)?;
    segments.extend(other_segments.iter().map(|s| VersionSegment::new(s)));

    Ok(segments)
}

fn parse_version_platform<'i>(i: &mut Input<'i>) -> Res<VersionPlatform> {
    let version = parse_version.parse_next(i)?;
    let platform = opt(preceded('-', parse_platform))
        .parse_next(i)?
        .unwrap_or_default();

    Ok(VersionPlatform { version, platform })
}

fn parse_hex_string<'i>(i: &mut Input<'i>) -> Res<&'i str> {
    take_while(1.., |c: char| c.is_hex_digit()).parse_next(i)
}

fn parse_bool<'i>(i: &mut Input<'i>) -> Res<bool> {
    alt(("true".map(|_| true), "false".map(|_| false))).parse_next(i)
}

fn parse_checksum<'i>(i: &mut Input<'i>) -> Res<Checksum<'i>> {
    // nokogiri (1.18.10-arm-linux-gnu) sha256=51f4f25ab5d5ba1012d6b16aad96b840a10b067b93f35af6a55a2c104a7ee322
    // rack (3.2.3)
    let name = parse_gem_name.parse_next(i)?;
    space1.parse_next(i)?;
    let version_platform = delimited('(', parse_version_platform, ')').parse_next(i)?;
    let value = opt((space1, "sha256=")).parse_next(i)?;
    if value.is_some() {
        let sha256 = parse_hex_string.try_map(hex::decode).parse_next(i)?;
        Ok(Checksum {
            gem_version: GemVersion {
                name,
                version_platform,
            },
            value: sha256,
            algorithm: ChecksumAlgorithm::SHA256,
        })
    } else {
        Ok(Checksum {
            gem_version: GemVersion {
                name,
                version_platform,
            },
            value: vec![],
            algorithm: ChecksumAlgorithm::None,
        })
    }
}

fn parse_num(i: &mut Input<'_>) -> Res<u32> {
    take_while(1.., |c: char| c.is_ascii_digit())
        .try_map(|digits: &str| digits.parse::<u32>())
        .parse_next(i)
}

fn parse_alphanum<'i>(i: &mut Input<'i>) -> Res<&'i str> {
    take_while(1.., |c: char| c.is_alphanumeric()).parse_next(i)
}

fn parse_git_section<'i>(i: &mut Input<'i>) -> Res<GitSection<'i>> {
    "GIT\n".parse_next(i)?;
    let remote = delimited("  remote: ", parse_remote, line_ending).parse_next(i)?;
    let revision = delimited("  revision: ", parse_hex_string, line_ending).parse_next(i)?;
    let branch = opt(delimited(
        "  branch: ",
        take_while(1.., |c: char| {
            c.is_alphanumeric() || c == '.' || c == '-' || c == '_' || c == '/'
        }),
        line_ending,
    ))
    .parse_next(i)?;
    let git_ref = opt(delimited(
        "  ref: ",
        take_while(1.., |c: char| {
            c.is_alphanumeric() || c == '.' || c == '-' || c == '_' || c == '/'
        }),
        line_ending,
    ))
    .parse_next(i)?;
    let tag = opt(delimited(
        "  tag: ",
        take_while(1.., |c: char| {
            c.is_alphanumeric() || c == '.' || c == '-' || c == '_' || c == '/'
        }),
        line_ending,
    ))
    .parse_next(i)?;
    let submodules = opt(delimited("  submodules: ", parse_bool, line_ending)).parse_next(i)?;
    let glob = opt(delimited(
        "  glob: ",
        take_while(1.., |c: char| {
            c.is_alphanumeric()
                || c == '.'
                || c == '-'
                || c == '_'
                || c == '/'
                || c == '*'
                || c == '?'
                || c == '['
                || c == ']'
                || c == '^'
                || c == '\\'
                || c == '{'
                || c == '}'
                || c == ','
        }),
        line_ending,
    ))
    .parse_next(i)?;
    "  specs:\n".parse_next(i)?;
    let specs = repeat(0.., parse_spec).parse_next(i)?;
    Ok(GitSection {
        branch,
        git_ref,
        tag,
        remote,
        revision,
        submodules,
        glob,
        specs,
    })
}

fn parse_platforms<'i>(i: &mut Input<'i>) -> Res<Vec<Platform>> {
    "PLATFORMS\n".parse_next(i)?;
    repeat(1.., delimited(space1, parse_platform, line_ending)).parse_next(i)
}

fn parse_dependencies<'i>(i: &mut Input<'i>) -> Res<Vec<GemRange<'i>>> {
    "DEPENDENCIES\n".parse_next(i)?;
    repeat(0.., delimited(space1, parse_dependency, line_ending)).parse_next(i)
}

fn parse_checksums<'i>(i: &mut Input<'i>) -> Res<Vec<Checksum<'i>>> {
    "CHECKSUMS\n".parse_next(i)?;
    repeat(0.., delimited(space1, parse_checksum, line_ending)).parse_next(i)
}

fn parse_bundled_with<'i>(i: &mut Input<'i>) -> Res<BundledWithSection> {
    "BUNDLED WITH".parse_next(i)?;
    space0.parse_next(i)?;
    line_ending.parse_next(i)?;
    "  ".parse_next(i)?;
    let third_space = opt(' ').parse_next(i)?;
    let bundler_version = parse_version.parse_next(i)?;
    let indentation = match third_space {
        None => LockfileIndentation::Standard,
        Some(_) => LockfileIndentation::ThreeSpaces,
    };
    Ok(BundledWithSection {
        indentation,
        bundler_version,
    })
}

fn parse_ruby_version<'i>(i: &mut Input<'i>) -> Res<RubyVersionSection> {
    "RUBY VERSION".parse_next(i)?;
    space0.parse_next(i)?;
    line_ending.parse_next(i)?;
    "  ".parse_next(i)?;
    let third_space = opt(' ').parse_next(i)?;
    let cruby_version = parse_ruby_version_inner.parse_next(i)?;
    let engine_version = opt(delimited(" (", parse_ruby_version_inner, ")\n")).parse_next(i)?;
    let indentation = match third_space {
        None => LockfileIndentation::Standard,
        Some(_) => LockfileIndentation::ThreeSpaces,
    };
    Ok(RubyVersionSection {
        indentation,
        cruby_version,
        engine_version,
    })
}

fn parse_gem<'i>(i: &mut Input<'i>) -> Res<GemSection<'i>> {
    "GEM\n".parse_next(i)?;
    let remote = opt(delimited("  remote: ", parse_remote, line_ending)).parse_next(i)?;
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
            let original_input = input;
            let mut input = LocatingSlice::new(input);
            let out = parse_spec_dep.parse_next(&mut input).unwrap();
            assert_eq!(original_input.trim(), out.to_lock_name());
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
        for input in [
            "GIT
  remote: git://github.com/libgit2/rugged.git
  revision: 34a492ec7c5165824f39d8027d73712b0346aac2
  submodules: true
  specs:
    rugged (0.26.0)
",
            "GIT
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
        ] {
            let i = LocatingSlice::new(input);
            let git_section = parse_git_section.parse(i).unwrap();
            assert_eq!(input, git_section.to_string());
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
    fn test_parse_ruby_version_inner() {
        let input = "\
  ruby 3.3.1p55
";
        let mut i = LocatingSlice::new(input);
        let version = parse_ruby_version_inner.parse_next(&mut i).unwrap();
        assert_eq!(version.major, 3);
        assert_eq!(version.minor, 3);
        assert_eq!(version.patch, 1);
        assert_eq!(version.patchlevel, Some(55));
        assert_eq!(version.prerelease, None);
    }

    #[test]
    fn test_parse_ruby_version_inner_without_patchlevel() {
        let input = "\
  ruby 4.0.0
";
        let mut i = LocatingSlice::new(input);
        let version = parse_ruby_version_inner.parse_next(&mut i).unwrap();
        assert_eq!(version.major, 4);
        assert_eq!(version.minor, 0);
        assert_eq!(version.patch, 0);
        assert_eq!(version.patchlevel, None);
        assert_eq!(version.prerelease, None);
    }

    #[test]
    fn test_parse_ruby_version_inner_with_p0() {
        let input = "\
  ruby 3.2.0p0
";
        let mut i = LocatingSlice::new(input);
        let version = parse_ruby_version_inner.parse_next(&mut i).unwrap();
        assert_eq!(version.major, 3);
        assert_eq!(version.minor, 2);
        assert_eq!(version.patch, 0);
        assert_eq!(version.patchlevel, Some(0));
        assert_eq!(version.prerelease, None);
    }

    #[test]
    fn test_parse_ruby_version_inner_preserves_preview() {
        // Real format from GitHub: "ruby 3.3.0.preview2" (dot, not dash)
        // https://github.com/akitaonrails/rinhabackend-rails-api/blob/main/Gemfile.lock
        let input = "\
  ruby 3.3.0.preview2
";
        let mut i = LocatingSlice::new(input);
        let version = parse_ruby_version_inner.parse_next(&mut i).unwrap();
        assert_eq!(version.major, 3);
        assert_eq!(version.minor, 3);
        assert_eq!(version.patch, 0);
        assert_eq!(version.patchlevel, None);
        assert_eq!(version.prerelease, Some("preview2".to_string()));
    }

    #[test]
    fn test_parse_ruby_version_inner_preserves_rc() {
        // Real format from GitHub: "ruby 3.3.0.rc1" (dot, not dash)
        // https://github.com/pbstriker38/is_ruby_dead/blob/main/Gemfile.lock
        let input = "\
  ruby 3.3.0.rc1
";
        let mut i = LocatingSlice::new(input);
        let version = parse_ruby_version_inner.parse_next(&mut i).unwrap();
        assert_eq!(version.major, 3);
        assert_eq!(version.minor, 3);
        assert_eq!(version.patch, 0);
        assert_eq!(version.patchlevel, None);
        assert_eq!(version.prerelease, Some("rc1".to_string()));
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
            "1.0.0.pre.beta",
            "1.2.3.pre.beta.2.pre.release.pre.1",
            "1.0.pre.rc.1",
        ] {
            let i = LocatingSlice::new(input);
            let result = parse_version.parse(i);
            assert!(result.is_ok(), "{:?}", result);
        }
    }
}
