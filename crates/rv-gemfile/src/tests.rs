#[test]
fn test_parse_empty_sections() {
    let input = include_str!("../../rv-lockfile/tests/inputs/Gemfile.empty");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_feedyouremail() {
    let input = include_str!("../../rv-lockfile/tests/inputs/Gemfile.feedyouremail");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_git_gem() {
    let input = include_str!("../../rv-lockfile/tests/inputs/Gemfile.git");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn test_parse_git_rails() {
    let input = include_str!("../../rv-lockfile/tests/inputs/Gemfile.git-rails");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}
#[test]
fn test_parse_discourse() {
    let input = include_str!("../../rv-lockfile/tests/inputs/Gemfile.discourse");
    let output = must_parse(input);
    insta::assert_yaml_snapshot!(output);
}

fn must_parse(input: &str) -> crate::datatypes::Gemfile<'_> {
    match crate::parse(input) {
        Ok(o) => o,
        Err(e) => {
            let report = miette::Report::new(e);
            panic!("{report:?}")
        }
    }
}
