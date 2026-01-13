use std::collections::HashMap;

use pubgrub::{OfflineDependencyProvider, Ranges, SelectedDependencies};
use rv_lockfile::datatypes::SemverConstraint;

use super::{
    GemName,
    gem_version::GemVersion,
    gemserver::{VersionAvailable, VersionConstraints},
};

pub fn solve(
    gem: GemName,
    version: GemVersion,
    gem_info: HashMap<GemName, Vec<VersionAvailable>>,
) -> Result<SelectedDependencies<DepProvider>, pubgrub::PubGrubError<DepProvider>> {
    let provider = all_dependencies(gem_info);
    pubgrub::resolve(&provider, gem, version)
}

type DepProvider = OfflineDependencyProvider<GemName, Ranges<GemVersion>>;

/// Build a PubGrub "dependency provider", i.e. something that can be queried
/// with all the information of a GemServer (which gems are available, what versions that gem has,
/// and what dependencies that gem-version pair has).
/// This is really just taking the `gem_info` hashmap and organizing it in a way that PubGrub can understand.
fn all_dependencies(gem_info: HashMap<GemName, Vec<VersionAvailable>>) -> DepProvider {
    let mut m: OfflineDependencyProvider<GemName, Ranges<GemVersion>> =
        OfflineDependencyProvider::new();
    for (package, versions_available) in gem_info {
        for version_available in versions_available {
            m.add_dependencies(
                package.clone(),
                version_available.version,
                version_available
                    .deps
                    .into_iter()
                    .map(|dep| (dep.gem_name, dep.version_constraints.into())),
            );
        }
    }
    m
}

impl From<VersionConstraints> for Ranges<GemVersion> {
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
                // if >1.0, use the given numbers as the floor and the next major as not allowed.
                SemverConstraint::Pessimistic if v.major >= 1 => vec![
                    // Given version as the floor
                    Ranges::higher_than(v.clone()),
                    // Next major as not allowed.
                    Ranges::strictly_lower_than(v.next_major()),
                ],
                // if <1.0, use the given number as the floor and the next minor as not allowed.
                SemverConstraint::Pessimistic => vec![
                    // Given version as the floor
                    Ranges::higher_than(v.clone()),
                    // Next minor as not allowed.
                    Ranges::strictly_lower_than(v.next_minor()),
                ],
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
    use crate::commands::tool::install::{
        gemserver::VersionConstraint, transitive_dep_query::CachedGemDeps,
    };
    use std::str::FromStr;

    /// Tests that the conversion from RubyGems requirements to PubGrub ranges is correct.
    #[test]
    fn test_mapping() {
        struct Test {
            input: Vec<VersionConstraint>,
            expected: Ranges<GemVersion>,
        }
        for Test { input, expected } in vec![
            // ~> 0.4.3 allows >=0.4.3, <0.5
            Test {
                input: vec![VersionConstraint {
                    constraint_type: SemverConstraint::Pessimistic,
                    version: "0.4.3".parse().unwrap(),
                }],
                expected: Ranges::intersection(
                    &Ranges::higher_than(GemVersion::from_str("0.4.3").unwrap()),
                    &Ranges::strictly_lower_than(GemVersion::from_str("0.5").unwrap()),
                ),
            },
            // ~> 2.1.5 allows >=2.1.5, <3.0.0
            Test {
                input: vec![VersionConstraint {
                    constraint_type: SemverConstraint::Pessimistic,
                    version: "2.1.5".parse().unwrap(),
                }],
                expected: Ranges::intersection(
                    &Ranges::higher_than(GemVersion::from_str("2.1.5").unwrap()),
                    &Ranges::strictly_lower_than(GemVersion::from_str("3").unwrap()),
                ),
            },
        ] {
            // Take the Ruby gem version requirements,
            // translate them to PubGrub version ranges.
            let actual = Ranges::from(VersionConstraints::from(input));
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn test_resolution() {
        let data: CachedGemDeps = serde_json::from_str(include_str!(
            "../../../../testdata/all_rails_transitive_deps.json"
        ))
        .unwrap();
        let gem_info = data.gems_to_deps;
        let mut out: Vec<_> = solve("rails".to_owned(), "8.1.1".parse().unwrap(), gem_info)
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
