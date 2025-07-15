# Design Decisions: Lockfile Parsing Crate

## Design Decisions

### 1. Parser Architecture
**Decision**: Use a state machine-based parser with method dispatch similar to bundler's implementation.

**Rationale**:
- Mirrors the proven bundler implementation's approach
- Handles complex indentation-based hierarchy naturally (2/4/6 spaces)
- Extensible for new section types via additional parsing methods
- Clear separation between parsing logic and data structures

**Implementation**: 
- Use enum-based state tracking for current section type
- Method dispatch based on line content and current state
- Position tracking for error reporting

### 2. Data Structure Design
**Decision**: Mirror bundler's core data structures with Rust-idiomatic implementations.

**Rationale**:
- Maintains compatibility with existing bundler lockfile semantics
- Leverages proven data organization from bundler
- Enables straightforward conversion between formats

**Core Types**:
```rust
pub struct LockfileParser {
    sources: Vec<Source>,
    specs: HashMap<String, LazySpecification>,
    dependencies: HashMap<String, Dependency>,
    platforms: Vec<Platform>,
    bundler_version: Option<Version>,
    ruby_version: Option<String>,
}

pub enum Source {
    Git(GitSource),
    Gem(GemSource), 
    Path(PathSource),
    Plugin(PluginSource),
}
```

### 3. Source Type Handling
**Decision**: Use trait-based polymorphism for different source types.

**Rationale**:
- Type-safe handling of source-specific parsing logic
- Extensible for new source types (plugins)
- Clear separation of concerns per source type

**Implementation**:
```rust
pub trait SourceParser {
    fn parse_line(&mut self, line: &str) -> Result<(), ParseError>;
    fn finalize(self) -> Source;
}
```

### 4. Error Handling Strategy
**Decision**: Use structured error types with position tracking and detailed context.

**Rationale**:
- Better debugging experience than bundler's basic error messages
- Enables precise error location reporting
- Type-safe error handling with Rust's Result system

**Error Types**:

Add in sources for miette:

```rust
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("Invalid indentation at line {line}: expected {expected} spaces, found {found}")]
    InvalidIndentation { line: usize, expected: usize, found: usize },
    
    #[error("Merge conflict detected at line {line}")]
    MergeConflict { line: usize },
    
    #[error("Invalid version format: {version}")]
    InvalidVersion { version: String },
}
```

### 5. Platform Representation
**Decision**: Create dedicated Platform enum matching Gem::Platform semantics.

**Rationale**:
- Type-safe platform handling vs string-based approach
- Enables platform-specific validation and normalization
- Consistent with bundler's platform handling logic

### 6. Version Handling
**Decision**: Use semver crate for version parsing and comparison.

**Rationale**:
- Robust version handling with proper semantic versioning
- Handles complex version requirements and constraints
- Well-tested library with Ruby version compatibility

### 7. Indentation Parsing
**Decision**: Strict indentation validation using exact space counting.

**Rationale**:
- Matches bundler's precise indentation requirements (2/4/6 spaces)
- Prevents malformed lockfiles from being accepted
- Clear hierarchy representation in the file format

**Logic**:
- 2 spaces: Top-level items (dependencies, checksums, platform entries)
- 4 spaces: Gem specifications within source sections
- 6 spaces: Dependencies of specific gems

### 8. Memory Efficiency
**Decision**: Use string interning for gem names and lazy loading for specifications.

**Rationale**:
- Reduces memory usage for repeated gem names across dependencies
- Mirrors bundler's LazySpecification concept for deferred loading
- Efficient for large lockfiles with many dependencies

### 9. API Design
**Decision**: Provide both low-level parsing API and high-level convenience methods.

**Rationale**:
- Low-level API for maximum flexibility and performance
- High-level API for common use cases and ease of use
- Enables both library usage and standalone CLI tools

**APIs**:
```rust
// Low-level parsing
pub fn parse_lockfile(content: &str) -> Result<LockfileParser, ParseError>;

// High-level convenience  
pub fn load_lockfile<P: AsRef<Path>>(path: P) -> Result<LockfileParser, Error>;
pub fn dependencies_for_platform(&self, platform: &Platform) -> Vec<&Dependency>;
```

### 10. Testing Strategy
**Decision**: Comprehensive test suite with real lockfile examples and property-based testing.

**Rationale**:
- Ensures compatibility with existing bundler lockfiles
- Validates parsing of edge cases and malformed input
- Property-based testing for round-trip parsing consistency

**Test Categories**:
- Unit tests for individual parsing functions
- Integration tests with real bundler lockfiles
- Property-based tests for round-trip parsing
- Error condition testing with invalid inputs

### 11. Checksum Support
**Decision**: Full support for bundler 2.5.0+ checksum format with optional validation.

**Rationale**:
- Forward compatibility with bundler's security features
- Optional validation allows flexibility in usage
- Structured checksum representation for programmatic access

### 12. Strict Mode
**Decision**: Support both strict and lenient parsing modes.

**Rationale**:
- Strict mode for production use and validation
- Lenient mode for development and recovery scenarios
- Matches bundler's validation approach for dependencies

## Alternatives Considered

### Custom String-Based Parsing
**Rejected**: Would require reimplementing complex lockfile semantics and lose bundler compatibility.

### JSON/TOML Alternative Format
**Rejected**: Would break compatibility with existing bundler ecosystem and tooling.

### Async Parsing
**Rejected**: Lockfiles are typically small and don't benefit from async I/O overhead.

### Zero-Copy Parsing
**Rejected**: Complex lifetime management doesn't justify performance gains for typical lockfile sizes.

## Implementation Phases

### Phase 1: Core Parser
- Basic lockfile structure parsing
- Source section handling (GIT, GEM, PATH)
- Dependencies and platforms parsing

### Phase 2: Advanced Features  
- Checksum support
- Plugin source types
- Strict validation mode

### Phase 3: Integration
- High-level convenience APIs
- CLI tool integration
- Performance optimization