use indexmap::IndexMap;
use miette::Result;
use rv_gem_types::specification::Specification;
use rv_gem_types::{Dependency, Requirement, Version};
use saphyr::{Yaml, YamlEmitter};
use saphyr_parser::Tag;
use std::borrow::Cow;

use crate::SerializationError;

pub fn serialize_specification_to_yaml(spec: &Specification) -> Result<String> {
    let yaml_doc = specification_to_yaml_node(spec)?;

    let mut output = String::new();
    let mut emitter = YamlEmitter::new(&mut output);
    emitter
        .dump(&yaml_doc)
        .map_err(|emit_error| SerializationError::Emit { emit_error })?;

    Ok(output)
}

fn specification_to_yaml_node(spec: &Specification) -> Result<Yaml<'static>> {
    let mut mapping = saphyr::Mapping::new();

    // Add fields in the order Ruby typically serializes them
    insert_string_field(&mut mapping, "name", &spec.name);
    insert_version_field(&mut mapping, "version", &spec.version);
    insert_string_field(&mut mapping, "platform", &spec.platform.to_string());
    insert_string_array_field(&mut mapping, "authors", &spec.authors);
    insert_null_field(&mut mapping, "autorequire");
    insert_string_field(&mut mapping, "bindir", &spec.bindir);
    insert_empty_array_field(&mut mapping, "cert_chain");
    insert_null_field(&mut mapping, "date");
    insert_dependencies_field(&mut mapping, "dependencies", &spec.dependencies);

    if let Some(description) = &spec.description {
        insert_string_field(&mut mapping, "description", description);
    }

    insert_string_array_field(&mut mapping, "email", &spec.email);
    insert_string_array_field(&mut mapping, "executables", &spec.executables);
    insert_string_array_field(&mut mapping, "extensions", &spec.extensions);
    insert_empty_array_field(&mut mapping, "extra_rdoc_files");
    insert_string_array_field(&mut mapping, "files", &spec.files);

    if let Some(homepage) = &spec.homepage {
        insert_string_field(&mut mapping, "homepage", homepage);
    }

    insert_string_array_field(&mut mapping, "licenses", &spec.licenses);
    insert_metadata_field(&mut mapping, "metadata", &spec.metadata);
    if let Some(post_install_message) = spec.post_install_message.as_ref() {
        insert_string_field(&mut mapping, "post_install_message", post_install_message);
    } else {
        insert_null_field(&mut mapping, "post_install_message");
    }
    insert_empty_array_field(&mut mapping, "rdoc_options");
    insert_string_array_field(&mut mapping, "require_paths", &spec.require_paths);
    insert_requirement_field(
        &mut mapping,
        "required_ruby_version",
        &spec.required_ruby_version,
    );
    insert_requirement_field(
        &mut mapping,
        "required_rubygems_version",
        &spec.required_rubygems_version,
    );
    insert_string_array_field(&mut mapping, "requirements", &spec.requirements);
    insert_string_field(&mut mapping, "rubygems_version", &spec.rubygems_version);
    insert_null_field(&mut mapping, "signing_key");
    insert_integer_field(
        &mut mapping,
        "specification_version",
        spec.specification_version,
    );
    insert_string_field(&mut mapping, "summary", &spec.summary);
    insert_empty_array_field(&mut mapping, "test_files");

    // Create tagged YAML node for Gem::Specification
    let tag = Tag {
        handle: "!".into(),
        suffix: "ruby/object:Gem::Specification".into(),
    };

    Ok(Yaml::Tagged(
        Cow::Owned(tag),
        Box::new(Yaml::Mapping(mapping)),
    ))
}

fn insert_string_field(mapping: &mut saphyr::Mapping<'static>, key: &str, value: &str) {
    let key_yaml = Yaml::scalar_from_string(key.to_string());
    let value_yaml = Yaml::scalar_from_string(value.to_string());
    mapping.insert(key_yaml, value_yaml);
}

fn insert_integer_field(mapping: &mut saphyr::Mapping<'static>, key: &str, value: i32) {
    let key_yaml = Yaml::scalar_from_string(key.to_string());
    let value_yaml = Yaml::Value(saphyr::Scalar::Integer(value as i64));
    mapping.insert(key_yaml, value_yaml);
}

fn insert_null_field(mapping: &mut saphyr::Mapping<'static>, key: &str) {
    let key_yaml = Yaml::scalar_from_string(key.to_string());
    let value_yaml = Yaml::Value(saphyr::Scalar::Null);
    mapping.insert(key_yaml, value_yaml);
}

fn insert_string_array_field(mapping: &mut saphyr::Mapping<'static>, key: &str, values: &[String]) {
    let key_yaml = Yaml::scalar_from_string(key.to_string());
    let array_items: Vec<Yaml> = values
        .iter()
        .map(|s| Yaml::scalar_from_string(s.clone()))
        .collect();
    let value_yaml = Yaml::Sequence(array_items);
    mapping.insert(key_yaml, value_yaml);
}

fn insert_empty_array_field(mapping: &mut saphyr::Mapping<'static>, key: &str) {
    let key_yaml = Yaml::scalar_from_string(key.to_string());
    let value_yaml = Yaml::Sequence(vec![]);
    mapping.insert(key_yaml, value_yaml);
}

fn insert_version_field(mapping: &mut saphyr::Mapping<'static>, key: &str, version: &Version) {
    let key_yaml = Yaml::scalar_from_string(key.to_string());
    let version_yaml = version_to_yaml_node(version);
    mapping.insert(key_yaml, version_yaml);
}

fn version_to_yaml_node(version: &Version) -> Yaml<'static> {
    let mut version_mapping = saphyr::Mapping::new();
    let version_key = Yaml::scalar_from_string("version".to_string());
    // Force version to be a string to match Ruby's format
    let version_value = Yaml::Value(saphyr::Scalar::String(version.to_string().into()));
    version_mapping.insert(version_key, version_value);

    let tag = Tag {
        handle: "!".into(),
        suffix: "ruby/object:Gem::Version".into(),
    };

    Yaml::Tagged(Cow::Owned(tag), Box::new(Yaml::Mapping(version_mapping)))
}

fn insert_requirement_field(
    mapping: &mut saphyr::Mapping<'static>,
    key: &str,
    requirement: &Requirement,
) {
    let key_yaml = Yaml::scalar_from_string(key.to_string());
    let requirement_yaml = requirement_to_yaml_node(requirement);
    mapping.insert(key_yaml, requirement_yaml);
}

fn requirement_to_yaml_node(requirement: &Requirement) -> Yaml<'static> {
    let mut req_mapping = saphyr::Mapping::new();
    let requirements_key = Yaml::scalar_from_string("requirements".to_string());

    let mut requirements_array = Vec::new();
    for constraint in &requirement.constraints {
        let constraint_array = vec![
            Yaml::scalar_from_string(constraint.operator.to_string()),
            version_to_yaml_node(&constraint.version),
        ];
        requirements_array.push(Yaml::Sequence(constraint_array));
    }

    req_mapping.insert(requirements_key, Yaml::Sequence(requirements_array));

    let tag = Tag {
        handle: "!".into(),
        suffix: "ruby/object:Gem::Requirement".into(),
    };

    Yaml::Tagged(Cow::Owned(tag), Box::new(Yaml::Mapping(req_mapping)))
}

fn insert_dependencies_field(
    mapping: &mut saphyr::Mapping<'static>,
    key: &str,
    dependencies: &[Dependency],
) {
    let key_yaml = Yaml::scalar_from_string(key.to_string());
    let mut deps_array = Vec::new();

    for dep in dependencies {
        deps_array.push(dependency_to_yaml_node(dep));
    }

    mapping.insert(key_yaml, Yaml::Sequence(deps_array));
}

fn dependency_to_yaml_node(dependency: &Dependency) -> Yaml<'static> {
    let mut dep_mapping = saphyr::Mapping::new();

    let name_key = Yaml::scalar_from_string("name".to_string());
    let name_value = Yaml::scalar_from_string(dependency.name.clone());
    dep_mapping.insert(name_key, name_value);

    let requirement_key = Yaml::scalar_from_string("requirement".to_string());
    let requirement_value = requirement_to_yaml_node(&dependency.requirement);
    dep_mapping.insert(requirement_key, requirement_value);

    let type_key = Yaml::scalar_from_string("type".to_string());
    let type_value = Yaml::scalar_from_string(format!(":{}", dependency.dep_type.as_ref()));
    dep_mapping.insert(type_key, type_value);

    let prerelease_key = Yaml::scalar_from_string("prerelease".to_string());
    let prerelease_value = Yaml::Value(saphyr::Scalar::Boolean(
        dependency.requirement.is_prerelease(),
    ));
    dep_mapping.insert(prerelease_key, prerelease_value);

    let version_requirements_key = Yaml::scalar_from_string("version_requirements".to_string());
    let version_requirements_value = requirement_to_yaml_node(&dependency.requirement);
    dep_mapping.insert(version_requirements_key, version_requirements_value);

    let tag = Tag {
        handle: "!".into(),
        suffix: "ruby/object:Gem::Dependency".into(),
    };

    Yaml::Tagged(Cow::Owned(tag), Box::new(Yaml::Mapping(dep_mapping)))
}

fn insert_metadata_field(
    mapping: &mut saphyr::Mapping<'static>,
    key: &str,
    metadata: &IndexMap<String, String>,
) {
    let key_yaml = Yaml::scalar_from_string(key.to_string());
    let mut metadata_mapping = saphyr::Mapping::new();

    for (meta_key, meta_value) in metadata {
        let meta_key_yaml = Yaml::scalar_from_string(meta_key.clone());
        let meta_value_yaml = Yaml::scalar_from_string(meta_value.clone());
        metadata_mapping.insert(meta_key_yaml, meta_value_yaml);
    }

    mapping.insert(key_yaml, Yaml::Mapping(metadata_mapping));
}
