use camino::Utf8PathBuf;
use serde::Serialize;
use tabled::{Table, settings::Style};

use crate::{config::Config, output_format::OutputFormat};
use fs_err as fs;

const NO_TOOLS_INSTALLED: &str = "No tools installed";

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Could not read the rv tool directory: {0}")]
    CouldNotReadToolDir(std::io::Error),
}

#[derive(Debug, Serialize, tabled::Tabled)]
struct Tool {
    gem_name: String,
    version: String,
}

pub fn list(_config: &Config, format: OutputFormat) -> Result<(), Error> {
    let tool_dir = crate::commands::tool::tool_dir();

    // If the tool directory is missing, then there's nothing installed.
    if !tool_dir.try_exists().unwrap_or_default() {
        match format {
            OutputFormat::Text => {
                println!("{NO_TOOLS_INSTALLED}");
            }
            OutputFormat::Json => {
                println!("[]"); // JSON empty list.
            }
        }
        return Ok(());
    }

    // Build a list of all tools, by walking the tools directory.
    let mut tools = Vec::new();
    let tool_dir_children = fs::read_dir(tool_dir).map_err(Error::CouldNotReadToolDir)?;
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
                let Some((gem_name, version)) = file_name.split_once('@') else {
                    eprintln!("Invalid dir name {path}");
                    continue;
                };
                tools.push(Tool {
                    gem_name: gem_name.to_owned(),
                    version: version.to_owned(),
                })
            }
            Err(e) => {
                eprintln!("Could not read dir {e}, skipping");
                continue;
            }
        }
    }

    // By default, let's just sort alphabetically.
    // In the future we should add options for sorting this I guess.
    // For now, users can use JSON output and sort it however they like with jq etc.
    tools.sort_by(|a, b| a.gem_name.cmp(&b.gem_name));

    // Now display the list.
    match format {
        OutputFormat::Text if tools.is_empty() => {
            println!("{NO_TOOLS_INSTALLED}");
        }
        OutputFormat::Text => {
            let mut table = Table::new(tools);
            table.with(Style::modern());
            println!("{table}");
        }
        OutputFormat::Json => {
            let j = serde_json::to_string(&tools)
                .expect("Serializing this data to JSON should always succeed");
            println!("{j}");
        }
    }
    Ok(())
}
