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

// Tests for gems that now parse successfully due to prerelease field support

#[test]
fn test_ronn_0_7_3_dependency_prerelease_field() {
    // ronn-0.7.3.gem now parses successfully with prerelease field support
    let yaml_content = std::fs::read_to_string("tests/fixtures/ronn-0.7.3.gemspec.yaml")
        .expect("ronn-0.7.3 fixture should exist");
    let result = parse(&yaml_content);

    match result {
        Ok(spec) => {
            assert_eq!(spec.name, "ronn");
            assert_eq!(spec.version.to_string(), "0.7.3");
            // Verify dependencies with prerelease fields are handled correctly
            assert!(!spec.dependencies.is_empty());
        }
        Err(e) => {
            panic!("ronn-0.7.3 should now parse successfully: {e}");
        }
    }
}

#[test]
fn test_net_http_pipeline_1_0_1_dependency_prerelease() {
    let yaml_content =
        std::fs::read_to_string("tests/fixtures/net-http-pipeline-1.0.1.gemspec.yaml")
            .expect("net-http-pipeline-1.0.1 fixture should exist");
    let result = parse(&yaml_content);

    match result {
        Ok(spec) => {
            assert_eq!(spec.name, "net-http-pipeline");
            assert_eq!(spec.version.to_string(), "1.0.1");
        }
        Err(e) => {
            panic!("net-http-pipeline-1.0.1 should now parse successfully: {e}");
        }
    }
}

#[test]
fn test_postgres_0_8_1_dependency_prerelease() {
    let yaml_content = std::fs::read_to_string("tests/fixtures/postgres-0.8.1.gemspec.yaml")
        .expect("postgres-0.8.1 fixture should exist");
    let result = parse(&yaml_content);

    match result {
        Ok(spec) => {
            assert_eq!(spec.name, "postgres");
            assert_eq!(spec.version.to_string(), "0.8.1");
        }
        Err(e) => {
            panic!("postgres-0.8.1 should now parse successfully: {e}");
        }
    }
}

#[test]
fn test_dm_do_adapter_1_2_0_dependency_prerelease() {
    let yaml_content = std::fs::read_to_string("tests/fixtures/dm-do-adapter-1.2.0.gemspec.yaml")
        .expect("dm-do-adapter-1.2.0 fixture should exist");
    let result = parse(&yaml_content);

    match result {
        Ok(spec) => {
            assert_eq!(spec.name, "dm-do-adapter");
            assert_eq!(spec.version.to_string(), "1.2.0");
        }
        Err(e) => {
            panic!("dm-do-adapter-1.2.0 should now parse successfully: {e}");
        }
    }
}

#[test]
fn test_dm_postgres_adapter_1_2_0_dependency_prerelease() {
    let yaml_content =
        std::fs::read_to_string("tests/fixtures/dm-postgres-adapter-1.2.0.gemspec.yaml")
            .expect("dm-postgres-adapter-1.2.0 fixture should exist");
    let result = parse(&yaml_content);

    match result {
        Ok(spec) => {
            assert_eq!(spec.name, "dm-postgres-adapter");
            assert_eq!(spec.version.to_string(), "1.2.0");
        }
        Err(e) => {
            panic!("dm-postgres-adapter-1.2.0 should now parse successfully: {e}");
        }
    }
}

#[test]
fn test_proxies_0_2_1_dependency_prerelease() {
    let yaml_content = std::fs::read_to_string("tests/fixtures/proxies-0.2.1.gemspec.yaml")
        .expect("proxies-0.2.1 fixture should exist");
    let result = parse(&yaml_content);

    match result {
        Ok(spec) => {
            assert_eq!(spec.name, "proxies");
            assert_eq!(spec.version.to_string(), "0.2.1");
        }
        Err(e) => {
            panic!("proxies-0.2.1 should now parse successfully: {e}");
        }
    }
}

#[test]
fn test_rest_client_1_6_7_dependency_prerelease() {
    let yaml_content = std::fs::read_to_string("tests/fixtures/rest-client-1.6.7.gemspec.yaml")
        .expect("rest-client-1.6.7 fixture should exist");
    let result = parse(&yaml_content);

    match result {
        Ok(spec) => {
            assert_eq!(spec.name, "rest-client");
            assert_eq!(spec.version.to_string(), "1.6.7");
        }
        Err(e) => {
            panic!("rest-client-1.6.7 should now parse successfully: {e}");
        }
    }
}

#[test]
fn test_sinatra_1_0_dependency_prerelease() {
    let yaml_content = std::fs::read_to_string("tests/fixtures/sinatra-1.0.gemspec.yaml")
        .expect("sinatra-1.0 fixture should exist");
    let result = parse(&yaml_content);

    match result {
        Ok(spec) => {
            assert_eq!(spec.name, "sinatra");
            assert_eq!(spec.version.to_string(), "1.0");
        }
        Err(e) => {
            panic!("sinatra-1.0 should now parse successfully: {e}");
        }
    }
}

#[test]
fn test_creole_0_5_0_dependency_prerelease() -> miette::Result<()> {
    // creole-0.5.0.gem with prerelease field in dependencies
    let yaml_content = std::fs::read_to_string("tests/fixtures/creole-0.5.0.gemspec.yaml")
        .expect("creole-0.5.0 fixture should exist");
    let result = parse(&yaml_content)?;

    insta::assert_debug_snapshot!(result.dependencies, @r#"
    [
        Dependency {
            name: "bacon",
            requirement: Requirement {
                constraints: [
                    VersionConstraint {
                        operator: GreaterEqual,
                        version: Version {
                            version: "0",
                            segments: [
                                Number(
                                    0,
                                ),
                            ],
                        },
                    },
                ],
            },
            dep_type: Development,
        },
        Dependency {
            name: "rake",
            requirement: Requirement {
                constraints: [
                    VersionConstraint {
                        operator: GreaterEqual,
                        version: Version {
                            version: "0",
                            segments: [
                                Number(
                                    0,
                                ),
                            ],
                        },
                    },
                ],
            },
            dep_type: Development,
        },
    ]
    "#);
    Ok(())
}

#[test]
fn test_mocha_on_bacon_0_2_2_yaml_anchors() {
    // mocha-on-bacon-0.2.2.gem now parses successfully despite YAML anchors and prerelease fields
    let yaml_content = std::fs::read_to_string("tests/fixtures/mocha-on-bacon-0.2.2.gemspec.yaml")
        .expect("mocha-on-bacon-0.2.2 fixture should exist");
    let result = parse(&yaml_content);

    match result {
        Ok(spec) => {
            assert_eq!(spec.name, "mocha-on-bacon");
            assert_eq!(spec.version.to_string(), "0.2.2");
        }
        Err(e) => {
            panic!("mocha-on-bacon-0.2.2 should now parse successfully: {e}");
        }
    }
}

#[test]
fn test_yaml_anchors_and_prerelease_field() {
    // This fixture now parses successfully with prerelease field support
    let yaml_content = load_fixture("yaml_anchors_and_prerelease");
    let result = parse(&yaml_content);

    match result {
        Ok(_spec) => {
            // Successfully parsed YAML with anchors and prerelease fields
        }
        Err(e) => {
            panic!("YAML anchors and prerelease field should now parse successfully: {e}");
        }
    }
}
