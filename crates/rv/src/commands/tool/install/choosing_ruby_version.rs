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

    if let Ok(local_ruby) = select_ruby_version_for(&installed_rubies, ruby_constraints, false) {
        return Ok(local_ruby);
    }

    let remote_rubies = &config.remote_rubies().await;
    select_ruby_version_for(remote_rubies, ruby_constraints, false)
        .or_else(|_| select_ruby_version_for(remote_rubies, ruby_constraints, true))
}

/// Find the highest Ruby version that meets these constraints from the available choices.
fn select_ruby_version_for(
    candidate_rubies: &[rv_ruby::Ruby],
    ruby_constraints: &Requirement,
    match_prereleases: bool,
) -> std::result::Result<RubyVersion, Error> {
    // Otherwise, we'll use the latest Ruby the gem allows.
    for candidate_ruby in candidate_rubies.iter().rev() {
        let version = &candidate_ruby.version;

        if ruby_constraints.matches(&rv_version::Version::from(version), match_prereleases) {
            return Ok(version.clone());
        }
    }
    Err(Error::NoMatchingRuby {
        requirement: ruby_constraints.clone(),
    })
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
        let selected_ruby = select_ruby_version_for(&rubies, &requirement, false).unwrap();

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
        let match_prereleases = true;
        let selected_ruby =
            select_ruby_version_for(&rubies, &requirement, match_prereleases).unwrap();
        assert_eq!(expected, selected_ruby);

        let expected = RubyVersion::from_str("ruby-3.4.8").unwrap();
        let match_prereleases = false;
        let selected_ruby =
            select_ruby_version_for(&rubies, &requirement, match_prereleases).unwrap();
        assert_eq!(expected, selected_ruby);
    }
}
