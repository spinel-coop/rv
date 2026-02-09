#[test]
fn test_parse_file() {
    let input = include_str!("../tests/inputs/Gemfile.tapioca.lock");
    let output = crate::parse(input).unwrap();
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_file_two_sources() {
    let input = include_str!("../tests/inputs/Gemfile.twosources.lock");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_empty_sections() {
    let input = include_str!("../tests/inputs/Gemfile.empty.lock");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_with_checksums() {
    let input = include_str!("../tests/inputs/Gemfile.withchecksums.lock");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_with_paths() {
    let input = include_str!("../tests/inputs/Gemfile.withpath.lock");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_feedyouremail() {
    let input = include_str!("../tests/inputs/Gemfile.feedyouremail.lock");
    let output = must_parse(input);
    assert_eq!(output.dependencies.len(), 52);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_gitlab() {
    let input = include_str!("../tests/inputs/Gemfile.gitlab.lock");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_gemdir() {
    let input = include_str!("../tests/inputs/Gemfile.gemdir.lock");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_git_gem() {
    let input = include_str!("../tests/inputs/Gemfile.git.lock");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_git_rails() {
    let input = include_str!("../tests/inputs/Gemfile.git-rails.lock");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}
#[test]
fn test_parse_discourse() {
    let input = include_str!("../tests/inputs/Gemfile.discourse.lock");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_withoutsource() {
    // If the Gemfile has no declared source, Bundler will default to http://rubygems.org,
    // which provides the endpoints needed to resolve a lockfile successfully, but does not
    // provide the endpoints needed to record checksums. So this lock has empty checksums.
    let input = include_str!("../tests/inputs/Gemfile.withoutsource.lock");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_commit_watcher() {
    let input = include_str!("../tests/inputs/Gemfile.commit-watcher.lock");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_git_ref() {
    // Test parsing GIT sections with a `ref:` field, like from huginn's Gemfile.lock
    // https://github.com/huginn/huginn/blob/master/Gemfile.lock#L51
    let input = include_str!("../tests/inputs/Gemfile.git-ref.lock");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_git_tag() {
    // Test parsing GIT sections with a `tag:` field, like from ekylibre's Gemfile.lock
    // https://github.com/ekylibre/ekylibre/blob/main/Gemfile.lock
    let input = include_str!("../tests/inputs/Gemfile.git-tag.lock");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_lobsters() {
    // Test parsing a lockfile with a Ruby version without patchlevel (e.g., "ruby 4.0.0")
    // https://github.com/lobsters/lobsters/blob/main/Gemfile.lock
    let input = include_str!("../tests/inputs/Gemfile.lobsters.lock");
    let output = must_parse(input);
    assert_eq!(output.ruby_version, Some("ruby 4.0.0"));
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_mastodon() {
    // Test parsing Mastodon's Gemfile.lock (has `ref:` field in GIT section)
    // https://github.com/mastodon/mastodon
    let input = include_str!("../tests/inputs/Gemfile.mastodon.lock");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}

fn must_parse(input: &str) -> crate::datatypes::GemfileDotLock<'_> {
    match crate::parse(input) {
        Ok(o) => o,
        Err(e) => {
            let report = miette::Report::new(e);
            panic!("{report:?}")
        }
    }
}

#[test]
fn test_parse_with_crlf_line_endings() {
    // Take an existing fixture with Unix line endings
    let input_lf = include_str!("../tests/inputs/Gemfile.faker.lock");
    assert!(
        !input_lf.contains("\r\n"),
        "fixture should have Unix line endings"
    );

    // Convert to Windows line endings (CRLF)
    let input_crlf = input_lf.replace('\n', "\r\n");
    assert!(
        input_crlf.contains("\r\n"),
        "converted string should have Windows line endings"
    );

    // Normalize the CRLF input back to LF
    let normalized = crate::normalize_line_endings(&input_crlf);

    // Both should parse identically
    let output_lf = must_parse(input_lf);
    let output_normalized = must_parse(&normalized);

    assert_eq!(
        output_lf.gem_spec_count(),
        output_normalized.gem_spec_count(),
        "gem spec count should match"
    );
    assert_eq!(
        output_lf.ruby_version, output_normalized.ruby_version,
        "ruby version should match"
    );
    assert_eq!(
        output_lf.bundled_with, output_normalized.bundled_with,
        "bundled_with should match"
    );
    assert_eq!(
        output_lf.dependencies.len(),
        output_normalized.dependencies.len(),
        "dependencies count should match"
    );
}

#[test]
fn test_gem_spec_count_multiple_sources() {
    let input = include_str!("../tests/inputs/Gemfile.twosources.lock");
    let lockfile = must_parse(input);

    // twosources.lock has 2 GEM sections with 1 gem each
    assert_eq!(lockfile.gem.len(), 2);
    assert_eq!(lockfile.gem_spec_count(), 2);
}

#[test]
fn test_gem_spec_count_single_source() {
    let input = include_str!("../tests/inputs/Gemfile.faker.lock");
    let lockfile = must_parse(input);

    assert_eq!(lockfile.gem.len(), 1);
    assert_eq!(lockfile.gem_spec_count(), 33);
}

#[test]
fn test_parse_minimal_ruby_project() {
    // This fixture is also used by the Windows CI integration test.
    // It contains only pure-Ruby gems (no native extensions).
    let input = include_str!("../tests/inputs/Gemfile.minimal-ruby-project.lock");
    let lockfile = must_parse(input);

    // Should have rake, rspec, and rspec's dependencies (7 gems total)
    assert_eq!(lockfile.gem_spec_count(), 7);
    assert_eq!(lockfile.dependencies.len(), 2); // rake and rspec

    // Verify key gems are present
    let gem_names: Vec<&str> = lockfile
        .gem
        .iter()
        .flat_map(|g| g.specs.iter().map(|s| s.gem_version.name))
        .collect();
    assert!(gem_names.contains(&"rake"), "should contain rake");
    assert!(gem_names.contains(&"rspec"), "should contain rspec");
    assert!(
        gem_names.contains(&"rspec-core"),
        "should contain rspec-core"
    );
}

#[test]
fn test_platform_specific_spec_count() {
    let input = include_str!("../tests/inputs/Gemfile.testwithnative.lock");

    let lockfile = must_parse(input);

    assert_eq!(lockfile.gem_spec_count(), 2);
    assert_eq!(lockfile.platform_specific_spec_count(), 1);
}

#[test]
fn test_discard_installed_gems() {
    use camino::Utf8PathBuf;
    use std::fs;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let install_path = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();

    // A fake installed gem (We check based on dir, so just a dir with the name is enough)
    let installed_gem_dir = install_path.join("gems").join("rake-13.3.0");
    fs::create_dir_all(&installed_gem_dir).unwrap();

    let input = include_str!("../tests/inputs/Gemfile.twosources.lock");

    let mut lockfile = must_parse(input);

    lockfile.discard_installed_gems(&install_path);

    assert_eq!(lockfile.gem_spec_count(), 1);
    assert_eq!(lockfile.gem[0].specs[0].gem_version.name, "rack");
}

#[test]
fn test_prefer_platform_specific_gems() {
    // Use the real Discourse lockfile fixture which has libv8-node with
    // multiple platform variants (ruby, x86_64-linux, aarch64-linux, etc.)
    let input = include_str!("..//tests/inputs/Gemfile.discourse.lock");
    let lockfile = must_parse(input);

    // Get all specs from the gem sources
    let all_specs: Vec<_> = lockfile
        .gem
        .clone()
        .into_iter()
        .flat_map(|section| section.specs)
        .collect();

    // Get all specs filtered by platform specific
    let result: Vec<_> = lockfile
        .gem
        .clone()
        .into_iter()
        .flat_map(|section| section.platform_specific_gems())
        .collect();

    // Count how many libv8-node variants exist before filtering
    let libv8_before: Vec<_> = all_specs
        .iter()
        .filter(|s| s.gem_version.name == "libv8-node")
        .collect();
    assert!(
        libv8_before.len() > 1,
        "fixture should have multiple libv8-node variants, found {}",
        libv8_before.len()
    );

    // Should only have ONE libv8-node after filtering
    let libv8_after: Vec<_> = result
        .iter()
        .filter(|s| s.gem_version.name == "libv8-node")
        .collect();
    assert_eq!(
        libv8_after.len(),
        1,
        "should have exactly one libv8-node after filtering, found {}",
        libv8_after.len()
    );

    // Verify the correct platform was chosen for the current machine
    let libv8 = libv8_after[0];

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    let expected_version = "24.1.0.0-arm64-darwin";
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    let expected_version = "24.1.0.0-x86_64-darwin";
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    let expected_version = "24.1.0.0-aarch64-linux";
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    let expected_version = "24.1.0.0-x86_64-linux";
    // No Windows-specific libv8-node variant in the fixture, so the generic
    // (ruby platform) variant is selected.
    #[cfg(target_os = "windows")]
    let expected_version = "24.1.0.0";

    assert_eq!(
        libv8.gem_version.version, expected_version,
        "should select platform-specific version for current platform"
    );
}
