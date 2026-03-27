use std::collections::HashMap;
use std::fmt::Display;

use rv_gem_types::{ProjectDependency, ReleaseTuple, VersionPlatform};

use super::gemserver::{GemName, GemRelease};

use pubgrub::Ranges;

pub type DepProvider =
    pubgrub::OfflineDependencyProvider<ResolutionPackage, Ranges<VersionPlatform>>;
pub type ResolutionError = pubgrub::PubGrubError<DepProvider>;

#[derive(Clone, Ord, PartialOrd, PartialEq, Eq, Hash, Debug)]
pub enum ResolutionPackage {
    Gem(GemName),
    Gemfile,
}

impl Display for ResolutionPackage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Gem(gem_name) => write!(f, "{gem_name}"),
            Self::Gemfile => write!(f, "the gemfile"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolutionRoot {
    pub package: ResolutionPackage,
    pub version_platform: VersionPlatform,
    pub deps: Vec<ProjectDependency>,
}

pub fn solve(
    root: &ResolutionRoot,
    gem_info: &HashMap<ResolutionPackage, HashMap<VersionPlatform, GemRelease>>,
) -> Result<Vec<(ReleaseTuple, GemRelease)>, ResolutionError> {
    let provider = all_dependencies(root, gem_info);
    let solution = pubgrub::resolve(
        &provider,
        root.package.clone(),
        root.version_platform.clone(),
    )?;

    Ok(solution
        .into_iter()
        .filter_map(|(p, vp)| match p {
            ResolutionPackage::Gem(ref name) if name.as_str() != "bundler" => {
                let gem_release = gem_info[&p][&vp].clone();
                let release_tuple = ReleaseTuple {
                    name: name.to_string(),
                    version: vp.version,
                    platform: vp.platform,
                };

                Some((release_tuple, gem_release))
            }
            _ => None,
        })
        .collect())
}

/// Build a PubGrub "dependency provider", i.e. something that can be queried
/// with all the information of a GemServer (which gems are available, what versions that gem has,
/// and what dependencies that gem-version pair has).
/// This is really just taking the `gem_info` hashmap and organizing it in a way that PubGrub can understand.
fn all_dependencies(
    root: &ResolutionRoot,
    gem_info: &HashMap<ResolutionPackage, HashMap<VersionPlatform, GemRelease>>,
) -> DepProvider {
    let mut m = DepProvider::new();

    m.add_dependencies(
        root.package.clone(),
        root.version_platform.clone(),
        root.deps
            .clone()
            .into_iter()
            .map(|dep| (ResolutionPackage::Gem(dep.name), dep.requirement.into())),
    );

    for (package, gem_releases) in gem_info {
        for (version_platform, gem_release) in gem_releases {
            m.add_dependencies(
                package.clone(),
                version_platform.clone(),
                gem_release
                    .deps
                    .clone()
                    .into_iter()
                    .map(|dep| (ResolutionPackage::Gem(dep.name), dep.requirement.into())),
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
