#[test]
fn test_parse_file() {
    let input = include_str!("../tests/inputs/Gemfile.lock.test0");
    let output = crate::parse(input).unwrap();
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_file_two_sources() {
    let input = include_str!("../tests/inputs/Gemfile.lock.twosources");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_empty_sections() {
    let input = include_str!("../tests/inputs/Gemfile.lock.empty");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_with_checksums() {
    let input = include_str!("../tests/inputs/Gemfile.lock.withchecksums");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_with_paths() {
    let input = include_str!("../tests/inputs/Gemfile.lock.withpath");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_feedyouremail() {
    let input = include_str!("../tests/inputs/Gemfile.lock.feedyouremail");
    let output = must_parse(input);
    assert_eq!(output.dependencies.len(), 52);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_gitlab() {
    let input = include_str!("../tests/inputs/Gemfile.lock.gitlab");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_gemdir() {
    let input = include_str!("../tests/inputs/Gemfile.lock.gemdir");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_discourse() {
    let input = include_str!("../tests/inputs/Gemfile.lock.discourse");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_withoutsource() {
    // If the Gemfile has no declared source, Bundler will default to http://rubygems.org,
    // which provides the endpoints needed to resolve a lockfile successfully, but does not
    // provide the endpoints needed to record checksums. So this lock has empty checksums.
    let input = include_str!("../tests/inputs/Gemfile.lock.withoutsource");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_commit_watcher() {
    let input = include_str!("../tests/inputs/Gemfile.lock.commit-watcher");
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
