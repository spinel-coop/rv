use camino::Utf8PathBuf;
use ignore::WalkBuilder;
use std::path::Path;

fn path_buf_to_utf8_path(path: std::path::PathBuf) -> Result<Utf8PathBuf, std::path::PathBuf> {
    Utf8PathBuf::from_path_buf(path)
}

#[derive(Debug)]
pub struct DiscoveryError {
    pub(crate) path: String,
}

impl std::fmt::Display for DiscoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Discovery error for path: {}", self.path)
    }
}

impl std::error::Error for DiscoveryError {}

#[derive(Debug)]
pub(crate) struct DiscoverableFileFailure {
    pub(crate) path: String,
    pub(crate) error: std::io::Error,
}

#[derive(Debug, Default)]
pub(crate) struct DiscoveryResult {
    pub(crate) files: Vec<(Utf8PathBuf, Vec<u8>)>,
    pub(crate) failures: Vec<DiscoverableFileFailure>,
}

pub(crate) fn expand_paths(paths: &[String]) -> Result<Vec<String>, DiscoveryError> {
    let mut expanded = Vec::with_capacity(paths.len());

    for path in paths {
        if let Some(file_path) = path.strip_prefix('@') {
            let content = std::fs::read_to_string(file_path).map_err(|_| DiscoveryError {
                path: file_path.to_string(),
            })?;
            for line in content.lines() {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    expanded.push(trimmed.to_string());
                }
            }
        } else {
            expanded.push(path.clone());
        }
    }

    Ok(expanded)
}

pub(crate) fn discover_rb_files(
    include_paths: &[String],
    include_gitignored: bool,
) -> Result<DiscoveryResult, DiscoveryError> {
    let mut result = DiscoveryResult::default();

    for path in include_paths {
        let walk = WalkBuilder::new(Path::new(path))
            .git_ignore(!include_gitignored)
            .ignore(!include_gitignored)
            .build();

        for entry in walk {
            match entry {
                Ok(dir_entry) => {
                    let entry_path = dir_entry.path();
                    let extension_is_rb = entry_path.extension().is_some_and(|ext| ext == "rb");
                    if extension_is_rb {
                        if let Some(path_str) = entry_path.to_str() {
                            if let Ok(utf8_path) = path_buf_to_utf8_path(entry_path.to_path_buf()) {
                                match std::fs::read(&utf8_path) {
                                    Ok(bytes) => result.files.push((utf8_path, bytes)),
                                    Err(e) => {
                                        result.failures.push(DiscoverableFileFailure {
                                            path: path_str.to_string(),
                                            error: e,
                                        });
                                    }
                                }
                            } else {
                                result.failures.push(DiscoverableFileFailure {
                                    path: path_str.to_string(),
                                    error: std::io::Error::new(
                                        std::io::ErrorKind::InvalidData,
                                        "path is not valid UTF-8",
                                    ),
                                });
                            }
                        } else {
                            result.failures.push(DiscoverableFileFailure {
                                path: entry_path.to_string_lossy().to_string(),
                                error: std::io::Error::new(
                                    std::io::ErrorKind::InvalidData,
                                    "path is not valid UTF-8",
                                ),
                            });
                        }
                    }
                }
                Err(e) => {
                    result.failures.push(DiscoverableFileFailure {
                        path: path.clone(),
                        error: std::io::Error::other(e.to_string()),
                    });
                }
            }
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::{NamedTempFile, TempDir};

    #[test]
    fn expand_paths_passes_through_non_at_paths() {
        let paths = vec!["lib/foo.rb".to_string(), "dir/".to_string()];
        let expanded = expand_paths(&paths).unwrap();
        assert_eq!(expanded, paths);
    }

    #[test]
    fn expand_paths_reads_from_at_file() {
        let tmp = NamedTempFile::new().unwrap();
        fs::write(&tmp, "lib/a.rb\nlib/b.rb\n").unwrap();
        let path = format!("@{}", tmp.path().to_str().unwrap());
        let expanded = expand_paths(&[path]).unwrap();
        assert_eq!(expanded, vec!["lib/a.rb", "lib/b.rb"]);
    }

    #[test]
    fn expand_paths_errors_on_missing_at_file() {
        let paths = vec!["@nonexistent.txt".to_string()];
        let result = expand_paths(&paths);
        assert!(result.is_err());
    }

    #[test]
    fn expand_paths_errors_on_invalid_utf8_in_at_file() {
        let tmp = NamedTempFile::new().unwrap();
        fs::write(tmp.path(), b"lib/a.rb\n\xff\xfe").unwrap();
        let path = format!("@{}", tmp.path().to_str().unwrap());
        assert!(expand_paths(&[path]).is_err());
    }

    #[test]
    fn discover_rb_files_accepts_a_single_path() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.rb"), "x = 1").unwrap();
        let path = tmp.path().to_str().unwrap().to_string();
        let files = discover_rb_files(&[path], false).unwrap();
        assert!(!files.files.is_empty());
    }

    #[test]
    fn discover_rb_files_accepts_multiple_paths() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.rb"), "x = 1").unwrap();
        let path1 = tmp.path().to_str().unwrap().to_string();
        let path2 = tmp.path().join("subdir");
        fs::create_dir_all(&path2).unwrap();
        fs::write(path2.join("b.rb"), "y = 2").unwrap();
        let path2 = path2.to_str().unwrap().to_string();

        let files = discover_rb_files(&[path1, path2], true).unwrap();
        assert!(!files.files.is_empty());
    }

    #[test]
    fn discover_rb_files_handles_missing_directory_gracefully() {
        let files = discover_rb_files(&["/nonexistent/directory".to_string()], false).unwrap();
        assert!(files.files.is_empty());
    }
}
