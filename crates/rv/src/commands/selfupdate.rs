use axoupdater::AxoUpdater;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    AxoupdateError(#[from] axoupdater::AxoupdateError),
}

type Result<T> = miette::Result<T, Error>;

pub async fn selfupdate() -> Result<()> {
    let update = AxoUpdater::new_for("rv").load_receipt()?.run().await?;

    if update.is_some() {
        println!("Update installed!");
    } else {
        println!("rv is already up to date!");
    }

    Ok(())
}
