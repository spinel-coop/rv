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
fn test_parse_mastodon() {
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
