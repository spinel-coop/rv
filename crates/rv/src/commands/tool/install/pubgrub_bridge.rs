use std::collections::HashMap;

use pubgrub::{OfflineDependencyProvider, Ranges};

use super::{
    GemName,
    gem_version::GemVersion,
    gemserver::{VersionAvailable, VersionConstraints},
};

/// Build a PubGrub "dependency provider", i.e. something that can be queried
/// with all the information of a GemServer (which gems are available, what versions that gem has,
/// and what dependencies that gem-version pair has).
/// This is really just taking the `gem_info` hashmap and organizing it in a way that PubGrub can understand.
pub fn all_dependencies(
    gem_info: HashMap<GemName, Vec<VersionAvailable>>,
) -> OfflineDependencyProvider<GemName, Ranges<GemVersion>> {
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
    fn from(_value: VersionConstraints) -> Self {
        todo!("Translate GemVersions into PubGrub's data model")
    }
}
