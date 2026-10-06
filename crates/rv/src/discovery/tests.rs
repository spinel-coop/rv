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
    let files = discover_rb_files(&[path], false, None).unwrap();
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

    let files = discover_rb_files(&[path1, path2], true, None).unwrap();
    assert!(!files.files.is_empty());
}

#[test]
fn discover_rb_files_handles_missing_directory_gracefully() {
    let files = discover_rb_files(&["/nonexistent/directory".to_string()], false, None).unwrap();
    assert!(files.files.is_empty());
}

#[test]
fn is_ruby_file_recognizes_extensions_and_conventional_names() {
    for name in [
        "lib/foo.rb",
        "tasks/db.rake",
        "config.ru",
        "rv.gemspec",
        "sig/foo.rbi",
        "views/index.json.jbuilder",
        "Rakefile",
        "Gemfile",
        "Vagrantfile",
        "Steepfile",
    ] {
        assert!(is_ruby_file(Path::new(name)), "{name} should be Ruby");
    }

    for name in ["README.md", "Gemfile.lock", "src/main.rs", "data.json"] {
        assert!(!is_ruby_file(Path::new(name)), "{name} should not be Ruby");
    }
}

#[test]
fn discover_rb_files_walks_conventional_ruby_filenames_in_directories() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    fs::write(dir.join("Rakefile"), "task :default").unwrap();
    fs::write(dir.join("config.ru"), "run App").unwrap();
    fs::write(dir.join("rv.gemspec"), "Gem::Specification.new").unwrap();
    fs::write(dir.join("README.md"), "# docs").unwrap();

    let files = discover_rb_files(&[dir.to_str().unwrap().to_string()], false, None).unwrap();
    let names = discovered_file_names(&files);

    assert!(names.contains(&"Rakefile".to_string()), "got {names:?}");
    assert!(names.contains(&"config.ru".to_string()), "got {names:?}");
    assert!(names.contains(&"rv.gemspec".to_string()), "got {names:?}");
    assert!(!names.contains(&"README.md".to_string()), "got {names:?}");
}

#[test]
fn discover_rb_files_reads_an_explicitly_named_file_whatever_its_name() {
    let tmp = TempDir::new().unwrap();
    let script = tmp.path().join("bin-script");
    fs::write(&script, "#!/usr/bin/env ruby\nputs 1\n").unwrap();

    let files = discover_rb_files(&[script.to_str().unwrap().to_string()], false, None).unwrap();
    assert_eq!(
        discovered_file_names(&files),
        vec!["bin-script".to_string()]
    );
}

#[test]
fn discover_rb_files_honors_the_custom_ignore_file() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    fs::write(dir.join("keep.rb"), "x = 1").unwrap();
    fs::write(dir.join("skip.rb"), "y = 2").unwrap();
    fs::write(dir.join(RUBYFMT_IGNORE_FILE), "skip.rb\n").unwrap();

    let files = discover_rb_files(
        &[dir.to_str().unwrap().to_string()],
        false,
        Some(RUBYFMT_IGNORE_FILE),
    )
    .unwrap();
    let names = discovered_file_names(&files);

    assert!(names.contains(&"keep.rb".to_string()), "got {names:?}");
    assert!(!names.contains(&"skip.rb".to_string()), "got {names:?}");
}

#[test]
fn discover_rb_files_ignores_the_custom_ignore_file_when_not_requested() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    fs::write(dir.join("keep.rb"), "x = 1").unwrap();
    fs::write(dir.join("skip.rb"), "y = 2").unwrap();
    fs::write(dir.join(RUBYFMT_IGNORE_FILE), "skip.rb\n").unwrap();

    let files = discover_rb_files(&[dir.to_str().unwrap().to_string()], false, None).unwrap();
    let names = discovered_file_names(&files);

    assert!(names.contains(&"keep.rb".to_string()), "got {names:?}");
    assert!(names.contains(&"skip.rb".to_string()), "got {names:?}");
}

fn discovered_file_names(result: &DiscoveryResult) -> Vec<String> {
    result
        .files
        .iter()
        .filter_map(|(path, _)| path.file_name().map(str::to_string))
        .collect()
}
