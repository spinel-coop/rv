use miette::{miette, Result};
use miette::{Context, IntoDiagnostic};
use rv_gem_package::Package;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    // Get paths from command line arguments or use default
    let paths = if args.len() > 1 {
        args[1..].iter().map(PathBuf::from).collect()
    } else {
        get_default_gem_cache_paths()?
    };

    println!("Searching for gems in {} path(s):", paths.len());
    for path in &paths {
        println!("  {}", path.display());
    }
    println!();

    let mut total_gems = 0;
    let mut verified_gems = 0;
    let mut failed_gems = 0;
    let mut errors = Vec::new();

    for path in paths {
        if !path.exists() {
            println!("⚠️  Path does not exist: {}", path.display());
            continue;
        }

        println!("📁 Scanning: {}", path.display());

        let gem_files = find_gem_files(&path)?;
        println!("   Found {} .gem files", gem_files.len());

        for gem_file in gem_files {
            total_gems += 1;

            match verify_gem(&gem_file) {
                Ok(info) => {
                    verified_gems += 1;
                    println!("✅ {} v{}", info.name, info.version);
                }
                Err(e) => {
                    failed_gems += 1;
                    let error_msg = format!(
                        "❌ {}: {:?}",
                        gem_file.file_name().unwrap_or_default().to_string_lossy(),
                        e
                    );
                    println!("{}", error_msg);
                    errors.push(error_msg);
                }
            }
        }
        println!();
    }

    // Print summary
    println!("📊 Summary:");
    println!("   Total gems found: {}", total_gems);
    println!("   Successfully verified: {}", verified_gems);
    println!("   Failed verification: {}", failed_gems);

    if !errors.is_empty() {
        println!("\n🔍 Detailed errors:");
        for error in &errors[..errors.len().min(10)] {
            // Show max 10 errors
            println!("   {}", error);
        }
        if errors.len() > 10 {
            println!("   ... and {} more errors", errors.len() - 10);
        }
    }

    if failed_gems > 0 {
        std::process::exit(1);
    }

    Ok(())
}

/// Information about a verified gem
struct GemInfo {
    name: String,
    version: String,
}

/// Verify a single gem file
fn verify_gem(gem_path: &Path) -> Result<GemInfo> {
    let mut package = Package::open(gem_path)?;

    // Get spec info first
    let spec = package.spec()?;

    let gem_info = GemInfo {
        name: spec.name.clone(),
        version: spec.version.to_string(),
    };

    // Verify checksums
    package.verify()?;

    Ok(gem_info)
}

/// Find all .gem files in a directory (non-recursive for performance)
fn find_gem_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut gem_files = Vec::new();

    if dir.is_file() && dir.extension().map_or(false, |ext| ext == "gem") {
        gem_files.push(dir.to_path_buf());
        return Ok(gem_files);
    }

    if !dir.is_dir() {
        return Ok(gem_files);
    }

    for entry in fs::read_dir(dir).into_diagnostic()? {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();

        if path.is_file() && path.extension().map_or(false, |ext| ext == "gem") {
            gem_files.push(path);
        }
    }

    // Sort for consistent output
    gem_files.sort();
    Ok(gem_files)
}

/// Get default gem cache paths from ~/.gem/ruby/*/cache/
fn get_default_gem_cache_paths() -> Result<Vec<PathBuf>> {
    let home_dir = env::var("HOME")
        .or_else(|_| env::var("USERPROFILE"))
        .map_err(|_| miette!("Could not determine home directory"))?;

    let gem_dir = Path::new(&home_dir).join(".gem").join("ruby");

    if !gem_dir.exists() {
        return Ok(vec![]);
    }

    let mut cache_paths = Vec::new();

    // Look for version directories (e.g., 3.0.0, 3.1.0, etc.)
    for entry in fs::read_dir(&gem_dir).into_diagnostic()? {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();

        if path.is_dir() {
            let cache_path = path.join("cache");
            if cache_path.exists() && cache_path.is_dir() {
                cache_paths.push(cache_path);
            }
        }
    }

    // Sort by version (roughly)
    cache_paths.sort();

    // If no cache directories found, provide a helpful message
    if cache_paths.is_empty() {
        eprintln!("No gem cache directories found in {}", gem_dir.display());
        eprintln!("You can specify paths manually as command line arguments.");
        eprintln!(
            "Example: {} /path/to/gems /another/path",
            env::args().next().unwrap_or_default()
        );
    }

    Ok(cache_paths)
}
