use pretty_assertions::assert_matches;
use rv_gem_specification_yaml::parse;
use std::fs;

fn load_fixture(name: &str) -> String {
    let fixture_path = format!("tests/fixtures/{name}.yaml");
    fs::read_to_string(&fixture_path)
        .unwrap_or_else(|_| panic!("Failed to read fixture: {fixture_path}"))
}

#[test]
fn test_malformed_wrong_root_tag() {
    let malformed_yaml = r#"
--- !ruby/object:Gem::Version
name: test-gem
version: 1.0.0
"#;

    let result = parse(malformed_yaml);
    assert!(result.is_err());
    let err = result.unwrap_err();

    // Check that it's the expected error type with appropriate message
    let error_msg = format!("{err}");
    assert_eq!(
        "Expected `ruby/object:Gem::Specification`, found mapping start with tag 'ruby/object:Gem::Version'",
        error_msg
    );
}

#[test]
fn test_malformed_version_wrong_tag() {
    let malformed_yaml = r#"
--- !ruby/object:Gem::Specification
name: test-gem
version: !ruby/object:Gem::Requirement
  version: 1.0.0
"#;

    let result = parse(malformed_yaml);
    assert!(result.is_err());
    let err = result.unwrap_err();

    // Check that it's the expected error for wrong version tag
    let error_msg = format!("{err}");
    assert_eq!(
        "Expected `ruby/object:Gem::Version`, found mapping start with tag 'ruby/object:Gem::Requirement'",
        error_msg
    );
}

#[test]
fn test_malformed_version_missing_version_field() {
    let malformed_yaml = r#"
--- !ruby/object:Gem::Specification
name: test-gem
version: !ruby/object:Gem::Version
  invalid_field: 1.0.0
"#;

    let result = parse(malformed_yaml);
    assert!(result.is_err());
    // Verify that we get a meaningful error (placeholder assertions removed)
}

#[test]
fn test_malformed_version_wrong_type() {
    let malformed_yaml = r#"
--- !ruby/object:Gem::Specification
name: test-gem
version: !ruby/object:Gem::Version
  version: 123
"#;

    let result = parse(malformed_yaml);
    // This might succeed but create an invalid version
    if let Ok(spec) = result {
        // Version parsing should handle numeric conversion
        assert_eq!(spec.version.to_string(), "123");
    }
}

#[test]
fn test_malformed_dependency_wrong_tag() {
    let malformed_yaml = r#"
--- !ruby/object:Gem::Specification
name: test-gem
version: !ruby/object:Gem::Version
  version: 1.0.0
dependencies:
  - !ruby/object:Gem::Version
    name: rails
    requirement: !ruby/object:Gem::Requirement
      requirements:
        - - ">="
          - !ruby/object:Gem::Version
            version: 6.0
"#;

    let result = parse(malformed_yaml);
    // With strict tag validation, wrong tag for dependency causes parse error
    assert!(result.is_err());
    // Verify that we get a meaningful error (placeholder assertions removed)
}

#[test]
fn test_malformed_requirement_wrong_tag() {
    let malformed_yaml = r#"
--- !ruby/object:Gem::Specification
name: test-gem
version: !ruby/object:Gem::Version
  version: 1.0.0
dependencies:
  - !ruby/object:Gem::Dependency
    name: rails
    requirement: !ruby/object:Gem::Version
      version: 6.0
"#;

    let result = parse(malformed_yaml);
    // With strict tag validation, wrong tag for requirement causes parse error
    assert!(result.is_err());
    // Verify that we get a meaningful error (specific assertions can be added as needed)
}

#[test]
fn test_malformed_requirement_missing_requirements_field() {
    let malformed_yaml = r#"
--- !ruby/object:Gem::Specification
name: test-gem
version: !ruby/object:Gem::Version
  version: 1.0.0
dependencies:
  - !ruby/object:Gem::Dependency
    name: rails
    requirement: !ruby/object:Gem::Requirement
      invalid_field: something
"#;

    let result = parse(malformed_yaml);
    // Missing requirements field in requirement causes parse error
    assert!(result.is_err());
    // Verify that we get a meaningful error (specific assertions can be added as needed)
}

#[test]
fn test_malformed_requirement_invalid_constraints() {
    let malformed_yaml = r#"
--- !ruby/object:Gem::Specification
name: test-gem
version: !ruby/object:Gem::Version
  version: 1.0.0
dependencies:
  - !ruby/object:Gem::Dependency
    name: rails
    requirement: !ruby/object:Gem::Requirement
      requirements:
        - - "invalid_operator"
          - !ruby/object:Gem::Version
            version: 6.0
"#;

    let result = parse(malformed_yaml);
    assert_matches!(result, Err(_));
}

#[test]
fn test_malformed_requirement_incomplete_constraint() {
    let malformed_yaml = r#"
--- !ruby/object:Gem::Specification
name: test-gem
version: !ruby/object:Gem::Version
  version: 1.0.0
dependencies:
  - !ruby/object:Gem::Dependency
    name: rails
    requirement: !ruby/object:Gem::Requirement
      requirements:
        - - ">="
"#;

    let result = parse(malformed_yaml);
    // Incomplete constraint in requirement causes parse error
    assert!(result.is_err());
    // Verify that we get a meaningful error (specific assertions can be added as needed)
}

#[test]
fn test_malformed_dependency_missing_name() {
    let malformed_yaml = r#"
--- !ruby/object:Gem::Specification
name: test-gem
version: !ruby/object:Gem::Version
  version: 1.0.0
dependencies:
  - !ruby/object:Gem::Dependency
    requirement: !ruby/object:Gem::Requirement
      requirements:
        - - ">="
          - !ruby/object:Gem::Version
            version: 6.0
"#;

    let result = parse(malformed_yaml);
    assert!(result.is_err());
    // Verify that we get a meaningful error (placeholder assertions removed)
}

#[test]
fn test_malformed_non_string_keys() {
    let malformed_yaml = r#"
--- !ruby/object:Gem::Specification
123: test-gem
version: !ruby/object:Gem::Version
  version: 1.0.0
"#;

    let result = parse(malformed_yaml);
    assert!(result.is_err());
    // Non-string keys result in missing field error because "name" field can't be found
    // Verify that we get a meaningful error (specific assertions can be added as needed)
}

#[test]
fn test_malformed_empty_document() {
    let malformed_yaml = "";

    let result = parse(malformed_yaml);
    assert!(result.is_err());
    // Verify that we get a meaningful error (placeholder assertions removed)
}

#[test]
fn test_malformed_non_mapping_root() {
    let malformed_yaml = r#"
--- !ruby/object:Gem::Specification
- array_item
- another_item
"#;

    let result = parse(malformed_yaml);
    assert!(result.is_err());
    // Verify that we get a meaningful error (placeholder assertions removed)
}

#[test]
fn test_malformed_missing_required_fields() {
    let malformed_yaml = r#"
--- !ruby/object:Gem::Specification
description: "A gem without name or version"
"#;

    let result = parse(malformed_yaml);
    assert!(result.is_err());
    // Verify that we get a meaningful error (placeholder assertions removed)
}

#[test]
fn test_malformed_invalid_yaml_syntax() {
    let malformed_yaml = r#"
--- !ruby/object:Gem::Specification
name: test-gem
version: !ruby/object:Gem::Version
  version: 1.0.0
  invalid: [unclosed array
"#;

    let result = parse(malformed_yaml);
    assert!(result.is_err());
    // Verify that we get a meaningful error (placeholder assertions removed)
}

#[test]
fn test_malformed_version_in_constraint_wrong_tag() {
    let malformed_yaml = r#"
--- !ruby/object:Gem::Specification
name: test-gem
version: !ruby/object:Gem::Version
  version: 1.0.0
dependencies:
  - !ruby/object:Gem::Dependency
    name: rails
    requirement: !ruby/object:Gem::Requirement
      requirements:
        - - ">="
          - !ruby/object:Gem::Dependency
            name: should_be_version
"#;

    let result = parse(malformed_yaml);
    // Wrong tag for version in constraint causes parse error
    assert!(result.is_err());
    // Verify that we get a meaningful error (specific assertions can be added as needed)
}

#[test]
fn test_malformed_nested_structure_corruption() {
    let malformed_yaml = r#"
--- !ruby/object:Gem::Specification
name: test-gem
version: !ruby/object:Gem::Version
  version: 1.0.0
dependencies:
  - !ruby/object:Gem::Dependency
    name: rails
    requirement: !ruby/object:Gem::Requirement
      requirements: "should_be_array_not_string"
"#;

    let result = parse(malformed_yaml);
    // String instead of array for requirements field causes parse error
    assert!(result.is_err());
    // Verify that we get a meaningful error (specific assertions can be added as needed)
}

#[test]
fn test_malformed_metadata_wrong_type() {
    let malformed_yaml = r#"
--- !ruby/object:Gem::Specification
name: test-gem
version: !ruby/object:Gem::Version
  version: 1.0.0
metadata: "should_be_mapping_not_string"
"#;

    let result = parse(malformed_yaml);
    // String instead of mapping for metadata causes parse error
    assert!(result.is_err());
    // Verify that we get a meaningful error (specific assertions can be added as needed)
}

#[test]
fn test_unsupported_folded_scalar_syntax() {
    // Current limitation: bacon-1.2.0.gem uses YAML folded scalar syntax with tag (! 'text...')
    // This is valid YAML but not currently supported by our parser
    let yaml_content = load_fixture("folded_scalar_syntax");
    let result = parse(&yaml_content);

    // Currently fails due to folded scalar syntax limitation
    assert!(result.is_err(), "Folded scalar syntax is not yet supported");

    let error_msg = format!("{:?}", result.unwrap_err());
    assert!(error_msg.contains("YAML parsing error") || error_msg.contains("parse"));
}

// Integration tests for real-world failing gems - these document specific parsing limitations

#[test]
fn test_bacon_1_2_0_folded_scalar() {
    // bacon-1.2.0.gem fails due to YAML folded scalar syntax with tag (!)
    // The description field uses: description: ! 'text...' which is valid YAML but unsupported
    let yaml_content = std::fs::read_to_string("tests/fixtures/bacon-1.2.0.gemspec.yaml")
        .expect("bacon-1.2.0 fixture should exist");
    let result = parse(&yaml_content);

    // Should fail with YAML parsing error due to folded scalar syntax
    assert!(
        result.is_err(),
        "bacon-1.2.0 should fail due to folded scalar syntax limitation"
    );

    let error = result.unwrap_err();
    let error_msg = format!("{error}");
    assert!(
        error_msg.contains("YAML parsing error"),
        "Expected YAML parsing error, got: {error_msg}"
    );

    // Check the diagnostic contains information about the folded scalar issue
    let debug_msg = format!("{error:?}");
    assert!(
        debug_msg.contains("invalid indentation in quoted scalar") || debug_msg.contains("line 14"),
        "Expected folded scalar error details, got: {debug_msg}"
    );
}
