use axoupdater::AxoUpdater;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    AxoupdateError(#[from] axoupdater::AxoupdateError),
}

type Result<T> = miette::Result<T, Error>;

pub(crate) async fn selfupdate() -> Result<()> {
    if homebrew_install()? {
        println!(
            "Your copy of `rv` was installed via Homebrew. Run `brew upgrade rv` to update it."
        );
        return Ok(());
    }

    let mut updater = AxoUpdater::new_for("rv");

    if updater.load_receipt().is_err() || !updater.check_receipt_is_for_this_executable()? {
        println!(
            "Your copy of `rv` was not installed via a method that `rv selfupdate` supports. Please update manually."
        );
        return Ok(());
    }

    if let Some(result) = updater.run().await? {
        println!("rv {} installed!", result.new_version);
    } else {
        println!("rv is already up to date!");
    }

    Ok(())
}

fn homebrew_install() -> Result<bool> {
    if cfg!(target_family = "windows") {
        return Ok(false);
    }

    let current_exe = rv_dirs::canonicalize_utf8(rv_dirs::current_exe()?)?;

    Ok(current_exe.starts_with("/usr/local/Cellar")
        || current_exe.starts_with("/opt/homebrew/Cellar"))
}
