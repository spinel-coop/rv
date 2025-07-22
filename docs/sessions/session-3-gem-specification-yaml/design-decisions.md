# Session 3: rv-gem-specification-yaml Design Decisions

## Overview
Create a new crate `rv-gem-specification-yaml` that provides YAML serialization/deserialization for `Gem::Specification` objects with full Ruby compatibility.

## Design Decisions

### 1. Winnow Parser Combinators with saphyr-parser Events
**Decision**: Use `winnow` parser combinator library with `saphyr-parser` events for YAML parsing
**Rationale**: Ruby's YAML output uses tags and custom serialization patterns that require strict validation. Winnow combinators with event-based parsing provides:
- Battle-tested combinator patterns (`many0`, `separated_list0`, `opt`, `alt`)
- Rich error handling with context built-in
- Zero-cost abstractions designed for performance  
- Precise source location tracking for error reporting
- Familiar API that Rust developers already know
- Natural composition of complex parsers from simple combinators
**Alternatives**:
- serde_yaml: Too rigid for Ruby's tag-based serialization
- saphyr high-level API: Less strict, harder to provide precise error locations
- yaml-rust2: Similar limitations to serde
- Custom event-based parser: More complex than winnow combinators

**Implementation**: Single `YamlParser<T, 'a>` trait with type-driven specialization using winnow combinators

### 2. Test-Driven Development with Ruby Fixtures
**Decision**: Generate test cases by creating actual Ruby `Gem::Specification` objects and dumping their YAML
**Rationale**: Ensures compatibility with real-world Ruby output rather than guessing at the format
**Implementation**: Use insta snapshots for regression testing

### 3. Separate Crate Architecture
**Decision**: Create a dedicated crate for YAML functionality
**Rationale**:
- Keeps YAML dependencies isolated
- Allows optional YAML support in main library
- Follows Rust ecosystem patterns (e.g., serde_json vs serde)

### 4. Strict Parser with miette Error Reporting
**Decision**: Implement strict parsing that fails on unexpected types or tags, with detailed error reporting using miette
**Rationale**:
- Ruby's YAML has a well-defined structure that should be validated strictly
- Better error messages help users identify malformed YAML quickly
- Source span tracking allows pinpointing exact error locations
- Prevents silent data corruption from mismatched types
**Trade-offs**: Less permissive than lenient parsing, but much safer and user-friendly

### 5. Complete YAML Serialization Implementation
**Decision**: Implement both parsing and serialization for full round-trip capability
**Benefits**:
- Enables round-trip testing to validate Ruby compatibility
- Supports bidirectional interoperability with Ruby tools
- Provides confidence in format fidelity
- Allows rv to generate Ruby-compatible YAML output
**Implementation**: Custom serialization using saphyr emitter with proper Ruby tag generation

## Ruby YAML Characteristics to Handle
- Object tags: `!ruby/object:Gem::Specification`, `!ruby/object:Gem::Version`, `!ruby/object:Gem::Requirement`, `!ruby/object:Gem::Dependency`
- Array serialization patterns
- Nil value handling
- Symbol vs string key differences
- Strict type validation (strings vs numbers vs booleans)
- Nested tag validation (versions within requirements, etc.)

## Final Implementation Summary

### Core Architecture Achieved
The final implementation successfully combines winnow parser combinators with saphyr-parser events, providing:

- **Custom YamlEventStream**: Implements winnow's `Stream` trait for YAML event parsing
- **Type-driven parsing**: Specialized parser functions for each Ruby object type
- **Comprehensive error reporting**: miette integration with precise source span tracking
- **Round-trip capability**: Both parsing and serialization with Ruby compatibility
- **Robust testing**: Generated Ruby fixtures with insta snapshot validation

### Key Implementation Details
- **Parser Architecture**: Direct combinator functions rather than trait hierarchies
- **Event Processing**: Lazy event streaming with precise error positioning
- **Ruby Compatibility**: Handles all Ruby YAML tags and nested object structures
- **Error Types**: Specific error variants with detailed diagnostic information
- **Serialization**: Custom saphyr emitter integration for Ruby-compatible output

### Technical Achievements
- ✅ Complete winnow integration with YAML event streams
- ✅ Full Ruby tag support (Specification, Version, Requirement, Dependency)
- ✅ Round-trip testing validates format fidelity
- ✅ Comprehensive error reporting with source locations
- ✅ Zero-copy event processing for performance
- ✅ Stable test output with IndexMap for deterministic ordering
