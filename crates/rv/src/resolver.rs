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
