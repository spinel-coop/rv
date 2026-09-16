use camino::Utf8PathBuf;
use ignore::WalkBuilder;
use std::fs::read;
use std::io::BufRead;
use std::path::Path;

/// Expand paths, handling `@file` notation. Returns a flattened list of paths.
///
/// When a path starts with `@`, it's treated as a file containing a list of
/// paths (one per line). All other paths are passed through unchanged.
pub fn expand_paths(include_paths: &[String]) -> Vec<String> {
    let mut expanded = Vec::new();

    for path in include_paths {
        if let Some(file_name) = path.strip_prefix('@') {
            match std::fs::File::open(file_name) {
                Ok(file) => {
                    let buf = std::io::BufReader::new(file);
                    expanded.extend(buf.lines().map(|l| l.expect("Could not parse line")));
                }
                Err(_) => expanded.push(path.clone()),
            }
        } else {
            expanded.push(path.clone());
        }
    }

    expanded
}

/// Discover `.rb` files from the given include paths.
///
/// Returns `(path, source)` pairs for each `.rb` file found. Respects
/// `.gitignore` when `include_gitignored` is `false`. Does not apply
/// `.rubyfmtignore` — that is the caller's responsibility.
///
/// Requires `include_paths` to be non-empty. For stdin handling, the
/// caller should read stdin and pass the buffer directly.
pub fn discover_rb_files(
    include_paths: &[String],
    include_gitignored: bool,
) -> Vec<(Utf8PathBuf, Vec<u8>)> {
    let paths = expand_paths(include_paths);

    let (file_paths, dir_paths): (Vec<_>, Vec<_>) =
        paths.iter().partition(|p| Path::new(p).is_file());

    let mut results: Vec<(Utf8PathBuf, Vec<u8>)> = Vec::new();

    for path in &file_paths {
        if let Ok(buffer) = read(path) {
            results.push((Utf8PathBuf::from(path.as_str()), buffer));
        }
    }

    if !dir_paths.is_empty() {
        let mut builder = WalkBuilder::new(&dir_paths[0]);
        for path in &dir_paths[1..] {
            builder.add(path);
        }
        builder.git_ignore(!include_gitignored);

        for result in builder.build() {
            match result {
                Ok(pp) => {
                    let file_path = pp.path();
                    if file_path.extension().and_then(|e| e.to_str()) == Some("rb") {
                        if let Ok(buffer) = read(file_path) {
                            results.push((file_path.to_path_buf().try_into().unwrap(), buffer));
                        }
                    }
                }
                Err(_) => {}
            }
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn expand_paths_handles_at_file() {
        let dir = tempdir().unwrap();
        let list_file = dir.path().join("paths.txt");
        fs::write(&list_file, "lib/a.rb\nlib/b.rb").unwrap();

        let result = expand_paths(&[format!("@{}", list_file.display())]);
        assert_eq!(result, vec!["lib/a.rb", "lib/b.rb"]);
    }

    #[test]
    fn expand_paths_passes_through_regular_paths() {
        let result = expand_paths(&["lib/foo.rb".to_string(), "spec/bar.rb".to_string()]);
        assert_eq!(result, vec!["lib/foo.rb", "spec/bar.rb"]);
    }

    #[test]
    fn discover_rb_files_finds_rb_files() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("lib")).unwrap();
        fs::write(dir.path().join("lib").join("foo.rb"), "puts 'hello'").unwrap();
        let _ = fs::write(dir.path().join("lib").join("bar.txt"), "not ruby");

        let result = discover_rb_files(&[dir.path().to_string_lossy().to_string()], false);
        let paths: Vec<_> = result.iter().map(|(p, _)| p.to_string()).collect();
        assert!(paths.iter().any(|p| p.ends_with("foo.rb")));
        assert!(!paths.iter().any(|p| p.ends_with("bar.txt")));
    }

    #[test]
    fn discover_rb_files_returns_source_content() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("lib")).unwrap();
        fs::write(dir.path().join("lib").join("foo.rb"), "puts 'hello'").unwrap();

        let result = discover_rb_files(&[dir.path().to_string_lossy().to_string()], false);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].1, b"puts 'hello'");
    }
}
