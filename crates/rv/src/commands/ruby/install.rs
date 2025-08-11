use camino::{Utf8Path, Utf8PathBuf};
use miette::{IntoDiagnostic, Result};
use owo_colors::OwoColorize;
use rv_dirs::user_cache_dir;

use crate::config::Config;
use rv_ruby::request::VersionRequest;

pub async fn install(config: &Config, requested: VersionRequest) -> Result<()> {
    let rubies_dir = rubies_dir(config);

    if requested.patch.is_none() {
        return Err(miette::miette!(
            "Major, minor, and patch version is required for Ruby installation"
        ));
    }
    let url = ruby_url(&requested.to_string());
    let tarball_path = tarball_path(config, &requested.to_string());

    if tarball_path.exists() {
        println!(
            "Tarball {} already exists, skipping download.",
            tarball_path.cyan()
        );
    } else {
        download_ruby_tarball(&url, &tarball_path).await?;
    }

    extract_ruby_tarball(&tarball_path, rubies_dir).await?;

    println!(
        "Installed Ruby version {} to {}",
        requested.to_string().cyan(),
        rubies_dir.cyan()
    );

    Ok(())
}

fn rubies_dir(config: &Config) -> &Utf8PathBuf {
    config.ruby_dirs.first().unwrap()
}

fn ruby_url(version: &str) -> String {
    format!(
        "https://github.com/spinel-coop/rv-ruby/releases/download/{version}/{version}.arm64_sonoma.bottle.tar.gz"
    )
}

fn tarball_path(config: &Config, version: &str) -> Utf8PathBuf {
    user_cache_dir(&config.root).join(format!("rubies/{version}.tar.gz"))
}

async fn download_ruby_tarball(url: &str, tarball_path: &Utf8PathBuf) -> Result<()> {
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

    println!("Downloaded {} to {}", url.cyan(), tarball_path.cyan());

    Ok(())
}

async fn extract_ruby_tarball(tarball_path: &Utf8Path, rubies_dir: &Utf8Path) -> Result<()> {
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
