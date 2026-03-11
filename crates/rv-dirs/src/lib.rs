use std::{env, ffi::OsString, io};

use camino::{Utf8Path, Utf8PathBuf};
use etcetera::BaseStrategy;
use indexmap::IndexSet;

pub fn canonical_ruby_dirs(
    ruby_dir: &[Utf8PathBuf],
    root: &Utf8Path,
) -> io::Result<IndexSet<Utf8PathBuf>> {
    let dirs = if ruby_dir.is_empty() {
        default_ruby_dirs(root)
    } else {
        ruby_dir
            .iter()
            .map(|path: &Utf8PathBuf| Ok(root.join(canonicalize_utf8(path)?)))
            .collect::<io::Result<Vec<_>>>()?
    };

    Ok(dirs.into_iter().collect())
}

pub fn current_exe() -> io::Result<Utf8PathBuf> {
    let exe = env::var("RV_TEST_EXE").unwrap_or(env::current_exe()?.to_str().unwrap().to_string());

    Ok(Utf8PathBuf::from(exe))
}

pub fn relativize(path: &Utf8Path) -> String {
    let Some(current_dir) = std::env::current_dir().ok() else {
        return path.to_string();
    };

    let Some(file_name) = path.file_name().map(|f| f.to_string()) else {
        return path.to_string();
    };

    let mut relative_path = file_name.clone();

    for dir in current_dir.ancestors() {
        if dir.join(&file_name).is_file() {
            return relative_path;
        }

        relative_path.insert_str(0, "../");
    }

    relative_path
}

pub fn unexpand(path: &Utf8Path) -> String {
    path.as_str().replace(home_dir().as_str(), "~")
}

/// Default Ruby installation directories
pub fn default_ruby_dirs(root: &Utf8Path) -> Vec<Utf8PathBuf> {
    let paths: [(_, _); 6] = [
        (true, xdg_data_path()),
        (false, legacy_default_data_path()),
        (false, legacy_default_path()),
        (false, "/opt/rubies".into()),
        (false, "/usr/local/rubies".into()),
        (false, "/opt/homebrew/Cellar/ruby".into()),
    ];

    paths
        .iter()
        .filter_map(|(always_include, path)| {
            let join = root.join(path.strip_prefix("/").unwrap_or(path));
            canonicalize_utf8(&join)
                .ok()
                .or(always_include.then_some(path.into()))
        })
        .collect()
}

fn xdg_data_path() -> Utf8PathBuf {
    user_state_dir("/".into()).join("rubies")
}

fn legacy_default_data_path() -> Utf8PathBuf {
    home_dir().join(".data/rv/.rubies")
}

fn legacy_default_path() -> Utf8PathBuf {
    home_dir().join(".rubies")
}

/// Canonicalize a path without the Windows `\\?\` extended-length prefix.
///
/// On Windows, [`std::fs::canonicalize`] returns paths with the `\\?\` prefix,
/// which breaks `cmd.exe`, many Windows tools, and string-based path comparisons.
/// This function uses [`dunce::canonicalize`] to return clean canonical paths
/// on all platforms (following the same pattern as uv's `simple_canonicalize`).
pub fn canonicalize_utf8(path: impl AsRef<Utf8Path>) -> io::Result<Utf8PathBuf> {
    dunce::canonicalize(path.as_ref()).and_then(|p| {
        Utf8PathBuf::try_from(p).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
    })
}

pub fn project_root(root: &Utf8PathBuf) -> io::Result<Utf8PathBuf> {
    let current_dir = Utf8PathBuf::try_from(std::env::current_dir()?)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;

    Ok(current_dir
        .ancestors()
        .take_while(|d| Some(*d) != root.parent())
        .find(|d| d.join("Gemfile.lock").is_file())
        .map(|p| p.to_path_buf())
        .unwrap_or(current_dir))
}

pub fn root_dir() -> Utf8PathBuf {
    Utf8PathBuf::from(env::var("RV_ROOT_DIR").unwrap_or("/".to_owned()))
}

/// Returns an appropriate user-home directory, or the system temporary directory if the platform
/// does not have user home directories
pub fn home_dir() -> Utf8PathBuf {
    etcetera::home_dir()
        .ok()
        .unwrap_or_else(env::temp_dir)
        .to_string_lossy()
        .as_ref()
        .into()
}

/// Returns an appropriate user-level directory for storing executables.
///
/// This follows, in order:
///
/// - `$OVERRIDE_VARIABLE` (if provided)
/// - `$XDG_BIN_HOME`
/// - `$HOME/.local/bin`
///
/// On all platforms.
///
/// Does not check if the directory exists.
pub fn user_executable_directory(override_variable: Option<&'static str>) -> Utf8PathBuf {
    override_variable
        .and_then(env::var_os)
        .and_then(parse_path)
        .or_else(|| env::var_os("XDG_BIN_HOME").and_then(parse_path))
        .unwrap_or_else(|| home_dir().join(".local/bin"))
}

/// Returns an appropriate user-level directory for storing the cache.
///
/// Corresponds to `$XDG_CACHE_HOME/rv` on Unix.
pub fn user_cache_dir(root: &Utf8Path) -> Utf8PathBuf {
    let cache_path = etcetera::base_strategy::choose_base_strategy()
        .ok()
        .map(|dirs| dirs.cache_dir().join("rv"))
        .unwrap_or_else(|| env::temp_dir().join(".cache/rv"));

    root.join(cache_path.to_string_lossy().as_ref())
}

/// Returns an appropriate user-level directory for storing application state.
///
/// Corresponds to `$XDG_DATA_HOME/rv` on Unix.
pub fn user_state_dir(root: &Utf8Path) -> Utf8PathBuf {
    let data_path = etcetera::base_strategy::choose_base_strategy()
        .ok()
        .map(|dirs| dirs.data_dir().join("rv"))
        .unwrap_or_else(|| env::temp_dir().join(".local/share/rv"));

    root.join(data_path.to_string_lossy().as_ref())
}

/// Return a [`Utf8PathBuf`] if the given [`OsString`] is an absolute path.
fn parse_path(path: OsString) -> Option<Utf8PathBuf> {
    let path = Utf8PathBuf::from(path.into_string().unwrap());
    if path.is_absolute() { Some(path) } else { None }
}

/// Returns the path to the user configuration directory.
///
/// On Windows, use, e.g., C:\Users\Alice\AppData\Roaming
/// On Linux and macOS, use `XDG_CONFIG_HOME` or $HOME/.config, e.g., /home/alice/.config.
pub fn user_config_dir(root: &Utf8Path) -> Utf8PathBuf {
    let config_path = etcetera::base_strategy::choose_base_strategy()
        .ok()
        .map(|dirs| dirs.config_dir().join("rv"))
        .unwrap_or_else(|| env::temp_dir().join(".config/rv"));

    root.join(config_path.to_string_lossy().as_ref())
}

#[cfg(not(windows))]
fn locate_system_config_xdg(value: Option<&str>) -> Option<Utf8PathBuf> {
    // On Linux and macOS, read the `XDG_CONFIG_DIRS` environment variable.

    let default = "/etc/xdg";
    let config_dirs = value.filter(|s| !s.is_empty()).unwrap_or(default);

    for dir in config_dirs.split(':').take_while(|s| !s.is_empty()) {
        let rv_toml_path = Utf8Path::new(dir).join("rv").join("rv.toml");
        if rv_toml_path.is_file() {
            return Some(rv_toml_path);
        }
    }
    None
}

#[cfg(windows)]
fn locate_system_config_windows(system_drive: impl AsRef<Utf8Path>) -> Option<Utf8PathBuf> {
    // On Windows, use `%SYSTEMDRIVE%\ProgramData\rv\rv.toml` (e.g., `C:\ProgramData`).
    let candidate = system_drive
        .as_ref()
        .join("ProgramData")
        .join("rv")
        .join("rv.toml");
    candidate.as_path().is_file().then_some(candidate)
}

/// Returns the path to the system configuration file.
///
/// On Unix-like systems, uses the `XDG_CONFIG_DIRS` environment variable (falling back to
/// `/etc/xdg/rv/rv.toml` if unset or empty) and then `/etc/rv/rv.toml`
///
/// On Windows, uses `%SYSTEMDRIVE%\ProgramData\rv\rv.toml`.
pub fn system_config_file() -> Option<Utf8PathBuf> {
    #[cfg(windows)]
    {
        env::var("SYSTEMDRIVE")
            .ok()
            .and_then(|system_drive| locate_system_config_windows(format!("{system_drive}\\")))
    }

    #[cfg(not(windows))]
    {
        if let Some(path) = locate_system_config_xdg(env::var("XDG_CONFIG_DIRS").ok().as_deref()) {
            return Some(path);
        }

        // Fallback to `/etc/rv/rv.toml` if `XDG_CONFIG_DIRS` is not set or no valid
        // path is found.
        let candidate = Utf8Path::new("/etc/rv/rv.toml");
        match candidate.try_exists() {
            Ok(true) => Some(candidate.to_path_buf()),
            Ok(false) => None,
            Err(err) => {
                tracing::warn!("Failed to query system configuration file: {err}");
                None
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use assert_fs::fixture::FixtureError;
    use assert_fs::prelude::*;
    use indoc::indoc;

    #[test]
    #[cfg(not(windows))]
    fn test_locate_system_config_xdg() -> Result<(), FixtureError> {
        // Write a `rv.toml` to a temporary directory.
        let context = assert_fs::TempDir::new()?;
        context.child("rv").child("rv.toml").write_str(indoc! {
            r#"
            [ruby]
            index-url = "https://rubygems.org"
        "#,
        })?;

        // None
        assert_eq!(locate_system_config_xdg(None), None);

        // Empty string
        assert_eq!(locate_system_config_xdg(Some("")), None);

        // Single colon
        assert_eq!(locate_system_config_xdg(Some(":")), None);

        // Assert that the `system_config_file` function returns the correct path.
        assert_eq!(
            locate_system_config_xdg(Some(context.to_str().unwrap())).unwrap(),
            context.child("rv").child("rv.toml").path()
        );

        // Write a separate `rv.toml` to a different directory.
        let first = context.child("first");
        let first_config = first.child("rv").child("rv.toml");
        first_config.write_str("")?;

        assert_eq!(
            locate_system_config_xdg(Some(
                format!("{}:{}", first.to_string_lossy(), context.to_string_lossy()).as_str()
            ))
            .unwrap(),
            first_config.path()
        );

        Ok(())
    }

    #[test]
    #[cfg(windows)]
    fn test_windows_config() -> Result<(), FixtureError> {
        // Write a `rv.toml` to a temporary directory.
        let context = assert_fs::TempDir::new()?;
        context
            .child("ProgramData")
            .child("rv")
            .child("rv.toml")
            .write_str(indoc! { r#"
            [ruby]
            index-url = "https://rubygems.org"
        "#})?;

        // This is typically only a drive (that is, letter and colon) but we
        // allow anything, including a path to the test fixtures...
        let context_utf8 = Utf8Path::from_path(context.path()).unwrap();
        assert_eq!(
            locate_system_config_windows(context_utf8).unwrap(),
            context
                .child("ProgramData")
                .child("rv")
                .child("rv.toml")
                .path()
        );

        // This does not have a `ProgramData` child, so contains no config.
        let context = assert_fs::TempDir::new()?;
        let context_utf8 = Utf8Path::from_path(context.path()).unwrap();
        assert_eq!(locate_system_config_windows(context_utf8), None);

        Ok(())
    }
}
