use miette::{IntoDiagnostic, Result};
use owo_colors::OwoColorize;
use rsfs::GenFS;
use std::path::{Path, PathBuf};

use crate::config::Config;

fn rubies_dir<F: GenFS>(config: &Config<F>) -> &PathBuf {
    config.ruby_dirs.first().unwrap()
}

fn ruby_url(version: &str) -> String {
    format!(
        "https://github.com/spinel-coop/rv-ruby/releases/download/{version}/portable-ruby-{version}.arm64_sonoma.bottle.tar.gz"
    )
}

fn tarball_path<F: GenFS>(config: &Config<F>, version: &str) -> Result<PathBuf> {
    Ok(rubies_dir(config).join(format!("portable-ruby-{version}.tar.gz")))
}

async fn download_ruby_tarball<F: GenFS>(fs: &F, url: &str, tarball_path: &Path) -> Result<()> {
    let response = reqwest::get(url).await.into_diagnostic()?;
    if !response.status().is_success() {
        return Err(miette::miette!(
            "Failed to download {} with status {}",
            url,
            response.status()
        ));
    }
    let tarball = response.bytes().await.into_diagnostic()?;
    let mut tarball_file = fs.create_file(tarball_path).into_diagnostic()?;
    std::io::Write::write_all(&mut tarball_file, &tarball).into_diagnostic()?;

    println!(
        "Downloaded {} to {}",
        url.cyan(),
        tarball_path.display().to_string().cyan()
    );

    Ok(())
}

async fn extract_ruby_tarball<F: GenFS>(fs: &F, tarball_path: &Path, rubies_dir: &Path) -> Result<()> {
    let tarball_file = fs.open_file(tarball_path).into_diagnostic()?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(tarball_file));
    for e in archive.entries().into_diagnostic()? {
        let mut entry = e.into_diagnostic()?;
        let entry_path = entry.path().into_diagnostic()?;
        let path = entry_path
            .strip_prefix("portable-ruby/")
            .into_diagnostic()?
            .to_str()
            .ok_or(miette::miette!("Invalid path in tarball"))?;
        let entry_target_path = rubies_dir.join(path);

        // For rsfs compatibility, we need to extract to the actual filesystem for now
        // This is a limitation but necessary for tar extraction
        entry.unpack(&entry_target_path).into_diagnostic()?;
    }

    Ok(())
}

pub async fn install<F: GenFS>(config: &Config<F>, version: String) -> Result<()> {
    let rubies_dir = rubies_dir(config);
    let url = ruby_url(&version);
    let tarball_path = tarball_path(config, &version)?;

    if config.root.metadata(&tarball_path).is_ok() {
        println!(
            "Tarball {} already exists, skipping download.",
            tarball_path.display().to_string().cyan()
        );
    } else {
        download_ruby_tarball(&config.root, &url, &tarball_path).await?;
    }

    extract_ruby_tarball(&config.root, &tarball_path, rubies_dir).await?;

    println!(
        "Installed Ruby version {} to {}",
        version.cyan(),
        rubies_dir.display().to_string().cyan()
    );

    Ok(())
}
