use std::env;
use std::path::PathBuf;
use std::process::Command;

pub fn brew_prefix() -> Option<PathBuf> {
    if cfg!(target_os = "windows") {
        return None;
    }

    let brew_path = which::which("brew").ok()?;
    let output = Command::new(brew_path).arg("--prefix").output().ok()?;

    if !output.status.success() {
        return None;
    }

    let prefix = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if prefix.is_empty() {
        return None;
    }

    Some(PathBuf::from(prefix))
}

pub fn is_homebrew_install() -> bool {
    if cfg!(target_family = "windows") {
        return false;
    }

    let exe_path = match env::current_exe() {
        Ok(p) => p,
        Err(_) => return false,
    };

    let brew_prefix = match brew_prefix() {
        Some(p) => p,
        None => return false,
    };

    if !exe_path.starts_with(&brew_prefix) {
        return false;
    }

    let rel_path = match exe_path.strip_prefix(&brew_prefix) {
        Ok(p) => p,
        Err(_) => return false,
    };

    let has_bin = rel_path.components().any(|c| c.as_os_str() == "bin");

    let exe_name = match exe_path.file_name() {
        Some(n) => n,
        None => return false,
    };

    let ends_with_exe = matches!(rel_path.components().next_back(), Some(std::path::Component::Normal(n)) if n == exe_name);

    has_bin && ends_with_exe
}
