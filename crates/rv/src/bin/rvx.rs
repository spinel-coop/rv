use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::{
    ffi::OsString,
    process::{Command, ExitCode, ExitStatus},
};

/// Spawns a command exec style.
fn exec_spawn(cmd: &mut Command) -> std::io::Result<Infallible> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = cmd.exec();
        Err(err)
    }
    #[cfg(windows)]
    {
        rv_windows::spawn_child(cmd, false)
    }
}

/// Assuming the binary is called something like `rvx@1.2.3(.exe)`, compute the `@1.2.3(.exe)` part
/// so that we can preferentially find `rv@1.2.3(.exe)`, for folks who like managing multiple
/// installs in this way.
fn get_rvx_suffix(current_exe: &Path) -> Option<&str> {
    let os_file_name = current_exe.file_name()?;
    let file_name_str = os_file_name.to_str()?;
    file_name_str.strip_prefix("rvx")
}

/// Gets the path to `rv`, given info about `rvx`
fn get_rv_path(current_exe_parent: &Path, rvx_suffix: Option<&str>) -> std::io::Result<PathBuf> {
    // First try to find a matching suffixed `rv`, e.g. `rv@1.2.3(.exe)`
    let rv_with_suffix = rvx_suffix.map(|suffix| current_exe_parent.join(format!("rv{suffix}")));
    if let Some(rv_with_suffix) = &rv_with_suffix {
        #[expect(clippy::print_stderr, reason = "printing a very rare warning")]
        match rv_with_suffix.try_exists() {
            Ok(true) => return Ok(rv_with_suffix.to_owned()),
            Ok(false) => { /* definitely not there, proceed to fallback */ }
            Err(err) => {
                // We don't know if `rv@1.2.3` exists, something errored when checking.
                // We *could* blindly use `rv@1.2.3` in this case, as the code below does, however
                // in this extremely narrow corner case it's *probably* better to default to `rv`,
                // since we don't want to mess up existing users who weren't using suffixes?
                eprintln!(
                    "warning: failed to determine if `{}` exists, trying `rv` instead: {err}",
                    rv_with_suffix.display()
                );
            }
        }
    }

    // Then just look for good ol' `rv`
    let rv = current_exe_parent.join(format!("rv{}", std::env::consts::EXE_SUFFIX));
    // If we are sure the `rv` binary does not exist, display a clearer error message.
    // If we're not certain if rv exists (try_exists == Err), keep going and hope it works.
    if matches!(rv.try_exists(), Ok(false)) {
        let message = if let Some(rv_with_suffix) = rv_with_suffix {
            format!(
                "Could not find the `rv` binary at either of:\n  {}\n  {}",
                rv_with_suffix.display(),
                rv.display(),
            )
        } else {
            format!("Could not find the `rv` binary at: {}", rv.display())
        };
        Err(std::io::Error::new(std::io::ErrorKind::NotFound, message))
    } else {
        Ok(rv)
    }
}

fn run() -> std::io::Result<ExitStatus> {
    let current_exe = std::env::current_exe()?;
    let Some(bin) = current_exe.parent() else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not determine the location of the `rvx` binary",
        ));
    };
    let rvx_suffix = get_rvx_suffix(&current_exe);
    let rv = get_rv_path(bin, rvx_suffix)?;
    let args = ["tool", "run"]
        .iter()
        .map(OsString::from)
        // Skip the `rvx` name
        .chain(std::env::args_os().skip(1))
        .collect::<Vec<_>>();

    let mut cmd = Command::new(rv);
    cmd.args(&args);
    match exec_spawn(&mut cmd)? {}
}

#[expect(clippy::print_stderr)]
fn main() -> ExitCode {
    let result = run();
    match result {
        // Fail with 2 if the status cannot be cast to an exit code
        Ok(status) => u8::try_from(status.code().unwrap_or(2)).unwrap_or(2).into(),
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::from(2)
        }
    }
}
