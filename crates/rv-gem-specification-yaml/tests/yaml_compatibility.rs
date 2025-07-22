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

#[test]
fn test_parse_version_with_extras_specification() {
    let yaml_content = load_fixture("version_with_extras");
    let result = parse(&yaml_content);

    match result {
        Ok(spec) => {
            insta::assert_debug_snapshot!("version_with_extras_spec_parsed", spec);
        }
        Err(e) => {
            panic!("Failed to parse version with extras specification: {e}");
        }
    }
}

#[test]
fn test_parse_requirement_with_none_specification() {
    let yaml_content = load_fixture("requirement_with_none");
    let result = parse(&yaml_content);

    match result {
        Ok(spec) => {
            insta::assert_debug_snapshot!("requirement_with_none_spec_parsed", spec);
        }
        Err(e) => {
            panic!("Failed to parse requirement with none specification: {e}");
        }
    }
}

#[test]
fn test_parse_old_dependency_format_specification() {
    let yaml_content = load_fixture("old_dependency_format");
    let result = parse(&yaml_content);

    match result {
        Ok(spec) => {
            insta::assert_debug_snapshot!("old_dependency_format_spec_parsed", spec);
        }
        Err(e) => {
            panic!("Failed to parse old dependency format specification: {e}");
        }
    }
}

#[test]
fn test_parse_null_authors_email_specification() {
    let yaml_content = load_fixture("null_authors_email");
    let result = parse(&yaml_content);

    match result {
        Ok(spec) => {
            // Verify semantic null handling
            assert_eq!(spec.authors.len(), 3);
            assert_eq!(spec.authors[0], Some("Real Author".to_string()));
            assert_eq!(spec.authors[1], None);
            assert_eq!(spec.authors[2], Some("Another Author".to_string()));
            
            assert_eq!(spec.email.len(), 3);
            assert_eq!(spec.email[0], Some("real@example.com".to_string()));
            assert_eq!(spec.email[1], None);
            assert_eq!(spec.email[2], Some("another@example.com".to_string()));
            
            insta::assert_debug_snapshot!("null_authors_email_spec_parsed", spec);
        }
        Err(e) => {
            panic!("Failed to parse null authors/email specification: {e}");
        }
    }
}

#[test]
fn test_parse_comprehensive_features_specification() {
    let yaml_content = load_fixture("comprehensive_features");
    let result = parse(&yaml_content);

    match result {
        Ok(spec) => {
            // Verify all features work together
            assert_eq!(spec.name, "comprehensive-test");
            assert_eq!(spec.version.to_string(), "1.0.0");
            assert_eq!(spec.dependencies.len(), 2);
            assert_eq!(spec.dependencies[0].name, "runtime_dep");
            assert_eq!(spec.dependencies[1].name, "old_style_dep");
            assert!(!spec.required_ruby_version.is_latest_version());
            assert!(!spec.required_rubygems_version.is_latest_version());
            
            insta::assert_debug_snapshot!("comprehensive_features_spec_parsed", spec);
        }
        Err(e) => {
            panic!("Failed to parse comprehensive features specification: {e}");
        }
    }
}

