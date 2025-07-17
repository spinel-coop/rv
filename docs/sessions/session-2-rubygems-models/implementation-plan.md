# RubyGems Models Crate Implementation Plan

## Implementation Checklist

### Phase 1: Foundation
- [ ] **1.1a** Create rv-gem-types crate in workspace
- [ ] **1.1b** Set up basic Cargo.toml with serde & miette dependencies
- [ ] **1.1c** Create module structure (lib.rs, version.rs, platform.rs, etc.)
- [ ] **1.1d** Add rv-gem-types to workspace Cargo.toml

### Phase 2: Version Model
- [ ] **2.1a** Implement `Version` struct with parsing and comparison
- [ ] **2.1b** Test `Version` struct

### Phase 2.5: Requirement Model
- [ ] **2.5b** Implement `Requirement` struct with constraint parsing
- [ ] **2.5c** Add version requirement matching logic
- [ ] **2.5d** Write comprehensive tests for requirement functionality

### Phase 3: Platform Model
- [ ] **3.1a** Implement `Platform` enum with CPU/OS/version variants and a `ruby` variant
- [ ] **3.1b** Add platform matching logic for compatibility
- [ ] **3.1c** Implement platform string parsing and formatting
- [ ] **3.1d** Add tests for platform compatibility checking

### Phase 4: Basic Specification Models
- [ ] **4.1a** Implement `NameTuple` struct for lightweight gem identification
- [ ] **4.1b** Create `DependencyType` enum (runtime, development)
- [ ] **4.1c** Implement `Dependency` struct with name/version/type
- [ ] **4.1d** Add dependency resolution and matching logic

### Phase 5: Specification Model
- [ ] **5.1a** Design `Specification` struct with core fields
- [ ] **5.1b** Implement specification parsing from gemspec format
- [ ] **5.1c** Add specification validation logic
- [ ] **5.1d** Create serialization/deserialization for JSON/YAML formats

### Phase 6: Integration & Testing
- [ ] **6.1a** Create integration tests with real gemspec files
- [ ] **6.1b** Add benchmarks for performance-critical operations
- [ ] **6.1c** Update workspace dependencies to use rv-gem-type
- [ ] **6.1d** Run full test suite and fix any integration issues

## Suggested Classes for Implementation

### Core Models (Priority 1)
```rust
// version.rs
pub struct Version {
    version: String,
    segments: Vec<Either<String, u32>>
}

pub struct Requirement {
    constraints: Vec<VersionConstraint>,
}

pub struct VersionConstraint {
    operator: ComparisonOperator,
    version: Version,
}

pub enum ComparisonOperator {
    Equal,
    NotEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
    Pessimistic, // ~>
}
```

### Platform Models (Priority 2)
```rust
// platform.rs
pub enum Platform {
    Ruby,
    Current,
    Specific {
        cpu: Option<CPU>,
        os: String,
        version: Option<String>,
    },
}

pub enum CPU {
    X86,
    X64,
    Arm64,
    Other(String)
}

// name_tuple.rs
pub struct NameTuple {
    name: String,
    version: Version,
    platform: Platform,
}
```

### Dependency Models (Priority 3)
```rust
// dependency.rs
pub struct Dependency {
    name: String,
    requirement: Requirement,
    dependency_type: DependencyType,
    groups: Vec<String>,
}

pub enum DependencyType {
    Runtime,
    Development,
}
```

### Specification Models (Priority 4)
```rust
// specification.rs
pub struct Specification {
    name: String,
    version: Version,
    platform: Platform,
    dependencies: Vec<Dependency>,
    authors: Vec<String>,
    email: Option<String>,
    homepage: Option<String>,
    summary: Option<String>,
    description: Option<String>,
    licenses: Vec<String>,
    files: Vec<String>,
    executables: Vec<String>,
    extensions: Vec<String>,
    required_ruby_version: Option<Requirement>,
    required_rubygems_version: Option<Requirement>,
}
```

## Implementation Notes

1. **Error Handling**: Use `Result<T, E>` types throughout for proper error handling. Use miette to provide helpful diagnostics.
2. **Serialization**: Do not implement serde traits for JSON/YAML compatibility
3. **Performance**: Use `Cow<str>` for string fields that might be borrowed
4. **Validation**: Add validation methods for each model type
5. **Compatibility**: Ensure parsing matches RubyGems behavior exactly
6. **Testing**: Include property-based tests for version parsing/comparison