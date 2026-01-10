use axoupdater::AxoUpdater;
use miette::{IntoDiagnostic, Result};

pub async fn selfupdate() -> Result<()> {
    let update = AxoUpdater::new_for("rv")
        .load_receipt()?
        .run()
        .await?;

    if update.is_some() {
        println!("Update installed!");
    } else {
        println!("rv is already up to date!");
    }

    Ok(())
}
