use crate::config::Config;
use miette::{IntoDiagnostic, Result};
use owo_colors::OwoColorize;
use std::path::PathBuf;

fn rubies_dir(config: &Config) -> &PathBuf {
    config.ruby_dirs.first().unwrap()
}

fn ruby_url(version: &str) -> String {
    format!(
        "https://github.com/spinel-coop/rv-ruby/releases/download/{version}/portable-ruby-{version}.arm64_sonoma.bottle.tar.gz"
    )
}

fn tarball_path(config: &Config, version: &str) -> PathBuf {
    rubies_dir(config).join(format!("portable-ruby-{version}.tar.gz"))
}

async fn download_ruby_tarball(url: &str, tarball_path: &PathBuf) -> Result<()> {
    let response = reqwest::get(url).await.into_diagnostic()?;
    if !response.status().is_success() {
        return Err(miette::miette!(
            "Failed to download {} with status {}",
            url,
            response.status()
        ));
    }
    let tarball = response.bytes().await.into_diagnostic()?;
    // write tarball to tarball_path
    std::fs::write(tarball_path, &tarball).into_diagnostic()?;

    println!(
        "Downloaded {} to {}",
        url.cyan(),
        tarball_path.to_string_lossy().cyan()
    );

    Ok(())
}

async fn extract_ruby_tarball(tarball_path: &PathBuf, rubies_dir: &PathBuf) -> Result<()> {
    let tarball = std::fs::File::open(tarball_path).into_diagnostic()?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(tarball));
    for e in archive.entries().into_diagnostic()? {
        let mut entry = e.into_diagnostic()?;
        let entry_path = entry.path().into_diagnostic()?;
        let path = entry_path
            .strip_prefix("portable-ruby/")
            .into_diagnostic()?
            .to_str()
            .ok_or(miette::miette!("Invalid path in tarball"))?;
        let entry_path = rubies_dir.join(path);
        entry.unpack(entry_path).into_diagnostic()?;
    }

    Ok(())
}

pub async fn install(config: &Config, version: String) -> Result<()> {
    let rubies_dir = rubies_dir(config);
    let url = ruby_url(&version);
    let tarball_path = tarball_path(config, &version);

    if tarball_path.exists() {
        println!(
            "Tarball {} already exists, skipping download.",
            tarball_path.to_string_lossy().cyan()
        );
    } else {
        download_ruby_tarball(&url, &tarball_path).await?;
    }

    extract_ruby_tarball(&tarball_path, rubies_dir).await?;

    println!(
        "Installed Ruby version {} to {}",
        version.cyan(),
        rubies_dir.to_string_lossy().cyan()
    );

    Ok(())
}
