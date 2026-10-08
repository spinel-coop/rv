use rv_gem_specification_yaml::{parse, serialize_specification_to_yaml};

#[test]
fn ordinary_metadata_survives_serialization() {
    let original = parse(include_str!("fixtures/serialization_metadata.yaml")).unwrap();
    assert_eq!(original.date, "2026-10-07");
    assert_eq!(original.autorequire.as_deref(), Some("metadata_example"));
    assert_eq!(original.extra_rdoc_files, ["README.md", "CHANGELOG.md"]);
    assert_eq!(original.rdoc_options, ["--main", "README.md"]);
    assert_eq!(original.test_files, ["test/example_test.rb"]);

    let serialized = serialize_specification_to_yaml(&original).unwrap();
    let reparsed = parse(&serialized).unwrap();
    assert_eq!(reparsed, original);
}
