use owo_colors::OwoColorize;
use rv_gem_types::requirement::Requirement;
use rv_ruby::version::RubyVersion;

use crate::{
    commands::tool::install::{Error, Result},
    config::Config,
};

/// Which ruby version should be used to run a tool?
/// This takes the tool's Ruby version constraints as a parameter.
pub async fn ruby_to_use_for(
    config: &Config<'_>,
    ruby_constraints: &Requirement,
) -> Result<RubyVersion> {
    let installed_rubies = config.rubies();

    if let Ok(local_ruby) =
        select_ruby_version_for(&installed_rubies, ruby_constraints, MatchPrereleases::No)
    {
        return Ok(local_ruby);
    }

    let remote_rubies = &config.remote_rubies().await;
    select_ruby_version_for(remote_rubies, ruby_constraints, MatchPrereleases::No).or_else(|_| {
        select_ruby_version_for(remote_rubies, ruby_constraints, MatchPrereleases::Yes)
    })
}

#[derive(Debug, Eq, PartialEq)]
enum MatchPrereleases {
    Yes,
    No,
}

/// Find the highest Ruby version that meets these constraints from the available choices.
fn select_ruby_version_for(
    candidate_rubies: &[rv_ruby::Ruby],
    ruby_constraints: &Requirement,
    match_prereleases: MatchPrereleases,
) -> std::result::Result<RubyVersion, Error> {
    // If the gem can be used with any Ruby version, then we'll use the latest available.
    if ruby_constraints.is_latest_version() {
        let chosen = candidate_rubies
            .iter()
            .map(|r| r.version.clone())
            .max()
            .ok_or(Error::NoRubies)?;
        return Ok(chosen);
    }

    // Otherwise, we'll use the latest Ruby the gem allows.
    for candidate_ruby in candidate_rubies.iter().rev() {
        let version = &candidate_ruby.version;

        if version.is_prerelease() && match_prereleases == MatchPrereleases::No {
            continue;
        }

        if does_ruby_version_satisfy(&candidate_ruby.version, ruby_constraints) {
            return Ok(candidate_ruby.version.clone());
        }
    }
    Err(Error::NoMatchingRuby {
        requirement: ruby_constraints.clone(),
    })
}

/// Use pubgrub to check if this Ruby version satisfies these Ruby version constraints.
pub fn does_ruby_version_satisfy(
    ruby_version: &RubyVersion,
    ruby_constraints: &Requirement,
) -> bool {
    let Ok(version) = ruby_version.number().parse::<rv_version::Version>() else {
        eprintln!(
            "{}: Ruby version {ruby_version} could not be evaluated because it doesn't match the RubyGems Gem::Version schema",
            "WARNING".yellow(),
        );
        return false;
    };
    ruby_constraints.satisfied_by(&version)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rv_gem_types::{ComparisonOperator, VersionConstraint};

    use camino::Utf8PathBuf;
    use std::str::FromStr;

    fn ruby(version: &str) -> rv_ruby::Ruby {
        let version = RubyVersion::from_str(version).unwrap();
        let version_str = version.to_string();
        rv_ruby::Ruby {
            key: format!("{version_str}-macos-aarch64"),
            version,
            path: Utf8PathBuf::from(format!(
                "https://github.com/spinel-coop/rv-ruby/releases/download/latest/{version_str}.arm64_linux.tar.gz"
            )),
            managed: false,
            symlink: None,
            arch: "aarch64".into(),
            os: "macos".into(),
            gem_root: None,
        }
    }

    #[test]
    fn test_select_ruby_version_for() {
        let constraints = vec![VersionConstraint {
            operator: ComparisonOperator::LessThan,
            version: "3.4".parse().unwrap(),
        }];
        let requirement: Requirement = constraints.into();

        let rubies = vec![ruby("ruby-3.2.10"), ruby("ruby-3.3.10"), ruby("ruby-3.4.8")];

        let expected = RubyVersion::from_str("ruby-3.3.10").unwrap();
        let selected_ruby =
            select_ruby_version_for(&rubies, &requirement, MatchPrereleases::No).unwrap();

        assert_eq!(expected, selected_ruby);
    }

    #[test]
    fn test_select_ruby_version_for_prereleases() {
        let constraints = vec![VersionConstraint {
            operator: ComparisonOperator::LessThan,
            version: "3.5".parse().unwrap(),
        }];
        let requirement: Requirement = constraints.into();

        let rubies = vec![
            ruby("ruby-3.2.10"),
            ruby("ruby-3.3.10"),
            ruby("ruby-3.4.8"),
            ruby("3.5.0-preview1"),
        ];

        let expected = RubyVersion::from_str("ruby-3.5.0-preview1").unwrap();
        let match_prereleases = MatchPrereleases::Yes;
        let selected_ruby =
            select_ruby_version_for(&rubies, &requirement, match_prereleases).unwrap();
        assert_eq!(expected, selected_ruby);

        let expected = RubyVersion::from_str("ruby-3.4.8").unwrap();
        let match_prereleases = MatchPrereleases::No;
        let selected_ruby =
            select_ruby_version_for(&rubies, &requirement, match_prereleases).unwrap();
        assert_eq!(expected, selected_ruby);
    }
}
