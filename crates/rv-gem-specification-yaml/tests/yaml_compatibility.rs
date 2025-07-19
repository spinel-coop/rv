use rv_gem_specification_yaml::parse;
use std::fs;

fn load_fixture(name: &str) -> String {
    let fixture_path = format!("tests/fixtures/{name}.yaml");
    fs::read_to_string(&fixture_path)
        .unwrap_or_else(|_| panic!("Failed to read fixture: {fixture_path}"))
}

#[test]
fn test_parse_simple_specification() {
    let yaml_content = load_fixture("simple_spec");
    let result = parse(&yaml_content);

    match result {
        Ok(spec) => {
            insta::assert_debug_snapshot!("simple_spec_parsed", spec);
        }
        Err(e) => {
            panic!("Failed to parse simple specification: {e}");
        }
    }
}

#[test]
fn test_parse_complex_specification() {
    let yaml_content = load_fixture("complex_spec");
    let result = parse(&yaml_content);

    match result {
        Ok(spec) => {
            insta::assert_debug_snapshot!("complex_spec_parsed", spec);
        }
        Err(e) => {
            panic!("Failed to parse complex specification: {e}");
        }
    }
}

#[test]
fn test_parse_version_constraints_specification() {
    let yaml_content = load_fixture("version_constraints_spec");
    let result = parse(&yaml_content);

    match result {
        Ok(spec) => {
            insta::assert_debug_snapshot!("version_constraints_spec_parsed", spec);
        }
        Err(e) => {
            panic!("Failed to parse version constraints specification: {e}");
        }
    }
}

#[test]
fn test_parse_minimal_specification() {
    let yaml_content = load_fixture("minimal_spec");
    let result = parse(&yaml_content);

    match result {
        Ok(spec) => {
            insta::assert_debug_snapshot!("minimal_spec_parsed", spec);
        }
        Err(e) => {
            panic!("Failed to parse minimal specification: {e}");
        }
    }
}

#[test]
fn test_parse_prerelease_specification() {
    let yaml_content = load_fixture("prerelease_spec");
    let result = parse(&yaml_content);

    match result {
        Ok(spec) => {
            insta::assert_debug_snapshot!("prerelease_spec_parsed", spec);
        }
        Err(e) => {
            panic!("Failed to parse prerelease specification: {e}");
        }
    }
}

#[test]
fn test_parse_licensed_specification() {
    let yaml_content = load_fixture("licensed_spec");
    let result = parse(&yaml_content);

    match result {
        Ok(spec) => {
            insta::assert_debug_snapshot!("licensed_spec_parsed", spec);
        }
        Err(e) => {
            panic!("Failed to parse licensed specification: {e}");
        }
    }
}

#[test]
fn test_parse_edge_case_specification() {
    let yaml_content = load_fixture("edge_case_spec");
    let result = parse(&yaml_content);

    match result {
        Ok(spec) => {
            insta::assert_debug_snapshot!("edge_case_spec_parsed", spec);
        }
        Err(e) => {
            panic!("Failed to parse edge case specification: {e}");
        }
    }
}
