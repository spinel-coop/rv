use std::collections::HashMap;

use pubgrub::{OfflineDependencyProvider, Ranges};
use rv_lockfile::datatypes::SemverConstraint;

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
        let mut overall_range = Ranges::empty();
        for r in ranges {
            overall_range = overall_range.intersection(&r);
        }
        overall_range
    }
}
