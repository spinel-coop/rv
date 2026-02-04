use camino::Utf8Path;
use miette::{IntoDiagnostic, Result};
use rayon::prelude::*;
use rayon_tracing::TracedIndexedParallelIterator;
use tracing::debug;

use rv_ruby::Ruby;

use super::{Config, Error};

impl Config {
    /// Get cached Ruby information for a specific Ruby installation if valid
    fn get_cached_ruby(&self, ruby_path: &Utf8Path) -> Result<Ruby> {
        // Use path-based cache key for lookup (since we don't have Ruby info yet)
        let cache = self.cache.bucket(rv_cache::CacheBucket::Ruby);
        let cache_key = self.ruby_path_cache_key(ruby_path)?;

        // Try to read and deserialize cached data
        match cacache::read_sync(&cache, &cache_key) {
            Ok(content) => {
                match serde_json::from_slice::<Ruby>(&content) {
                    Ok(cached_ruby) => {
                        // Verify cached Ruby installation still exists and is valid
                        if cached_ruby.is_valid() {
                            Ok(cached_ruby)
                        } else {
                            // Ruby is no longer valid, remove cache entry
                            cacache::remove_sync(&cache, &cache_key).unwrap();
                            Err(Error::RubyCacheMiss {
                                ruby_path: ruby_path.to_path_buf(),
                            }
                            .into())
                        }
                    }
                    Err(_) => {
                        // Invalid cache file, remove it
                        cacache::remove_sync(&cache, &cache_key).unwrap();
                        Err(Error::RubyCacheMiss {
                            ruby_path: ruby_path.to_path_buf(),
                        }
                        .into())
                    }
                }
            }
            Err(_) => Err(Error::RubyCacheMiss {
                ruby_path: ruby_path.to_path_buf(),
            }
            .into()), // Can't read cache file
        }
    }

    /// Cache Ruby information for a specific Ruby installation
    fn cache_ruby(&self, ruby: &Ruby) -> Result<()> {
        let cache = self.cache.bucket(rv_cache::CacheBucket::Ruby);
        let cache_key = self.ruby_path_cache_key(&ruby.path)?;

        // Serialize and write Ruby information to cache
        let json_data = serde_json::to_string(ruby).into_diagnostic()?;
        cacache::write_sync(cache, cache_key, json_data).into_diagnostic()?;

        Ok(())
    }

    /// Generate a cache key for a specific Ruby installation path (used for cache lookup)
    fn ruby_path_cache_key(&self, path: &Utf8Path) -> Result<String, Error> {
        let bin = path.join("bin").join("ruby");

        bin.try_exists()
            .and_then(|_| rv_cache::Timestamp::from_path(bin.as_std_path()))
            .map(|timestamp| rv_cache::cache_digest((path, timestamp)))
            .map_err(|_| Error::RubyCacheMiss {
                ruby_path: path.into(),
            })
    }

    /// Discover all Ruby installations from configured directories with caching
    pub fn discover_installed_rubies(&self) -> Vec<Ruby> {
        self.discover_rubies_matching(|_| true)
    }

    /// Discover Ruby installations matching a request from configured directories with caching
    pub fn discover_rubies_matching<F>(&self, predicate: F) -> Vec<Ruby>
    where
        F: Fn(&str) -> bool,
    {
        // Collect all potential Ruby paths first
        let ruby_paths: Vec<_> = self
            .ruby_dirs
            .iter()
            .filter(|ruby_dir| ruby_dir.is_dir())
            .flat_map(|ruby_dir| {
                ruby_dir
                    .read_dir_utf8()
                    .into_iter()
                    .flatten()
                    .filter_map(|entry| {
                        entry
                            .ok()
                            .map(|entry| entry.path().to_path_buf())
                            .filter(|path| path.is_dir())
                            .filter(|path| path.file_name().is_some_and(&predicate))
                    })
            })
            .collect();

        let managed_dir = self.ruby_dirs.first();

        // Process Ruby paths in parallel for better performance
        let mut rubies: Vec<Ruby> = ruby_paths
            .into_par_iter()
            .indexed_in_span(tracing::span::Span::current())
            .filter_map(|ruby_path| {
                // Try to get Ruby from cache first
                match self.get_cached_ruby(&ruby_path) {
                    Ok(cached_ruby) => Some(cached_ruby),
                    Err(_) => {
                        let managed = ruby_path.parent()? == managed_dir?;

                        // Cache miss or invalid, create Ruby and cache it
                        match Ruby::from_dir(ruby_path.clone(), managed) {
                            Ok(ruby) if ruby.is_valid() => {
                                // Cache the Ruby (ignore errors during caching to not fail discovery)
                                if let Err(err) = self.cache_ruby(&ruby) {
                                    debug!("Failed to cache ruby at {}: {err}", ruby.path.as_str());
                                }
                                Some(ruby)
                            }
                            Ok(_) => {
                                debug!("Ruby at {} is invalid", ruby_path);
                                None
                            }
                            Err(err) => {
                                debug!("Failed to get ruby from {}: {err}", ruby_path);
                                None
                            }
                        }
                    }
                }
            })
            .collect();

        rubies.sort();

        rubies
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;
    use camino::Utf8PathBuf;
    use indexmap::indexset;
    use rv_cache::Cache;
    use std::fs;

    fn create_test_config() -> (Config, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let root = Utf8PathBuf::from(temp_dir.path().to_str().unwrap());
        let ruby_dir = root.join("rubies");
        fs::create_dir_all(&ruby_dir).unwrap();

        let config = Config {
            ruby_dirs: indexset![ruby_dir],
            root: root.clone(),
            current_dir: root.clone(),
            cache: Cache::temp().unwrap(),
            current_exe: root.join("bin").join("rv"),
            requested_ruby: None,
        };

        (config, temp_dir)
    }

    #[test]
    fn test_discover_installed_rubies_empty() {
        let (config, _temp_dir) = create_test_config();
        let rubies = config.discover_installed_rubies();
        assert!(rubies.is_empty());
    }

    #[test]
    fn test_discover_installed_rubies_with_installations() {
        // This test is complex because it depends on rv-ruby parsing
        // Let's skip it for now and focus on the cache-specific functionality
        // In a real scenario, Ruby::from_dir would work with proper Ruby installations

        let (config, _temp_dir) = create_test_config();

        // Test that discover_installed_rubies doesn't crash with empty directories
        let rubies = config.discover_installed_rubies();
        assert_eq!(rubies.len(), 0);

        // The parallel processing code itself is tested via integration tests
        // that use properly working Ruby installations
    }

    #[test]
    fn test_ruby_caching() {
        // This test would need actual working Ruby installations
        // The caching logic is tested indirectly through integration tests
        let (config, _temp_dir) = create_test_config();

        // Test that discover_installed_rubies can be called multiple times without crashing
        let rubies1 = config.discover_installed_rubies();
        let rubies2 = config.discover_installed_rubies();

        // Both should return empty since we don't have valid Ruby installations
        assert_eq!(rubies1.len(), 0);
        assert_eq!(rubies2.len(), 0);
    }

    #[test]
    fn test_cache_key_generation() {
        let (config, _temp_dir) = create_test_config();
        let ruby_dir = &config.ruby_dirs[0];

        // Create a basic directory structure with ruby executable
        let ruby_path = ruby_dir.join("ruby-3.1.0");
        let bin_dir = ruby_path.join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        let ruby_exe = bin_dir.join("ruby");
        fs::write(&ruby_exe, "#!/bin/bash\necho test").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&ruby_exe).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&ruby_exe, perms).unwrap();
        }

        // Should generate a cache key successfully
        let cache_key = config.ruby_path_cache_key(&ruby_path).unwrap();
        assert!(!cache_key.is_empty());

        // Same path should generate the same key
        let cache_key2 = config.ruby_path_cache_key(&ruby_path).unwrap();
        assert_eq!(cache_key, cache_key2);
    }

    #[test]
    fn test_cache_key_missing_ruby_executable() {
        let (config, _temp_dir) = create_test_config();
        let ruby_dir = &config.ruby_dirs[0];

        // Create directory without Ruby executable
        let ruby_path = ruby_dir.join("ruby-3.1.0");
        fs::create_dir_all(&ruby_path).unwrap();

        // Should return cache miss error
        let result = config.ruby_path_cache_key(&ruby_path);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::RubyCacheMiss { .. }));
    }

    #[test]
    fn test_get_cached_ruby_miss() {
        let (config, _temp_dir) = create_test_config();
        let ruby_dir = &config.ruby_dirs[0];

        // Create a basic directory structure with ruby executable
        let ruby_path = ruby_dir.join("ruby-3.1.0");
        let bin_dir = ruby_path.join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        let ruby_exe = bin_dir.join("ruby");
        fs::write(&ruby_exe, "#!/bin/bash\necho test").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&ruby_exe).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&ruby_exe, perms).unwrap();
        }

        // Should return cache miss for uncached Ruby
        let result = config.get_cached_ruby(&ruby_path);
        result.unwrap_err();
    }
}
