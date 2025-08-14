use camino::Utf8PathBuf;
use clap::Parser;
use std::io;

use crate::Cache;

#[derive(Parser, Debug, Clone)]
#[command(next_help_heading = "Cache Options")]
pub struct CacheArgs {
    /// Avoid reading from or writing to the cache, instead using a temporary directory for the
    /// duration of the operation.
    #[arg(
        global = true,
        long,
        short,
        value_parser = clap::builder::BoolishValueParser::new(),
        env = "RV_NO_CACHE"
    )]
    pub no_cache: bool,

    /// Path to the cache directory.
    ///
    /// Defaults to platform-specific cache directory or `~/.cache/rv` on Unix systems.
    #[arg(global = true, long, env = "RV_CACHE_DIR")]
    pub cache_dir: Option<Utf8PathBuf>,
}

impl CacheArgs {
    pub fn to_cache(&self) -> io::Result<Cache> {
        self.try_into()
    }
}

impl Cache {
    /// Create a cache from settings, preferring in order:
    ///
    /// 1. A temporary cache directory, if the user requested `--no-cache`.
    /// 2. The specific cache directory specified by the user via `--cache-dir` or `RV_CACHE_DIR`.
    /// 3. The system-appropriate cache directory.
    ///
    /// Returns an absolute cache dir.
    pub fn from_settings(
        no_cache: bool,
        cache_dir: Option<&Utf8PathBuf>,
    ) -> Result<Self, io::Error> {
        if no_cache {
            Self::temp()
        } else if let Some(cache_dir) = cache_dir {
            Ok(Self::from_path(cache_dir))
        } else {
            let cache_dir = rv_dirs::user_cache_dir(camino::Utf8Path::new("/"));
            Ok(Self::from_path(cache_dir))
        }
    }
}

impl TryFrom<&CacheArgs> for Cache {
    type Error = io::Error;

    fn try_from(value: &CacheArgs) -> Result<Self, Self::Error> {
        Cache::from_settings(value.no_cache, value.cache_dir.as_ref())
    }
}

impl TryFrom<CacheArgs> for Cache {
    type Error = io::Error;

    fn try_from(value: CacheArgs) -> Result<Self, Self::Error> {
        (&value).try_into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_cache_from_settings_no_cache() {
        let cache = Cache::from_settings(true, None).unwrap();
        assert!(cache.is_temporary());
    }

    #[test]
    fn test_cache_from_settings_with_cache_dir() {
        let temp_dir = tempdir().unwrap();
        let cache_path = camino::Utf8PathBuf::from(temp_dir.path().to_str().unwrap());

        let cache = Cache::from_settings(false, Some(&cache_path)).unwrap();
        assert!(!cache.is_temporary());
        assert_eq!(cache.root(), cache_path);
    }

    #[test]
    fn test_cache_from_settings_default() {
        let cache = Cache::from_settings(false, None).unwrap();
        assert!(!cache.is_temporary());
        // Should use rv_dirs::user_cache_dir result
        assert!(!cache.root().as_str().is_empty());
    }

    #[test]
    fn test_cache_args_try_from() {
        let temp_dir = tempdir().unwrap();
        let cache_path = camino::Utf8PathBuf::from(temp_dir.path().to_str().unwrap());

        let args = CacheArgs {
            no_cache: false,
            cache_dir: Some(cache_path.clone()),
        };

        let cache: Cache = args.try_into().unwrap();
        assert!(!cache.is_temporary());
        assert_eq!(cache.root(), cache_path);
    }

    #[test]
    fn test_cache_args_try_from_no_cache() {
        let args = CacheArgs {
            no_cache: true,
            cache_dir: None,
        };

        let cache: Cache = args.try_into().unwrap();
        assert!(cache.is_temporary());
    }
}
