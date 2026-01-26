use camino::Utf8PathBuf;
use rv_ruby::version::RubyVersion;
use rv_version::{Version, VersionError};
use tracing::debug;

use crate::commands::tool::{Installed, install as tool_install};
use crate::config::Config;
use fs_err as fs;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    VersionError(#[from] VersionError),
    #[error("You cannot give the version in both the executable and the gem, give only one.")]
    VersionGivenTwice,
    #[error("Could not read the rv tool directory: {0}")]
    CouldNotReadToolDir(std::io::Error),
    #[error("Could not find executable {exe} under gem {gem}@{version}")]
    ExecutableNotFound {
        exe: String,
        gem: String,
        version: Version,
    },
    #[error(transparent)]
    Install(#[from] tool_install::Error),
    #[error("Tool was not found, and you set --no-install so rv won't install it.")]
    NotInstalled,
    #[error(transparent)]
    ExecError(std::io::Error),
    #[error("No .ruby-version found for this tool")]
    NoRubyVersion,
    #[error("Could not read .ruby-version: {0}")]
    CouldNotReadRubyVersion(std::io::Error),
    #[error("Invalid version in .ruby-version: {0}")]
    InvalidRubyVersion(rv_ruby::version::ParseVersionError),
}

/// A version of a gem, given by the user.
#[derive(Clone)]
enum UserVersion {
    /// Use this specific version.
    Use(Version),
    /// Use the latest version available.
    Latest,
}

use UserVersion::Latest;

impl UserVersion {
    /// Append this version to the gem name, e.g. mygem@1.2.0 or mygem@latest
    fn suffix_of(&self, gem_name: &str) -> String {
        match self {
            Self::Use(version) => format!("{gem_name}@{version}"),
            Latest => format!("{gem_name}@latest"),
        }
    }
}

impl std::fmt::Debug for UserVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Use(v) => write!(f, "{v}"),
            Latest => write!(f, "latest"),
        }
    }
}

/// A gem or executable name, with a version.
/// If version is not given, assumed to be 'latest'.
#[derive(Debug)]
struct WithVersion<'i> {
    name: &'i str,
    version: UserVersion,
}

impl<'i> WithVersion<'i> {
    // This uses an adhoc method called `parse` instead of implementing `FromStr`,
    // so that it can reference the input string instead of copying it.
    // Unfortunately `FromStr` cannot keep references to the original string.
    fn parse(s: &'i str) -> Result<Self, VersionError> {
        if let Some((lhs, version_str)) = s.split_once('@') {
            let version = UserVersion::Use(version_str.parse()?);
            Ok(Self { name: lhs, version })
        } else {
            Ok(Self {
                name: s,
                version: Latest,
            })
        }
    }
}

/// Run a tool, installing it from a gem if necessary.
pub async fn run(
    config: &Config,
    executable: String,
    gem: Option<String>,
    gem_server: String,
    no_install: bool,
) -> Result<(), Error> {
    // Parse out the CLI args.
    let executable = WithVersion::parse(&executable)?;
    let gem = match gem {
        Some(ref gem) => Some(WithVersion::parse(gem)?),
        None => None,
    };
    let (target_gem_name, target_gem_version) = match gem {
        // If the user didn't give us a gem name, assume the binary name and gem name are the same.
        None => (executable.name, executable.version),
        // If the user did give us a gem, find the gem version.
        Some(gem) => {
            let version = match (gem.version, executable.version) {
                (UserVersion::Use(_v1), UserVersion::Use(_v2)) => {
                    return Err(Error::VersionGivenTwice);
                }
                (UserVersion::Use(version), Latest) => UserVersion::Use(version),
                (Latest, UserVersion::Use(version)) => UserVersion::Use(version),
                (Latest, Latest) => Latest,
            };

            (gem.name, version)
        }
    };

    let target_executable_name = executable.name;

    debug!(
        "Locating gem {target_gem_name}, bin {target_executable_name}, version {target_gem_version:?}"
    );
    let installed_tool = match find_dir(target_gem_name, target_gem_version.clone())? {
        Some(dir) => {
            debug!("Found tool {target_gem_name}@{}", dir.version);
            dir
        }
        None => {
            if no_install {
                return Err(Error::NotInstalled);
            }
            tool_install::install(
                config,
                target_gem_version.suffix_of(target_gem_name),
                gem_server,
                false,
            )
            .await?
        }
    };
    let gem_home = installed_tool.dir.clone();
    let ruby_version_path = installed_tool.dir.join(".ruby-version");
    if !ruby_version_path.exists() {
        return Err(Error::NoRubyVersion)?;
    }
    let ruby_version: RubyVersion = fs::read_to_string(ruby_version_path)
        .map_err(Error::CouldNotReadRubyVersion)?
        .parse()
        .map_err(Error::InvalidRubyVersion)?;
    debug!("Tool requires Ruby {ruby_version}");
    let file = installed_tool.dir.join("bin").join(executable.name);
    if !file.exists() {
        return Err(Error::ExecutableNotFound {
            exe: target_executable_name.to_owned(),
            gem: target_gem_name.to_owned(),
            version: installed_tool.version,
        });
    }

    // TODO: I've got to add more env here.
    let mut cmd = std::process::Command::new(file);
    cmd.env("GEM_HOME", gem_home);

    exec(cmd)
}

#[cfg(unix)]
fn exec(mut cmd: std::process::Command) -> Result<(), Error> {
    use std::os::unix::process::CommandExt;
    Err(Error::ExecError(cmd.exec()))
}

/// Iterate over the tools directory, to find the right gem/version pair.
/// If no matching tool could be found, returns None.
/// Otherwise, returns the matching tool installation.
fn find_dir(
    target_gem_name: &str,
    target_gem_version: UserVersion,
) -> Result<Option<Installed>, Error> {
    let tool_dir = crate::commands::tool::tool_dir();
    if !tool_dir.exists() {
        debug!("No tool dir exists, so no tools are installed, so no matching tool found");
        return Ok(None);
    }
    let tool_dir_children = fs::read_dir(tool_dir).map_err(Error::CouldNotReadToolDir)?;
    let mut chosen_dir: Option<Installed> = None;
    for child in tool_dir_children {
        // Find installed tools that match the target gem.
        let child_dir = match child {
            Err(e) => {
                eprintln!("Could not read dir {e}, skipping");
                continue;
            }
            Ok(child) => child,
        };
        let Ok(path) = Utf8PathBuf::try_from(child_dir.path()) else {
            debug!("Skipping non-UTF-8 directory");
            continue;
        };
        if !path.is_dir() {
            continue;
        }
        let Some(file_name) = path.file_name() else {
            eprintln!("WARNING: Path {path} has no file name, skipping");
            continue;
        };
        let Some((this_gem_name, this_version)) = file_name.split_once('@') else {
            eprintln!("WARNING: Invalid dir name {path}, skipping");
            continue;
        };
        let Ok(this_version) = this_version.parse() else {
            eprintln!("WARNING: Invalid version in dir {path}, skipping");
            continue;
        };
        if this_gem_name != target_gem_name {
            continue;
        }

        // Now we've found a matching gem name, let's see if the version matches too.
        match target_gem_version {
            UserVersion::Use(ref version) => {
                if version == &this_version {
                    debug!("Found exact version requested");
                    return Ok(Some(Installed {
                        dir: path,
                        version: version.to_owned(),
                    }));
                }
            }
            Latest => match chosen_dir {
                Some(ref prev) => {
                    if this_version > prev.version {
                        debug!("Found later candidate version {this_version}");
                        chosen_dir = Some(Installed {
                            dir: path,
                            version: this_version,
                        });
                    } else {
                        // Previous version was larger, so leave it there.
                        // No-op.
                        debug!("Found earlier version {this_version}, ignoring it");
                    }
                }
                None => {
                    debug!("Found candidate version {this_version}");
                    chosen_dir = Some(Installed {
                        dir: path,
                        version: this_version,
                    })
                }
            },
        }
    }
    Ok(chosen_dir)
}
