use crate::config::Config;
use camino::Utf8PathBuf;
use fs_err as fs;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Could not read the rv tool directory: {0}")]
    CouldNotReadToolDir(std::io::Error),
    #[error("Could not delete the directory for the tool: {0}")]
    CouldNotDelete(std::io::Error),
}

pub fn uninstall(_config: &Config, target_gem_name: String) -> Result<(), Error> {
    let tool_dir = crate::commands::tool::tool_dir();

    // If the tool directory is missing, then there's nothing to uninstall.
    if !tool_dir.try_exists().unwrap_or_default() {
        tracing::debug!("No tools directory found at {tool_dir}");
        return Ok(());
    }

    // Walk the tools directory, looking for the named gem.
    let tool_dir_children = fs::read_dir(tool_dir).map_err(Error::CouldNotReadToolDir)?;
    let mut deleted = 0;
    for child in tool_dir_children {
        match child {
            Ok(child) => {
                let Ok(path) = Utf8PathBuf::try_from(child.path()) else {
                    tracing::debug!("Skipping non-UTF-8 directory");
                    continue;
                };
                if !path.is_dir() {
                    continue;
                }
                let Some(file_name) = path.file_name() else {
                    eprintln!("Path {path} has no file name, skipping");
                    continue;
                };
                let Some((gem_name, _version)) = file_name.split_once('@') else {
                    eprintln!("Invalid dir name {path}");
                    continue;
                };
                if gem_name == target_gem_name {
                    fs::remove_dir_all(path).map_err(Error::CouldNotDelete)?;
                    tracing::debug!("Uninstalled tool {target_gem_name}");
                    deleted += 1;
                }
            }
            Err(e) => {
                eprintln!("Could not read dir {e}, skipping");
                continue;
            }
        }
    }

    tracing::debug!("Deleted {deleted} installed tools for {target_gem_name}");
    Ok(())
}
