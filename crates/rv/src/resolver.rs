use std::collections::HashMap;

use rv_gem_types::{ReleaseTuple, VersionPlatform};

use super::gemserver::{GemName, GemRelease};

use pubgrub::Ranges;

pub type DepProvider = pubgrub::OfflineDependencyProvider<GemName, Ranges<VersionPlatform>>;
pub type ResolutionError = pubgrub::PubGrubError<DepProvider>;

pub fn solve(
    gem: GemName,
    release: GemRelease,
    gem_info: HashMap<GemName, HashMap<VersionPlatform, GemRelease>>,
) -> Result<Vec<(ReleaseTuple, GemRelease)>, ResolutionError> {
    let provider = all_dependencies(&gem_info);
    let solution = pubgrub::resolve(&provider, gem, release.version_platform)?;

    Ok(solution
        .into_iter()
        .map(|(p, vp)| {
            let gem_release = gem_info[&p][&vp].clone();
            let release_tuple = ReleaseTuple {
                name: p,
                version: vp.version,
                platform: vp.platform,
            };

            (release_tuple, gem_release)
        })
        .collect())
}

/// Solve for multiple root gems simultaneously, using a virtual root package.
/// This is used by `--with` to resolve multiple additional gems together.
pub fn solve_multiple(
    gems: Vec<(GemName, GemRelease)>,
    gem_info: HashMap<GemName, HashMap<VersionPlatform, GemRelease>>,
) -> Result<Vec<(ReleaseTuple, GemRelease)>, ResolutionError> {
    let virtual_root: GemName = "__rv_virtual_root__".to_string();
    let virtual_version = VersionPlatform {
        version: "0.0.0".parse().unwrap(),
        platform: rv_gem_types::Platform::Ruby,
    };

    // Build the dependency provider from all gem info.
    let mut provider = all_dependencies(&gem_info);

    // Add the virtual root with dependencies on all the input gems.
    let deps: Vec<(GemName, Ranges<VersionPlatform>)> = gems
        .iter()
        .map(|(name, release)| {
            let range = Ranges::singleton(release.version_platform().clone());
            (name.clone(), range)
        })
        .collect();
    provider.add_dependencies(virtual_root.clone(), virtual_version.clone(), deps);

    let solution = pubgrub::resolve(&provider, virtual_root.clone(), virtual_version)?;

    Ok(solution
        .into_iter()
        .filter(|(p, _)| *p != virtual_root)
        .map(|(p, vp)| {
            let gem_release = gem_info[&p][&vp].clone();
            let release_tuple = ReleaseTuple {
                name: p,
                version: vp.version,
                platform: vp.platform,
            };
            (release_tuple, gem_release)
        })
        .collect())
}

/// Build a PubGrub "dependency provider", i.e. something that can be queried
/// with all the information of a GemServer (which gems are available, what versions that gem has,
/// and what dependencies that gem-version pair has).
/// This is really just taking the `gem_info` hashmap and organizing it in a way that PubGrub can understand.
fn all_dependencies(
    gem_info: &HashMap<GemName, HashMap<VersionPlatform, GemRelease>>,
) -> DepProvider {
    let mut m = DepProvider::new();

    for (package, gem_releases) in gem_info {
        for (version_platform, gem_release) in gem_releases {
            m.add_dependencies(
                package.clone(),
                version_platform.clone(),
                gem_release
                    .deps
                    .clone()
                    .into_iter()
                    .map(|dep| (dep.name, dep.requirement.into())),
            );
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use rv_gem_types::ComparisonOperator;
    use rv_gem_types::requirement::{Requirement, VersionConstraint};
    use std::str::FromStr;

    fn vp(input: &str) -> VersionPlatform {
        VersionPlatform::from_str(input).unwrap()
    }

    fn release_with_dependencies(version: &str, dependencies: &[(&str, &str)]) -> GemRelease {
        GemRelease {
            version_platform: vp(version),
            deps: dependencies
                .iter()
                .map(|(name, requirement)| rv_gem_types::ProjectDependency {
                    name: (*name).to_string(),
                    requirement: Requirement::new(vec![(*requirement).to_string()]).unwrap(),
                })
                .collect(),
            metadata: crate::gemserver::Metadata::default(),
        }
    }

    fn unsatisfiable_self_dependency_registry() -> (
        GemRelease,
        HashMap<GemName, HashMap<VersionPlatform, GemRelease>>,
    ) {
        let root = release_with_dependencies("0", &[("child", ">= 0")]);
        let child_zero = release_with_dependencies("0", &[("child", ">= 1")]);
        let child_one = release_with_dependencies("1", &[("missing", "= 0")]);
        let registry = HashMap::from([
            ("root".to_string(), HashMap::from([(vp("0"), root.clone())])),
            (
                "child".to_string(),
                HashMap::from([(vp("0"), child_zero), (vp("1"), child_one)]),
            ),
        ]);
        (root, registry)
    }

    #[test]
    #[ignore = "PubGrub 0.4.0 can omit a required package after a self-dependency conflict"]
    fn solve_rejects_unsatisfiable_self_dependency() {
        let (root, registry) = unsatisfiable_self_dependency_registry();
        let result = solve("root".to_string(), root, registry);
        assert!(result.is_err(), "unsound successful solution: {result:?}");
    }

    #[test]
    #[ignore = "PubGrub 0.4.0 can omit a required package after a self-dependency conflict"]
    fn solve_multiple_rejects_unsatisfiable_self_dependency() {
        let (root, registry) = unsatisfiable_self_dependency_registry();
        let result = solve_multiple(vec![("root".to_string(), root)], registry);
        assert!(result.is_err(), "unsound successful solution: {result:?}");
    }

    #[test]
    fn satisfied_self_dependency_preserves_transitive_dependencies() {
        let root = release_with_dependencies("0", &[("child", ">= 0")]);
        let child = release_with_dependencies("0", &[("child", "= 0"), ("leaf", "= 0")]);
        let leaf = release_with_dependencies("0", &[]);
        let registry = HashMap::from([
            ("root".to_string(), HashMap::from([(vp("0"), root.clone())])),
            ("child".to_string(), HashMap::from([(vp("0"), child)])),
            ("leaf".to_string(), HashMap::from([(vp("0"), leaf)])),
        ]);
        let solution = solve("root".to_string(), root, registry).unwrap();
        assert_eq!(solution.len(), 3);
        assert!(solution.iter().any(|(tuple, _)| tuple.name == "leaf"));
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
                operator: ComparisonOperator::Pessimistic,
                version: "3.0.3".parse().unwrap(),
            }],
            expected: Ranges::intersection(
                &Ranges::higher_than(vp("3.0.3")),
                &Ranges::strictly_lower_than(vp("3.1.A")),
            ),
        }] {
            // Take the Ruby gem version requirements,
            // translate them to PubGrub version ranges.
            let actual = Ranges::from(Requirement::from(input));
            assert_eq!(actual, expected);
        }
    }
}
