# rv-gem-specification-yaml

A Rust library for parsing and serializing Ruby gem specification YAML files.

This crate provides functionality to parse the `metadata.gz` files found in Ruby gem packages (`.gem` files) and convert them to structured Rust types, as well as serialize specifications back to YAML format.

## Features

- **High Compatibility**: Successfully parses 99.8% of real-world Ruby gems (7584/7602 gems tested)
- **Round-trip Serialization**: Parse YAML specifications and serialize them back to YAML
- **Structured Error Types**: Detailed error reporting with diagnostic information
- **Ruby Compatibility**: Handles various Ruby gem specification formats and legacy patterns

## Usage

### Basic Parsing

```rust
use rv_gem_specification_yaml::parse;

let yaml_content = r#"
--- !ruby/object:Gem::Specification
name: example-gem
version: !ruby/object:Gem::Version
  version: 1.0.0
summary: An example gem
authors:
- Test Author
dependencies: []
"#;

let specification = parse(yaml_content)?;
println!("Gem: {} v{}", specification.name, specification.version);
```

### Serialization

```rust
use rv_gem_specification_yaml::serialize_specification_to_yaml;
use rv_gem_types::{Specification, Version};

let spec = Specification::new("my-gem".to_string(), Version::new("1.0.0")?)?
    .with_summary("A test gem".to_string());

let yaml_output = serialize_specification_to_yaml(&spec)?;
println!("{}", yaml_output);
```

### Error Handling

```rust
use rv_gem_specification_yaml::{parse, DeserializationError};

match parse(yaml_content) {
    Ok(spec) => println!("Parsed: {}", spec.name),
    Err(e) => {
        eprintln!("Parse error: {}", e);
        // Rich diagnostic information available
    }
}
```

## Supported Ruby Gem Patterns

This library successfully handles a wide variety of Ruby gem specification patterns:

- **Standard specifications** with all common fields
- **Version objects** with additional fields (`prerelease`, `hash`, `segments`)
- **Requirement objects** with legacy `none` field
- **Dependencies** with both old (`version_requirements`) and new (`requirement`) field names
- **Null values** in authors and email arrays (preserved semantically)
- **Complex version constraints** and dependency specifications
- **Legacy gem formats** from different RubyGems eras

## Known Limitations

The following Ruby gem patterns are **not yet supported** (affecting 0.2% of tested gems):

### 1. YAML Folded Scalar Syntax
```yaml
description: ! 'Multi-line text with
  folded scalar syntax...'
```
**Example**: `bacon-1.2.0.gem`  
**Status**: Valid YAML syntax not supported by parser

### 2. Gem::Version::Requirement Class
```yaml
required_ruby_version: !ruby/object:Gem::Version::Requirement
  requirements:
  - - ">"
    - !ruby/object:Gem::Version
      version: 0.0.0
```
**Example**: `terminal-table-1.4.5.gem`  
**Status**: Different class hierarchy than standard `Gem::Requirement`

### 3. YAML Anchors and References
```yaml
dependencies:
- !ruby/object:Gem::Dependency
  requirement: &id001 !ruby/object:Gem::Requirement
    # ... requirement definition
  version_requirements: *id001
  prerelease: false
```
**Example**: `mocha-on-bacon-0.2.2.gem`  
**Status**: YAML anchors/references and dependency `prerelease` field not implemented

## Architecture

The library is structured around several key components:

- **`parser.rs`**: Core YAML parsing logic using `saphyr` and `winnow`
- **`serialize.rs`**: YAML serialization functionality
- **Error types**: Structured error handling with `miette` diagnostics
- **Integration with `rv-gem-types`**: Uses shared Ruby gem type definitions

## Testing

The crate includes comprehensive tests:

```bash
# Run all tests
cargo test

# Run specific test suites
cargo test --test yaml_compatibility  # Success cases
cargo test --test malformed_yaml      # Error cases and limitations
cargo test --test round_trip          # Serialization round-trips
```

## Contributing

When adding new features or fixing parsing issues:

1. Add test fixtures in `tests/fixtures/`
2. Update test cases in `tests/yaml_compatibility.rs` for success cases
3. Document limitations in `tests/malformed_yaml.rs` for unsupported patterns
4. Run the full test suite to ensure no regressions
