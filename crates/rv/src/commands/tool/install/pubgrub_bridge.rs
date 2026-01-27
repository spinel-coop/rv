use std::collections::HashMap;

use pubgrub::{OfflineDependencyProvider, Ranges, SelectedDependencies};
use rv_lockfile::datatypes::SemverConstraint;
use rv_version::Version;

use super::gemserver::{GemRelease, VersionConstraints};

pub fn solve(
    gem: String,
    version: Version,
    gem_info: HashMap<String, Vec<GemRelease>>,
) -> Result<SelectedDependencies<DepProvider>, pubgrub::PubGrubError<DepProvider>> {
    let provider = all_dependencies(gem_info);
    pubgrub::resolve(&provider, gem, version)
}

pub type DepProvider = OfflineDependencyProvider<String, Ranges<Version>>;

/// Build a PubGrub "dependency provider", i.e. something that can be queried
/// with all the information of a GemServer (which gems are available, what versions that gem has,
/// and what dependencies that gem-version pair has).
/// This is really just taking the `gem_info` hashmap and organizing it in a way that PubGrub can understand.
fn all_dependencies(gem_info: HashMap<String, Vec<GemRelease>>) -> DepProvider {
    let mut m: OfflineDependencyProvider<String, Ranges<Version>> =
        OfflineDependencyProvider::new();
    for (package, gem_releases) in gem_info {
        for gem_release in gem_releases {
            m.add_dependencies(
                package.clone(),
                gem_release.version,
                gem_release
                    .deps
                    .into_iter()
                    .map(|dep| (dep.gem_name, dep.version_constraints.into())),
            );
        }
    }
    m
}

impl From<VersionConstraints> for Ranges<Version> {
    fn from(constraints: VersionConstraints) -> Self {
        // Convert the RubyGems constraints into PubGrub ranges.
        let ranges = constraints.inner.into_iter().flat_map(|constraint| {
            let v = constraint.version;
            match constraint.constraint_type {
                SemverConstraint::Exact => vec![
                    // It's just one range, this range.
                    Ranges::singleton(v),
                ],
                SemverConstraint::NotEqual => vec![
                    // Everything EXCEPT this one range.
                    Ranges::singleton(v).complement(),
                ],

                // These 4 are easy:
                SemverConstraint::GreaterThan => vec![Ranges::strictly_higher_than(v)],
                SemverConstraint::LessThan => vec![Ranges::strictly_lower_than(v)],
                SemverConstraint::GreaterThanOrEqual => vec![Ranges::higher_than(v)],
                SemverConstraint::LessThanOrEqual => vec![Ranges::lower_than(v)],
                // if <1.0, use the given number as the floor and the next minor as not allowed.
                SemverConstraint::Pessimistic => {
                    let (lower, upper) = v.pessimistic_range();
                    vec![
                        Ranges::higher_than(lower),
                        Ranges::strictly_lower_than(upper),
                    ]
                }
            }
        });

        // Now, join all those ranges together using &, because that's what multiple RubyGems
        // constraints are actually listed as.
        let mut overall_range = Ranges::full();
        for r in ranges {
            overall_range = overall_range.intersection(&r);
        }
        overall_range
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::tool::install::gemserver::VersionConstraint;
    use std::str::FromStr;

    /// Tests that the conversion from RubyGems requirements to PubGrub ranges is correct.
    #[test]
    fn test_mapping() {
        struct Test {
            input: Vec<VersionConstraint>,
            expected: Ranges<Version>,
        }
        #[expect(clippy::single_element_loop)] // Remove this 'expect' if you add another test.
        for Test { input, expected } in [Test {
            input: vec![VersionConstraint {
                constraint_type: SemverConstraint::Pessimistic,
                version: "3.0.3".parse().unwrap(),
            }],
            expected: Ranges::intersection(
                &Ranges::higher_than(Version::from_str("3.0.3").unwrap()),
                &Ranges::strictly_lower_than(Version::from_str("3.1").unwrap()),
            ),
        }] {
            // Take the Ruby gem version requirements,
            // translate them to PubGrub version ranges.
            let actual = Ranges::from(VersionConstraints::from(input));
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn test_resolution() {
        let gem_info: HashMap<String, Vec<GemRelease>> = serde_json::from_str(include_str!(
            "../../../../testdata/all_nokogiri_transitive_deps.json"
        ))
        .unwrap();
        let mut out: Vec<_> = solve("nokogiri".to_owned(), "1.19.0".parse().unwrap(), gem_info)
            .unwrap()
            .into_iter()
            .collect();
        out.sort();
        let mut resolved_rails = String::new();
        for (k, v) in out {
            resolved_rails.push_str(&format!("{k}: {v}\n"));
        }
        insta::assert_snapshot!(resolved_rails);
    }
}
