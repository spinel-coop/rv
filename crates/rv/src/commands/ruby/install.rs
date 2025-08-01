use miette::{IntoDiagnostic, Result};
use owo_colors::OwoColorize;
use std::path::PathBuf;

use crate::config::Config;

fn rubies_dir(config: &Config) -> &PathBuf {
    config.ruby_dirs.first().unwrap()
}

fn ruby_url(version: &str) -> String {
    format!(
        "https://github.com/spinel-coop/rv-ruby/releases/download/{version}/portable-ruby-{version}.arm64_sonoma.bottle.tar.gz"
    )
}

fn tarball_path(config: &Config, version: &str) -> Result<PathBuf> {
    Ok(rubies_dir(config).join(format!("portable-ruby-{version}.tar.gz")))
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

    // Create parent directories if they don't exist
    if let Some(parent) = tarball_path.parent() {
        std::fs::create_dir_all(parent).into_diagnostic()?;
    }

    std::fs::write(tarball_path, &tarball).into_diagnostic()?;

    println!(
        "Downloaded {} to {}",
        url.cyan(),
        tarball_path.display().to_string().cyan()
    );

    Ok(())
}

async fn extract_ruby_tarball(tarball_path: &PathBuf, rubies_dir: &PathBuf) -> Result<()> {
    let tarball_file = std::fs::File::open(tarball_path).into_diagnostic()?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(tarball_file));

    // Create the rubies directory if it doesn't exist
    std::fs::create_dir_all(rubies_dir).into_diagnostic()?;

    for e in archive.entries().into_diagnostic()? {
        let mut entry = e.into_diagnostic()?;
        let entry_path = entry.path().into_diagnostic()?;
        let path = entry_path
            .strip_prefix("portable-ruby/")
            .into_diagnostic()?;
        let target_path = rubies_dir.join(path);

        // Ensure parent directory exists
        if let Some(parent) = target_path.parent() {
            std::fs::create_dir_all(parent).into_diagnostic()?;
        }

        entry.unpack(&target_path).into_diagnostic()?;
    }

    Ok(())
}

pub async fn install(config: &Config, version: String) -> Result<()> {
    let rubies_dir = rubies_dir(config);
    let url = ruby_url(&version);
    let tarball_path = tarball_path(config, &version)?;

    if tarball_path.exists() {
        println!(
            "Tarball {} already exists, skipping download.",
            tarball_path.display().to_string().cyan()
        );
    } else {
        download_ruby_tarball(&url, &tarball_path).await?;
    }

    extract_ruby_tarball(&tarball_path, rubies_dir).await?;

    println!(
        "Installed Ruby version {} to {}",
        version.cyan(),
        rubies_dir.display().to_string().cyan()
    );

    Ok(())
}
