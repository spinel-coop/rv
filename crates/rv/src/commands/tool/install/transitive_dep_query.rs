use std::collections::HashMap;

use crate::commands::tool::install::gemserver::{self, Gemserver, VersionAvailable};

pub async fn query_all_gem_deps_from_server(
    root: &VersionAvailable,
    gemserver: &Gemserver,
    gems_to_deps: &mut HashMap<String, Vec<VersionAvailable>>,
) -> super::Result<()> {
    let mut gems_to_look_up: Vec<String> = root
        .deps
        .iter()
        .map(|dep| dep.gem_name.to_owned())
        .collect();
    while let Some(next_gem) = gems_to_look_up.pop() {
        if gems_to_deps.contains_key(&next_gem) {
            continue;
        }
        let dep_info_resp = gemserver.get_versions_for_gem(&next_gem).await?;
        let dep_versions = gemserver::parse_version_from_body(&dep_info_resp)?;
        eprintln!("Found {} versions for {}", dep_versions.len(), next_gem);
        for dep_version in &dep_versions {
            for dep in &dep_version.deps {
                if gems_to_deps.contains_key(&dep.gem_name) {
                    continue;
                }
                gems_to_look_up.push(dep.gem_name.to_owned());
            }
        }
        gems_to_deps.insert(next_gem.to_owned().to_owned(), dep_versions);
    }
    Ok(())
}
