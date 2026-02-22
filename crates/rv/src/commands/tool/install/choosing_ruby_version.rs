use owo_colors::OwoColorize;
use rv_lockfile::datatypes::SemverConstraint;
use rv_ruby::version::ReleasedRubyVersion;
use rv_version::VersionError;

use crate::{
    commands::tool::install::{Error, Result, gemserver::VersionConstraint},
    config::Config,
};

/// Which ruby version should be used to run a tool?
/// This takes the tool's Ruby version constraints as a parameter.
pub async fn ruby_to_use_for(
    config: &Config,
    ruby_constraints: &[VersionConstraint],
) -> Result<ReleasedRubyVersion> {
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
    ruby_constraints: &[VersionConstraint],
    match_prereleases: MatchPrereleases,
) -> std::result::Result<ReleasedRubyVersion, Error> {
    // If the gem can be used with any Ruby version, then we'll use the latest available.
    if ruby_constraints.is_empty() {
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
        requirements: ruby_constraints.to_vec(),
    })
}

fn ruby_version_to_gem_version(
    ruby_version: &ReleasedRubyVersion,
) -> std::result::Result<rv_version::Version, VersionError> {
    // Convert the ruby version to a string,
    // but skip the engine. Do not put the engine into this string,
    // because it would get counted as a string Segment in the Gem Version.
    let mut s = String::new();
    s.push_str(&format!("{}", ruby_version.major));
    s.push_str(&format!(".{}", ruby_version.minor));
    s.push_str(&format!(".{}", ruby_version.patch));

    if let Some(tiny) = ruby_version.tiny {
        s.push_str(&format!(".{tiny}"));
    }
    if let Some(ref prerelease) = ruby_version.prerelease {
        s.push_str(&format!("-{prerelease}"));
    }

    // Parse the string into a gem version.
    s.parse()
}

/// Use pubgrub to check if this Ruby version satisfies these Ruby version constraints.
pub fn does_ruby_version_satisfy(
    ruby_version: &ReleasedRubyVersion,
    ruby_constraints: &[VersionConstraint],
) -> bool {
    let Ok(version) = ruby_version_to_gem_version(ruby_version) else {
        eprintln!(
            "{}: Ruby version {ruby_version} could not be evaluated because it doesn't match the RubyGems Gem::Version schema",
            "WARNING".yellow(),
        );
        return false;
    };
    // Check each constraint to see if it fails.
    for constraint in ruby_constraints {
        if !meets_constraint(version.clone(), constraint) {
            return false;
        }
    }
    true
}

fn meets_constraint(version: rv_version::Version, constraint: &VersionConstraint) -> bool {
    match constraint.constraint_type {
        SemverConstraint::Exact => version == constraint.version,
        SemverConstraint::NotEqual => version != constraint.version,
        SemverConstraint::GreaterThan => version > constraint.version,
        SemverConstraint::LessThan => version < constraint.version,
        SemverConstraint::GreaterThanOrEqual => version >= constraint.version,
        SemverConstraint::LessThanOrEqual => version <= constraint.version,
        SemverConstraint::Pessimistic => {
            let (low, high) = constraint.version.pessimistic_range();
            version >= low && version < high
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    use camino::Utf8PathBuf;
    use std::str::FromStr;

    #[test]
    fn test_eval() {
        let chosen = "4.0.1".parse().unwrap();
        let constraint = VersionConstraint {
            constraint_type: SemverConstraint::GreaterThanOrEqual,
            version: "3.2".parse().unwrap(),
        };
        assert!(meets_constraint(chosen, &constraint));
    }

    fn ruby(version: &str) -> rv_ruby::Ruby {
        let version = ReleasedRubyVersion::from_str(version).unwrap();
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
            constraint_type: SemverConstraint::LessThan,
            version: "3.4".parse().unwrap(),
        }];

        let rubies = vec![ruby("ruby-3.2.10"), ruby("ruby-3.3.10"), ruby("ruby-3.4.8")];

        let expected = ReleasedRubyVersion::from_str("ruby-3.3.10").unwrap();
        let selected_ruby =
            select_ruby_version_for(&rubies, &constraints, MatchPrereleases::No).unwrap();

        assert_eq!(expected, selected_ruby);
    }

    #[test]
    fn test_select_ruby_version_for_prereleases() {
        let constraints = vec![VersionConstraint {
            constraint_type: SemverConstraint::LessThan,
            version: "3.5".parse().unwrap(),
        }];

        let rubies = vec![
            ruby("ruby-3.2.10"),
            ruby("ruby-3.3.10"),
            ruby("ruby-3.4.8"),
            ruby("3.5.0-preview1"),
        ];

        let expected = ReleasedRubyVersion::from_str("ruby-3.5.0-preview1").unwrap();
        let match_prereleases = MatchPrereleases::Yes;
        let selected_ruby =
            select_ruby_version_for(&rubies, &constraints, match_prereleases).unwrap();
        assert_eq!(expected, selected_ruby);

        let expected = ReleasedRubyVersion::from_str("ruby-3.4.8").unwrap();
        let match_prereleases = MatchPrereleases::No;
        let selected_ruby =
            select_ruby_version_for(&rubies, &constraints, match_prereleases).unwrap();
        assert_eq!(expected, selected_ruby);
    }
}
