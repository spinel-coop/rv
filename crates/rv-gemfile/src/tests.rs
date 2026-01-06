#[test]
fn test_parse_file() {
    let input = include_str!("../tests/inputs/Gemfile.tapioca");
    let output = crate::parse(input).unwrap();
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_file_two_sources() {
    let input = include_str!("../tests/inputs/Gemfile.twosources");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_empty_sections() {
    let input = include_str!("../tests/inputs/Gemfile.empty");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_with_checksums() {
    let input = include_str!("../tests/inputs/Gemfile.withchecksums");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_with_paths() {
    let input = include_str!("../tests/inputs/Gemfile.withpath");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_feedyouremail() {
    let input = include_str!("../tests/inputs/Gemfile.feedyouremail");
    let output = must_parse(input);
    assert_eq!(output.dependencies.len(), 52);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_gitlab() {
    let input = include_str!("../tests/inputs/Gemfile.gitlab");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_gemdir() {
    let input = include_str!("../tests/inputs/Gemfile.gemdir");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_git_gem() {
    let input = include_str!("../tests/inputs/Gemfile.git");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_git_rails() {
    let input = include_str!("../tests/inputs/Gemfile.git-rails");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}
#[test]
fn test_parse_discourse() {
    let input = include_str!("../tests/inputs/Gemfile.discourse");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_withoutsource() {
    // If the Gemfile has no declared source, Bundler will default to http://rubygems.org,
    // which provides the endpoints needed to resolve a lockfile successfully, but does not
    // provide the endpoints needed to record checksums. So this lock has empty checksums.
    let input = include_str!("../tests/inputs/Gemfile.withoutsource");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_commit_watcher() {
    let input = include_str!("../tests/inputs/Gemfile.commit-watcher");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_git_ref() {
    // Test parsing GIT sections with a `ref:` field, like from huginn's Gemfile.lock
    // https://github.com/huginn/huginn/blob/master/Gemfile.lock#L51
    let input = include_str!("../tests/inputs/Gemfile.git-ref");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_git_tag() {
    // Test parsing GIT sections with a `tag:` field, like from ekylibre's Gemfile.lock
    // https://github.com/ekylibre/ekylibre/blob/main/Gemfile.lock
    let input = include_str!("../tests/inputs/Gemfile.git-tag");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_lobsters() {
    // Test parsing a lockfile with a Ruby version without patchlevel (e.g., "ruby 4.0.0")
    // https://github.com/lobsters/lobsters/blob/main/Gemfile.lock
    let input = include_str!("../tests/inputs/Gemfile.lobsters");
    let output = must_parse(input);
    assert_eq!(output.ruby_version, Some("ruby 4.0.0"));
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
