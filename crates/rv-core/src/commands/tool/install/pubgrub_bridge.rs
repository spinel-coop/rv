use std::collections::HashMap;

use pubgrub::{OfflineDependencyProvider, Ranges, SelectedDependencies};
use rv_gem_types::{Platform, Version, VersionPlatform};
use rv_lockfile::datatypes::SemverConstraint;

use super::{
    GemName,
    gemserver::{GemRelease, VersionConstraint, VersionConstraints},
};

pub fn solve(
    gem: GemName,
    release: GemRelease,
    gem_info: HashMap<GemName, Vec<GemRelease>>,
) -> Result<SelectedDependencies<DepProvider>, pubgrub::PubGrubError<DepProvider>> {
    let provider = all_dependencies(gem_info);
    pubgrub::resolve(&provider, gem, release.version_platform)
}

pub type DepProvider = OfflineDependencyProvider<GemName, Ranges<VersionPlatform>>;

/// Build a PubGrub "dependency provider", i.e. something that can be queried
/// with all the information of a GemServer (which gems are available, what versions that gem has,
/// and what dependencies that gem-version pair has).
/// This is really just taking the `gem_info` hashmap and organizing it in a way that PubGrub can understand.
fn all_dependencies(gem_info: HashMap<GemName, Vec<GemRelease>>) -> DepProvider {
    let mut m: OfflineDependencyProvider<GemName, Ranges<VersionPlatform>> =
        OfflineDependencyProvider::new();
    for (package, gem_releases) in gem_info {
        for gem_release in gem_releases {
            m.add_dependencies(
                package.clone(),
                gem_release.version_platform,
                gem_release
                    .deps
                    .into_iter()
                    .map(|dep| (dep.gem_name, dep.version_constraints.into())),
            );
        }
    }
    m
}

impl From<VersionConstraint> for Ranges<VersionPlatform> {
    fn from(constraint: VersionConstraint) -> Self {
        let v = constraint.version;
        let min_v = VersionPlatform {
            version: v.clone(),
            platform: Platform::Ruby,
        };

        let max_v = VersionPlatform {
            version: v.clone(),
            platform: Platform::Current,
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
            // This one is weird, but at least it's encapsulated into a `bump` method.
            SemverConstraint::Pessimistic => {
                let bump_v = VersionPlatform {
                    version: Version::new(format!("{}.A", v.bump())).unwrap(),
                    platform: Platform::Ruby,
                };

                Ranges::intersection(
                    &Ranges::higher_than(min_v),
                    &Ranges::strictly_lower_than(bump_v),
                )
            }
        }
    }
}

impl From<VersionConstraints> for Ranges<VersionPlatform> {
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
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::commands::tool::install::gemserver::VersionConstraint;
    use std::str::FromStr;

    fn vp(input: &str) -> VersionPlatform {
        VersionPlatform::from_str(input).unwrap()
    }

    fn r(input: &str) -> GemRelease {
        GemRelease {
            version_platform: VersionPlatform::from_str(input).unwrap(),
            deps: Default::default(),
            metadata: Default::default(),
        }
    }

    /// Tests that the conversion from RubyGems requirements to PubGrub ranges is correct.
    #[test]
    fn test_mapping() {
        struct Test {
            input: Vec<VersionConstraint>,
            expected: Ranges<VersionPlatform>,
        }
        #[expect(clippy::single_element_loop)] // Remove this 'expect' if you add another test.
        for Test { input, expected } in [Test {
            input: vec![VersionConstraint {
                constraint_type: SemverConstraint::Pessimistic,
                version: "3.0.3".parse().unwrap(),
            }],
            expected: Ranges::intersection(
                &Ranges::higher_than(vp("3.0.3")),
                &Ranges::strictly_lower_than(vp("3.1.A")),
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
        /// All the information about a release of a gem available on some Gemserver.
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        pub struct PrevGemRelease {
            pub version: Version,
            pub platform: Platform,
            pub deps: Vec<crate::commands::tool::install::gemserver::Dep>,
            pub metadata: crate::commands::tool::install::gemserver::Metadata,
        }

        impl From<PrevGemRelease> for GemRelease {
            fn from(prev: PrevGemRelease) -> Self {
                let PrevGemRelease {
                    version,
                    platform,
                    deps,
                    metadata,
                } = prev;
                GemRelease {
                    version_platform: VersionPlatform { version, platform },
                    deps,
                    metadata,
                }
            }
        }

        let gem_info: HashMap<String, Vec<PrevGemRelease>> = serde_json::from_str(include_str!(
            "../../../../testdata/all_nokogiri_transitive_deps.json"
        ))
        .unwrap();
        let gem_info: HashMap<String, Vec<GemRelease>> = gem_info
            .into_iter()
            .map(|(k, releases)| (k, releases.into_iter().map(GemRelease::from).collect()))
            .collect();
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
