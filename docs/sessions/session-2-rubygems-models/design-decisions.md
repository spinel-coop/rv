# Session 2: RubyGems Models Crate Design Decisions

## Overview
Creating a new `rv-gem-types` crate to provide Rust implementations of key RubyGems model classes with 100% RubyGems compatibility.

## Design Decisions

### 1. Crate Structure
- **Decision**: Create a new crate `rv-gem-types` within the existing workspace
- **Rationale**: Separates concerns and allows for independent versioning of models
- **Alternative**: Implement models directly in the `rv` crate
- **Trade-offs**: Additional complexity but better modularity

### 2. Model Priority
Based on RubyGems research, prioritize implementation in this order:
1. Version/requirement models (most foundational)
2. Dependency models
3. Platform and basic specification models
4. Package
5. Source (look at bundler source subclasses for this one)

### 3. Rust Implementation Strategy
- **Decision**: Use idiomatic Rust patterns with serde for serialization
- **Rationale**: Leverage Rust's type system for safety and performance
- **Key patterns**: 
  - Enums for constrained values (Platform, DependencyType)
  - Structs for data models
  - Traits for shared behavior
  - Result types for error handling

### 4. Version Compatibility
- **Decision**: Focus on modern RubyGems specification format (v4)
- **Rationale**: Simplifies implementation while covering current use cases
- **Alternative**: Support all historical formats
- **Trade-offs**: May need to add legacy support later

### 5. Dependencies
- **Decision**: Minimize external dependencies, use regex and thiserror for implementation
- **Rationale**: Reduces dependency tree and improves compile times
- **Key dependencies**: regex (for platform parsing), thiserror (for error handling)

### 6. RubyGems Compatibility Strategy
- **Decision**: Implement exact RubyGems compatibility by analyzing canonical RubyGems source code
- **Rationale**: Ensures perfect interoperability with RubyGems ecosystem
- **Implementation**: Direct analysis of RubyGems source code and comprehensive test porting
- **Key areas**: Version parsing, platform matching (===), requirement handling, specification serialization

### 7. Platform Model Implementation
- **Decision**: Use enum with Ruby/Current/Specific variants rather than string-based approach
- **Rationale**: Type safety while maintaining compatibility
- **Key insight**: Platform matching uses the `===` operator logic from RubyGems, not simple equality
- **Special cases**: MinGW universal matching, ARM CPU compatibility, Linux version handling

### 8. Version Segment Design
- **Decision**: Use enum with Number/String variants for version segments
- **Rationale**: Handles mixed alphanumeric version components correctly
- **Compatibility**: Matches RubyGems prerelease handling and canonical segment logic

### 9. Test Strategy
- **Decision**: Port complete RubyGems test suites verbatim where possible
- **Rationale**: Ensures 100% behavioral compatibility
- **Coverage**: 78+ test cases from RubyGems Platform tests, comprehensive Version/Requirement testing