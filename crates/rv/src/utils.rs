use rv_dirs;

/// Checks if the current executable is installed via Homebrew.
pub fn is_homebrew_install() -> bool {
    if cfg!(target_family = "windows") {
        return false;
    }

    let current_exe = match rv_dirs::current_exe() {
        Ok(path) => match rv_dirs::canonicalize_utf8(path) {
            Ok(canonical) => canonical,
            Err(_) => return false,
        },
        Err(_) => return false,
    };

    current_exe.starts_with("/usr/local/Cellar") || current_exe.starts_with("/opt/homebrew/Cellar")
}
