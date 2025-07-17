use std::{
    env,
    ffi::OsString,
    path::{Path, PathBuf},
};

use etcetera::BaseStrategy;
use vfs::VfsPath;

/// Returns an appropriate user-level directory for storing executables.
///
/// This follows, in order:
///
/// - `$OVERRIDE_VARIABLE` (if provided)
/// - `$XDG_BIN_HOME`
/// - `$XDG_DATA_HOME/../bin`
/// - `$HOME/.local/bin`
///
/// On all platforms.
///
/// Returns `None` if a directory cannot be found, i.e., if `$HOME` cannot be resolved. Does not
/// check if the directory exists.
pub fn user_executable_directory(override_variable: Option<&'static str>) -> Option<PathBuf> {
    override_variable
        .and_then(std::env::var_os)
        .and_then(parse_path)
        .or_else(|| std::env::var_os("XDG_BIN_HOME").and_then(parse_path))
        .or_else(|| {
            std::env::var_os("XDG_DATA_HOME")
                .and_then(parse_path)
                .map(|path| path.join("../bin"))
        })
        .or_else(|| {
            let home_dir = etcetera::home_dir().ok();
            home_dir.map(|path| path.join(".local").join("bin"))
        })
}

/// Returns an appropriate user-level directory for storing the cache.
///
/// Corresponds to `$XDG_CACHE_HOME/rv` on Unix.
pub fn user_cache_dir(root: &VfsPath) -> VfsPath {
    let cache_path = etcetera::base_strategy::choose_base_strategy()
        .ok()
        .map(|dirs| dirs.cache_dir().join("rv"))
        .unwrap_or_else(|| std::env::temp_dir().join("rv"));

    root.join(cache_path.to_string_lossy().as_ref())
        .unwrap_or_else(|_| {
            root.join("tmp")
                .and_then(|p| p.join("rv"))
                .unwrap_or_else(|_| root.clone())
        })
}

/// Returns the legacy cache directory path.
///
/// Uses `/Users/user/Library/Application Support/rv` on macOS, in contrast to the new preference
/// for using the XDG directories on all Unix platforms.
pub fn legacy_user_cache_dir() -> Option<PathBuf> {
    etcetera::base_strategy::choose_native_strategy()
        .ok()
        .map(|dirs| dirs.cache_dir().join("rv"))
        .map(|dir| {
            if cfg!(windows) {
                dir.join("cache")
            } else {
                dir
            }
        })
}

/// Returns an appropriate user-level directory for storing application state.
///
/// Corresponds to `$XDG_DATA_HOME/rv` on Unix.
pub fn user_state_dir(root: &VfsPath) -> VfsPath {
    let data_path = etcetera::base_strategy::choose_base_strategy()
        .ok()
        .map(|dirs| dirs.data_dir().join("rv"))
        .unwrap_or_else(|| std::env::temp_dir().join("rv"));

    root.join(data_path.to_string_lossy().as_ref())
        .unwrap_or_else(|_| {
            root.join("tmp")
                .and_then(|p| p.join("rv"))
                .unwrap_or_else(|_| root.clone())
        })
}

/// Returns the legacy state directory path.
///
/// Uses `/Users/user/Library/Application Support/rv` on macOS, in contrast to the new preference
/// for using the XDG directories on all Unix platforms.
pub fn legacy_user_state_dir() -> Option<PathBuf> {
    etcetera::base_strategy::choose_native_strategy()
        .ok()
        .map(|dirs| dirs.data_dir().join("rv"))
        .map(|dir| if cfg!(windows) { dir.join("data") } else { dir })
}

/// Return a [`PathBuf`] if the given [`OsString`] is an absolute path.
fn parse_path(path: OsString) -> Option<PathBuf> {
    let path = PathBuf::from(path);
    if path.is_absolute() { Some(path) } else { None }
}

/// Returns the path to the user configuration directory.
///
/// On Windows, use, e.g., C:\Users\Alice\AppData\Roaming
/// On Linux and macOS, use `XDG_CONFIG_HOME` or $HOME/.config, e.g., /home/alice/.config.
pub fn user_config_dir() -> Option<PathBuf> {
    etcetera::choose_base_strategy()
        .map(|dirs| dirs.config_dir())
        .ok()
}

pub fn user_rv_config_dir() -> Option<PathBuf> {
    user_config_dir().map(|mut path| {
        path.push("rv");
        path
    })
}

#[cfg(not(windows))]
fn locate_system_config_xdg(value: Option<&str>) -> Option<PathBuf> {
    // On Linux and macOS, read the `XDG_CONFIG_DIRS` environment variable.

    use std::path::Path;
    let default = "/etc/xdg";
    let config_dirs = value.filter(|s| !s.is_empty()).unwrap_or(default);

    for dir in config_dirs.split(':').take_while(|s| !s.is_empty()) {
        let rv_toml_path = Path::new(dir).join("rv").join("rv.toml");
        if rv_toml_path.is_file() {
            return Some(rv_toml_path);
        }
    }
    None
}

#[cfg(windows)]
fn locate_system_config_windows(system_drive: impl AsRef<Path>) -> Option<PathBuf> {
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
pub fn system_config_file() -> Option<PathBuf> {
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
        let candidate = Path::new("/etc/rv/rv.toml");
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
    #[cfg(unix)]
    fn test_locate_system_config_xdg_unix_permissions() -> Result<(), FixtureError> {
        let context = assert_fs::TempDir::new()?;
        let config = context.child("rv").child("rv.toml");
        config.write_str("")?;
        fs_err::set_permissions(
            &context,
            std::os::unix::fs::PermissionsExt::from_mode(0o000),
        )
        .unwrap();

        assert_eq!(
            locate_system_config_xdg(Some(context.to_str().unwrap())),
            None
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
        assert_eq!(
            locate_system_config_windows(context.path()).unwrap(),
            context
                .child("ProgramData")
                .child("rv")
                .child("rv.toml")
                .path()
        );

        // This does not have a `ProgramData` child, so contains no config.
        let context = assert_fs::TempDir::new()?;
        assert_eq!(locate_system_config_windows(context.path()), None);

        Ok(())
    }
}
