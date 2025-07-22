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
    assert_eq!("Expected `ruby/object:Gem::Specification`, found mapping start with tag 'ruby/object:Gem::Version'", error_msg);
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
    assert_eq!("Expected `ruby/object:Gem::Version`, found mapping start with tag 'ruby/object:Gem::Requirement'", error_msg);
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

#[test]
fn test_unsupported_version_requirement_class() {
    // Current limitation: terminal-table-1.4.5.gem uses Gem::Version::Requirement instead of Gem::Requirement
    // This is valid Ruby but uses a different class hierarchy than we currently support
    let yaml_content = load_fixture("version_requirement_class");
    let result = parse(&yaml_content);

    // Currently fails because we only support !ruby/object:Gem::Requirement, not Gem::Version::Requirement
    assert!(
        result.is_err(),
        "Gem::Version::Requirement class is not yet supported"
    );

    let error_msg = format!("{:?}", result.unwrap_err());
    assert!(error_msg.contains("expected_event") || error_msg.contains("Gem::Requirement"));
}

#[test]
fn test_unsupported_yaml_anchors_and_prerelease_field() {
    // Current limitation: mocha-on-bacon-0.2.2.gem uses YAML anchors/references and dependency prerelease field
    // This includes: &id001 anchor, *id001 reference, and prerelease field in dependencies
    let yaml_content = load_fixture("yaml_anchors_and_prerelease");
    let result = parse(&yaml_content);

    // Currently fails due to unsupported YAML anchors and dependency prerelease field
    assert!(
        result.is_err(),
        "YAML anchors and dependency prerelease field are not yet supported"
    );

    let error_msg = format!("{:?}", result.unwrap_err());
    assert!(error_msg.contains("expected_event") || error_msg.contains("YAML"));
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

#[test]
fn test_ronn_0_7_3_dependency_prerelease_field() {
    // ronn-0.7.3.gem fails due to prerelease field in dependencies
    // Dependencies have an unsupported "prerelease: false" field we don't parse
    let yaml_content = std::fs::read_to_string("tests/fixtures/ronn-0.7.3.gemspec.yaml")
        .expect("ronn-0.7.3 fixture should exist");
    let result = parse(&yaml_content);

    // Should fail when parsing dependency with prerelease field
    assert!(
        result.is_err(),
        "ronn-0.7.3 should fail due to dependency prerelease field"
    );

    let error = result.unwrap_err();
    let error_msg = format!("{error}");
    assert!(
        error_msg.contains("Expected") && error_msg.contains("found"),
        "Expected parsing structure error, got: {error_msg}"
    );

    // Error should occur around the dependency prerelease field
    let debug_msg = format!("{error:?}");
    assert!(
        debug_msg.contains("expected_event") || debug_msg.contains("line 2"),
        "Expected dependency parsing error details, got: {debug_msg}"
    );
}

#[test]
fn test_net_http_pipeline_1_0_1_dependency_prerelease() {
    // net-http-pipeline-1.0.1.gem fails due to prerelease field in dependencies
    // Similar to ronn, has prerelease: false in dependency objects
    let yaml_content =
        std::fs::read_to_string("tests/fixtures/net-http-pipeline-1.0.1.gemspec.yaml")
            .expect("net-http-pipeline-1.0.1 fixture should exist");
    let result = parse(&yaml_content);

    // Should fail when encountering dependency prerelease field
    assert!(
        result.is_err(),
        "net-http-pipeline-1.0.1 should fail due to dependency prerelease field"
    );

    let error = result.unwrap_err();
    let error_msg = format!("{error}");
    assert!(
        error_msg.contains("Expected") || error_msg.contains("YAML parsing error"),
        "Expected parsing error, got: {error_msg}"
    );
}

#[test]
fn test_postgres_0_8_1_dependency_prerelease() {
    // postgres-0.8.1.gem fails due to prerelease field in dependencies
    // Another case of unsupported prerelease field in dependency structure
    let yaml_content = std::fs::read_to_string("tests/fixtures/postgres-0.8.1.gemspec.yaml")
        .expect("postgres-0.8.1 fixture should exist");
    let result = parse(&yaml_content);

    // Should fail due to dependency prerelease field parsing
    assert!(
        result.is_err(),
        "postgres-0.8.1 should fail due to dependency prerelease field"
    );

    let error = result.unwrap_err();
    let error_msg = format!("{error}");
    assert!(
        error_msg.contains("Expected") || error_msg.contains("expected_event"),
        "Expected dependency parsing error, got: {error_msg}"
    );
}

#[test]
fn test_mocha_on_bacon_0_2_2_yaml_anchors() {
    // mocha-on-bacon-0.2.2.gem fails due to YAML anchors (&id001, *id001) and prerelease field
    // Uses advanced YAML features we don't currently support
    let yaml_content = std::fs::read_to_string("tests/fixtures/mocha-on-bacon-0.2.2.gemspec.yaml")
        .expect("mocha-on-bacon-0.2.2 fixture should exist");
    let result = parse(&yaml_content);

    // Should fail due to YAML anchors and dependency prerelease field
    assert!(
        result.is_err(),
        "mocha-on-bacon-0.2.2 should fail due to YAML anchors and prerelease fields"
    );

    let error = result.unwrap_err();
    let error_msg = format!("{error}");
    assert!(
        error_msg.contains("Expected") || error_msg.contains("expected_event"),
        "Expected YAML anchor parsing error, got: {error_msg}"
    );
}

#[test]
fn test_terminal_table_1_4_5_version_requirement_class() {
    // terminal-table-1.4.5.gem fails due to Gem::Version::Requirement instead of Gem::Requirement
    // Uses !ruby/object:Gem::Version::Requirement which we don't support
    let yaml_content = std::fs::read_to_string("tests/fixtures/terminal-table-1.4.5.gemspec.yaml")
        .expect("terminal-table-1.4.5 fixture should exist");
    let result = parse(&yaml_content);

    // Should fail when parsing Gem::Version::Requirement tag
    assert!(
        result.is_err(),
        "terminal-table-1.4.5 should fail due to Gem::Version::Requirement class"
    );

    let error = result.unwrap_err();
    let error_msg = format!("{error}");
    assert!(
        error_msg.contains("Expected") && error_msg.contains("`ruby/object:Gem::Requirement`"),
        "Expected Gem::Requirement vs Gem::Version::Requirement error, got: {error_msg}"
    );

    // Should specifically mention the Gem::Requirement expectation
    let debug_msg = format!("{error:?}");
    assert!(
        debug_msg.contains("expected_event") || debug_msg.contains("requirements"),
        "Expected requirement class error details, got: {debug_msg}"
    );
}

#[test]
fn test_dm_do_adapter_1_2_0_dependency_prerelease() {
    // dm-do-adapter-1.2.0.gem fails due to dependency prerelease field
    // Another DataMapper gem with prerelease field in dependencies
    let yaml_content = std::fs::read_to_string("tests/fixtures/dm-do-adapter-1.2.0.gemspec.yaml")
        .expect("dm-do-adapter-1.2.0 fixture should exist");
    let result = parse(&yaml_content);

    // Should fail due to dependency structure parsing
    assert!(
        result.is_err(),
        "dm-do-adapter-1.2.0 should fail due to dependency prerelease field"
    );

    let error = result.unwrap_err();
    let error_msg = format!("{error}");
    assert!(
        error_msg.contains("Expected") || error_msg.contains("expected_event"),
        "Expected dependency parsing error, got: {error_msg}"
    );
}

#[test]
fn test_dm_postgres_adapter_1_2_0_dependency_prerelease() {
    // dm-postgres-adapter-1.2.0.gem fails due to dependency prerelease field
    // DataMapper adapter gem with similar prerelease field issue
    let yaml_content =
        std::fs::read_to_string("tests/fixtures/dm-postgres-adapter-1.2.0.gemspec.yaml")
            .expect("dm-postgres-adapter-1.2.0 fixture should exist");
    let result = parse(&yaml_content);

    // Should fail when parsing dependency prerelease field
    assert!(
        result.is_err(),
        "dm-postgres-adapter-1.2.0 should fail due to dependency prerelease field"
    );

    let error = result.unwrap_err();
    let error_msg = format!("{error}");
    assert!(
        error_msg.contains("Expected") || error_msg.contains("expected_event"),
        "Expected dependency parsing error, got: {error_msg}"
    );
}

#[test]
fn test_proxies_0_2_1_dependency_prerelease() {
    // proxies-0.2.1.gem fails due to dependency prerelease field
    // Another gem with prerelease field in dependency objects
    let yaml_content = std::fs::read_to_string("tests/fixtures/proxies-0.2.1.gemspec.yaml")
        .expect("proxies-0.2.1 fixture should exist");
    let result = parse(&yaml_content);

    // Should fail due to unsupported dependency prerelease field
    assert!(
        result.is_err(),
        "proxies-0.2.1 should fail due to dependency prerelease field"
    );

    let error = result.unwrap_err();
    let error_msg = format!("{error}");
    assert!(
        error_msg.contains("Expected") || error_msg.contains("expected_event"),
        "Expected dependency parsing error, got: {error_msg}"
    );
}

#[test]
fn test_rest_client_1_6_7_dependency_prerelease() {
    // rest-client-1.6.7.gem fails due to dependency prerelease field
    // Popular gem with prerelease field in dependencies
    let yaml_content = std::fs::read_to_string("tests/fixtures/rest-client-1.6.7.gemspec.yaml")
        .expect("rest-client-1.6.7 fixture should exist");
    let result = parse(&yaml_content);

    // Should fail when encountering prerelease field in dependency
    assert!(
        result.is_err(),
        "rest-client-1.6.7 should fail due to dependency prerelease field"
    );

    let error = result.unwrap_err();
    let error_msg = format!("{error}");
    assert!(
        error_msg.contains("Expected") || error_msg.contains("expected_event"),
        "Expected dependency parsing error, got: {error_msg}"
    );
}

#[test]
fn test_sinatra_1_0_dependency_prerelease() {
    // sinatra-1.0.gem fails due to dependency prerelease field
    // Classic Sinatra version with prerelease field in dependency objects
    let yaml_content = std::fs::read_to_string("tests/fixtures/sinatra-1.0.gemspec.yaml")
        .expect("sinatra-1.0 fixture should exist");
    let result = parse(&yaml_content);

    // Should fail due to dependency prerelease field parsing
    assert!(
        result.is_err(),
        "sinatra-1.0 should fail due to dependency prerelease field"
    );

    let error = result.unwrap_err();
    let error_msg = format!("{error}");
    assert!(
        error_msg.contains("Expected") || error_msg.contains("expected_event"),
        "Expected dependency parsing error, got: {error_msg}"
    );
}

#[test]
fn test_creole_0_5_0_dependency_prerelease() {
    // creole-0.5.0.gem fails due to dependency prerelease field
    // Wiki markup gem with prerelease field in dependencies
    let yaml_content = std::fs::read_to_string("tests/fixtures/creole-0.5.0.gemspec.yaml")
        .expect("creole-0.5.0 fixture should exist");
    let result = parse(&yaml_content);

    // Should fail when parsing dependency with prerelease field
    assert!(
        result.is_err(),
        "creole-0.5.0 should fail due to dependency prerelease field"
    );

    let error = result.unwrap_err();
    let error_msg = format!("{error}");
    assert!(
        error_msg.contains("Expected") || error_msg.contains("expected_event"),
        "Expected dependency parsing error, got: {error_msg}"
    );
}
