use rv_gem_specification_yaml::{parse, serialize_specification_to_yaml};
use std::{error::Error, fs};

fn load_fixture(name: &str) -> String {
    let fixture_path = format!("tests/fixtures/{name}.yaml");
    fs::read_to_string(&fixture_path)
        .unwrap_or_else(|_| panic!("Failed to read fixture: {fixture_path}"))
}

#[test]
fn test_round_trip_simple_specification() {
    let original_yaml = load_fixture("simple_spec");

    // Parse the original YAML
    let spec = parse(&original_yaml).expect("Failed to parse original YAML");

    // Serialize it back to YAML
    let round_trip_yaml =
        serialize_specification_to_yaml(&spec).expect("Failed to serialize specification");

    // Parse the round-tripped YAML
    let round_trip_spec = parse(&round_trip_yaml).expect("Failed to parse round-tripped YAML");

    // Test semantic equivalence
    assert_eq!(spec.name, round_trip_spec.name);
    assert_eq!(spec.version, round_trip_spec.version);
    assert_eq!(spec.summary, round_trip_spec.summary);
    assert_eq!(spec.authors, round_trip_spec.authors);
    assert_eq!(spec.email, round_trip_spec.email);
    assert_eq!(spec.homepage, round_trip_spec.homepage);
    assert_eq!(spec.description, round_trip_spec.description);
    assert_eq!(spec.licenses, round_trip_spec.licenses);
    assert_eq!(spec.files, round_trip_spec.files);
    assert_eq!(spec.executables, round_trip_spec.executables);
    assert_eq!(spec.dependencies.len(), round_trip_spec.dependencies.len());
    assert_eq!(spec.metadata, round_trip_spec.metadata);
    assert_eq!(spec.platform, round_trip_spec.platform);

    // Create snapshots for visual comparison
    insta::assert_snapshot!("round_trip_simple_original", original_yaml);
    insta::assert_snapshot!("round_trip_simple_generated", round_trip_yaml);
}

#[test]
fn test_round_trip_complex_specification() -> miette::Result<(), Box<dyn Error>> {
    let original_yaml = load_fixture("complex_spec");

    // Parse the original YAML
    let spec = parse(&original_yaml)?;

    // Serialize it back to YAML
    let round_trip_yaml = serialize_specification_to_yaml(&spec)?;

    // Parse the round-tripped YAML
    let round_trip_spec = parse(&round_trip_yaml)?;

    // Test semantic equivalence for complex spec
    assert_eq!(spec.name, round_trip_spec.name);
    assert_eq!(spec.version, round_trip_spec.version);
    assert_eq!(spec.dependencies.len(), round_trip_spec.dependencies.len());

    // Test dependencies in detail
    for (orig_dep, rt_dep) in spec
        .dependencies
        .iter()
        .zip(round_trip_spec.dependencies.iter())
    {
        assert_eq!(orig_dep.name, rt_dep.name);
        assert_eq!(orig_dep.dep_type, rt_dep.dep_type);
        assert_eq!(
            orig_dep.requirement.constraints.len(),
            rt_dep.requirement.constraints.len()
        );
    }

    // Test metadata preservation
    assert_eq!(spec.metadata, round_trip_spec.metadata);

    // Create snapshots for visual comparison
    insta::assert_snapshot!("round_trip_complex_original", original_yaml);
    insta::assert_snapshot!("round_trip_complex_generated", round_trip_yaml);

    Ok(())
}

#[test]
fn test_round_trip_version_constraints_specification() {
    let original_yaml = load_fixture("version_constraints_spec");

    // Parse the original YAML
    let spec = parse(&original_yaml).expect("Failed to parse original YAML");

    // Serialize it back to YAML
    let round_trip_yaml =
        serialize_specification_to_yaml(&spec).expect("Failed to serialize specification");

    // Parse the round-tripped YAML
    let round_trip_spec = parse(&round_trip_yaml).expect("Failed to parse round-tripped YAML");

    // Test semantic equivalence
    assert_eq!(spec.name, round_trip_spec.name);
    assert_eq!(spec.version, round_trip_spec.version);

    // Test complex version constraints
    for (orig_dep, rt_dep) in spec
        .dependencies
        .iter()
        .zip(round_trip_spec.dependencies.iter())
    {
        assert_eq!(orig_dep.name, rt_dep.name);
        assert_eq!(
            orig_dep.requirement.constraints.len(),
            rt_dep.requirement.constraints.len()
        );

        for (orig_constraint, rt_constraint) in orig_dep
            .requirement
            .constraints
            .iter()
            .zip(rt_dep.requirement.constraints.iter())
        {
            assert_eq!(orig_constraint.operator, rt_constraint.operator);
            assert_eq!(orig_constraint.version, rt_constraint.version);
        }
    }

    // Create snapshots for visual comparison
    insta::assert_snapshot!("round_trip_version_constraints_original", original_yaml);
    insta::assert_snapshot!("round_trip_version_constraints_generated", round_trip_yaml);
}

#[test]
fn test_round_trip_minimal_specification() {
    let original_yaml = load_fixture("minimal_spec");

    // Parse the original YAML
    let spec = parse(&original_yaml).expect("Failed to parse original YAML");

    // Serialize it back to YAML
    let round_trip_yaml =
        serialize_specification_to_yaml(&spec).expect("Failed to serialize specification");

    // Parse the round-tripped YAML
    let round_trip_spec = parse(&round_trip_yaml).expect("Failed to parse round-tripped YAML");

    // Test semantic equivalence
    assert_eq!(spec.name, round_trip_spec.name);
    assert_eq!(spec.version, round_trip_spec.version);
    assert_eq!(spec.summary, round_trip_spec.summary);
    assert_eq!(spec.authors, round_trip_spec.authors);

    // Create snapshots for visual comparison
    insta::assert_snapshot!("round_trip_minimal_original", original_yaml);
    insta::assert_snapshot!("round_trip_minimal_generated", round_trip_yaml);
}

#[test]
fn test_round_trip_prerelease_specification() {
    let original_yaml = load_fixture("prerelease_spec");

    // Parse the original YAML
    let spec = parse(&original_yaml).expect("Failed to parse original YAML");

    // Serialize it back to YAML
    let round_trip_yaml =
        serialize_specification_to_yaml(&spec).expect("Failed to serialize specification");

    // Parse the round-tripped YAML
    let round_trip_spec = parse(&round_trip_yaml).expect("Failed to parse round-tripped YAML");

    // Test semantic equivalence
    assert_eq!(spec.name, round_trip_spec.name);
    assert_eq!(spec.version, round_trip_spec.version);
    assert!(spec.version.is_prerelease());
    assert!(round_trip_spec.version.is_prerelease());

    // Test dependencies with development vs runtime types
    assert_eq!(spec.dependencies.len(), round_trip_spec.dependencies.len());

    // Create snapshots for visual comparison
    insta::assert_snapshot!("round_trip_prerelease_original", original_yaml);
    insta::assert_snapshot!("round_trip_prerelease_generated", round_trip_yaml);
}

#[test]
fn test_round_trip_licensed_specification() -> miette::Result<()> {
    let original_yaml = load_fixture("licensed_spec");

    // Parse the original YAML
    let spec = parse(&original_yaml)?;

    // Serialize it back to YAML
    let round_trip_yaml =
        serialize_specification_to_yaml(&spec).expect("Failed to serialize specification");

    // Parse the round-tripped YAML
    let round_trip_spec = parse(&round_trip_yaml).expect("Failed to parse round-tripped YAML");

    // Test semantic equivalence
    assert_eq!(spec.name, round_trip_spec.name);
    assert_eq!(spec.version, round_trip_spec.version);
    assert_eq!(spec.licenses, round_trip_spec.licenses);
    assert_eq!(spec.metadata, round_trip_spec.metadata);
    assert_eq!(spec.email, round_trip_spec.email);
    assert_eq!(spec.require_paths, round_trip_spec.require_paths);
    assert_eq!(spec.requirements, round_trip_spec.requirements);

    // Create snapshots for visual comparison
    insta::assert_snapshot!("round_trip_licensed_original", original_yaml);
    insta::assert_snapshot!("round_trip_licensed_generated", round_trip_yaml);

    Ok(())
}

#[test]
fn test_round_trip_edge_case_specification() {
    let original_yaml = load_fixture("edge_case_spec");

    // Parse the original YAML
    let spec = parse(&original_yaml).expect("Failed to parse original YAML");

    // Serialize it back to YAML
    let round_trip_yaml =
        serialize_specification_to_yaml(&spec).expect("Failed to serialize specification");

    // Parse the round-tripped YAML
    let round_trip_spec = parse(&round_trip_yaml).expect("Failed to parse round-tripped YAML");

    // Test semantic equivalence
    assert_eq!(spec.name, round_trip_spec.name);
    assert_eq!(spec.version, round_trip_spec.version);
    assert!(spec.version.is_prerelease());

    // Test complex multi-constraint dependencies
    assert_eq!(spec.dependencies.len(), round_trip_spec.dependencies.len());

    // Find the activesupport dependency with multiple constraints
    let activesupport_dep = spec
        .dependencies
        .iter()
        .find(|dep| dep.name == "activesupport")
        .expect("Should have activesupport dependency");
    let rt_activesupport_dep = round_trip_spec
        .dependencies
        .iter()
        .find(|dep| dep.name == "activesupport")
        .expect("Should have activesupport dependency in round-trip");

    assert_eq!(activesupport_dep.requirement.constraints.len(), 2);
    assert_eq!(rt_activesupport_dep.requirement.constraints.len(), 2);

    // Create snapshots for visual comparison
    insta::assert_snapshot!("round_trip_edge_case_original", original_yaml);
    insta::assert_snapshot!("round_trip_edge_case_generated", round_trip_yaml);
}
