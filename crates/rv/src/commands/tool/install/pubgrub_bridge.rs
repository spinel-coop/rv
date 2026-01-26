use std::collections::HashMap;

use pubgrub::{OfflineDependencyProvider, Ranges, SelectedDependencies};
use rv_gem_types::{Platform, Version};
use rv_lockfile::datatypes::SemverConstraint;

use super::gemserver::{GemRelease, VersionConstraint, VersionConstraints};

pub fn solve(
    gem: String,
    release: GemRelease,
    gem_info: HashMap<String, Vec<GemRelease>>,
) -> Result<SelectedDependencies<DepProvider>, pubgrub::PubGrubError<DepProvider>> {
    let provider = all_dependencies(gem_info);
    pubgrub::resolve(&provider, gem, release)
}

pub type DepProvider = OfflineDependencyProvider<String, Ranges<GemRelease>>;

/// Build a PubGrub "dependency provider", i.e. something that can be queried
/// with all the information of a GemServer (which gems are available, what versions that gem has,
/// and what dependencies that gem-version pair has).
/// This is really just taking the `gem_info` hashmap and organizing it in a way that PubGrub can understand.
fn all_dependencies(gem_info: HashMap<String, Vec<GemRelease>>) -> DepProvider {
    let mut m: OfflineDependencyProvider<String, Ranges<GemRelease>> =
        OfflineDependencyProvider::new();
    for (package, gem_releases) in gem_info {
        for gem_release in gem_releases {
            m.add_dependencies(
                package.clone(),
                gem_release.clone(),
                gem_release
                    .deps
                    .into_iter()
                    .map(|dep| (dep.gem_name, dep.version_constraints.into())),
            );
        }
    }
    m
}

impl From<VersionConstraint> for Ranges<GemRelease> {
    fn from(constraint: VersionConstraint) -> Self {
        let v = constraint.version;
        let min_v = GemRelease {
            version: v.clone(),
            platform: Platform::Ruby,
            ..GemRelease::default()
        };

        let max_v = GemRelease {
            version: v.clone(),
            platform: Platform::Current,
            ..GemRelease::default()
        };

        match constraint.constraint_type {
            SemverConstraint::Exact => {
                Ranges::intersection(&Ranges::higher_than(min_v), &Ranges::lower_than(max_v))
            }
            SemverConstraint::NotEqual => Ranges::union(
                &Ranges::strictly_lower_than(min_v),
                &Ranges::strictly_higher_than(max_v),
            ),

            // These 4 are easy:
            SemverConstraint::GreaterThan => Ranges::strictly_higher_than(max_v),
            SemverConstraint::LessThan => Ranges::strictly_lower_than(min_v),
            SemverConstraint::GreaterThanOrEqual => Ranges::higher_than(min_v),
            SemverConstraint::LessThanOrEqual => Ranges::lower_than(max_v),
            // if <1.0, use the given number as the floor and the next minor as not allowed.
            SemverConstraint::Pessimistic => {
                let bump_v = GemRelease {
                    version: Version::new(format!("{}.A", v.bump())).unwrap(),
                    platform: Platform::Ruby,
                    ..GemRelease::default()
                };

                Ranges::intersection(
                    &Ranges::higher_than(min_v),
                    &Ranges::strictly_lower_than(bump_v),
                )
            }
        }
    }
}

impl From<VersionConstraints> for Ranges<GemRelease> {
    fn from(constraints: VersionConstraints) -> Self {
        // Convert the RubyGems constraints into PubGrub ranges.
        let ranges = constraints.inner.into_iter().map(Ranges::from);

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

    fn r(input: &str) -> GemRelease {
        let (version, platform) = rv_gem_types::platform::version_platform_split(input).unwrap();

        GemRelease {
            version,
            platform,
            ..GemRelease::default()
        }
    }

    /// Tests that the conversion from RubyGems requirements to PubGrub ranges is correct.
    #[test]
    fn test_mapping() {
        struct Test {
            input: Vec<VersionConstraint>,
            expected: Ranges<GemRelease>,
        }
        #[expect(clippy::single_element_loop)] // Remove this 'expect' if you add another test.
        for Test { input, expected } in [Test {
            input: vec![VersionConstraint {
                constraint_type: SemverConstraint::Pessimistic,
                version: "3.0.3".parse().unwrap(),
            }],
            expected: Ranges::intersection(
                &Ranges::higher_than(r("3.0.3")),
                &Ranges::strictly_lower_than(r("3.1.A")),
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
        let mut out: Vec<_> = solve("nokogiri".to_owned(), r("1.19.0-arm64-darwin"), gem_info)
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
