use miette::{IntoDiagnostic, Result};
use owo_colors::OwoColorize;

use crate::config::Config;

pub async fn install(config: &Config, version: String) -> Result<()> {
    let ruby_dirs = &config.ruby_dirs;
    let ruby_dir = ruby_dirs.first().unwrap();

    println!(
        "Installing Ruby version {} in {}",
        version.cyan(),
        ruby_dir.as_str().cyan()
    );

    // build a URL to download the Ruby version
    let url = format!(
        "https://github.com/spinel-coop/rv-ruby/releases/download/{}/portable-ruby-{}.arm64_sonoma.bottle.tar.gz",
        version, version,
    );

    let tarball_path = ruby_dir
        .join(format!("portable-ruby-{}.tar.gz", version))
        .into_diagnostic()?;

    if tarball_path.exists().into_diagnostic()? {
        println!(
            "Tarball {} already exists, skipping download.",
            tarball_path.as_str().cyan()
        );
    } else {
        // download and untar the Ruby version in `url` into `ruby.path`
        let response = reqwest::get(&url).await.into_diagnostic()?;
        if !response.status().is_success() {
            return Err(miette::miette!(
                "Failed to download Ruby version {}: {}",
                version,
                response.status()
            ));
        }
        let tarball = response.bytes().await.into_diagnostic()?;
        tarball_path
            .create_file()
            .into_diagnostic()?
            .write(&tarball)
            .into_diagnostic()?;

        println!(
            "Downloaded Ruby version {} to {}",
            version.cyan(),
            tarball_path.as_str().cyan()
        );
    }

    let tar_gz = tarball_path.open_file().into_diagnostic()?;
    let tar = flate2::read::GzDecoder::new(tar_gz);
    let mut archive = tar::Archive::new(tar);
    for entry in archive.entries().into_diagnostic()? {
        let mut entry = entry.into_diagnostic()?;
        let path = entry.path().into_diagnostic()?;
        let path_str = path.to_str().unwrap();
        let prefix = format!("portable-ruby/");
        let path_str = path_str.strip_prefix(&prefix).unwrap_or(path_str);
        let path_str = path_str.strip_suffix("/").unwrap_or(path_str);
        let target_path = ruby_dir.join(path_str).into_diagnostic()?;
        println!(
            "Extracting {} to {}",
            path_str.cyan(),
            target_path.as_str().cyan()
        );
        if entry.header().entry_type().is_dir() {
            target_path.create_dir_all().into_diagnostic()?;
        } else {
            target_path.parent().create_dir_all().into_diagnostic()?;
            let mut file = target_path.create_file().into_diagnostic()?;
            std::io::copy(&mut entry, &mut file).into_diagnostic()?;
        }
    }
    println!(
        "Extracted Ruby version {} to {}",
        version.cyan(),
        ruby_dir.as_str().cyan()
    );

    Ok(())
}
