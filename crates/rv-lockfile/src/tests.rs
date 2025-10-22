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

fn must_parse(input: &str) -> crate::datatypes::GemfileDotLock<'_> {
    match crate::parse(input) {
        Ok(o) => o,
        Err(e) => {
            let report = miette::Report::new(e);
            panic!("{report:?}")
        }
    }
}
