use rv_gem_specification_yaml::{parse, serialize_specification_to_yaml};

#[test]
fn strings_that_resemble_yaml_scalars_remain_strings() {
    let original = parse(include_str!("fixtures/string_scalar_spec.yaml")).unwrap();
    assert_eq!(original.name, "null");
    assert_eq!(original.summary, "true");
    assert_eq!(
        original.authors,
        [None, Some("null".into()), Some("true".into())]
    );
    assert_eq!(
        original.metadata.get("null").map(String::as_str),
        Some("true")
    );

    let serialized = serialize_specification_to_yaml(&original).unwrap();
    let reparsed = parse(&serialized).expect("serialized String fields must parse as strings");
    assert_eq!(reparsed, original);
}
