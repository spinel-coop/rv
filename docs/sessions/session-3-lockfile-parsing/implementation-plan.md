# Implementation Plan: Lockfile Parsing Crate

## Implementation Checklist

### **Phase 1: Core Structure** 

- [ ] **1.1** Create new crate `rv-lockfile` with proper Cargo.toml
- [ ] **1.2** Define core error types with thiserror integration  
- [ ] **1.3** Implement basic Platform enum matching Gem::Platform
- [ ] **1.4** Create Source enum with Git/Gem/Path/Plugin variants
- [ ] **1.5** Define LazySpecification struct with all required fields
- [ ] **1.6** Implement Dependency struct with version requirements
- [ ] **1.7** Create main LockfileParser struct with all data fields

### **Phase 2: Parsing Infrastructure**

- [ ] **2.1** Implement line-by-line parser with position tracking
- [ ] **2.2** Create parsing state machine with section detection
- [ ] **2.3** Add indentation validation logic (2/4/6 spaces)
- [ ] **2.4** Implement merge conflict detection
- [ ] **2.5** Create method dispatch system for section handlers
- [ ] **2.6** Add comprehensive parse error reporting with context

### **Phase 3: Section Parsers**

- [ ] **3.1** Implement GIT source section parser
- [ ] **3.2** Implement GEM source section parser  
- [ ] **3.3** Implement PATH source section parser
- [ ] **3.4** Implement PLUGIN source section parser
- [ ] **3.5** Create DEPENDENCIES section parser
- [ ] **3.6** Implement PLATFORMS section parser
- [ ] **3.7** Add RUBY VERSION section parser
- [ ] **3.8** Create BUNDLED WITH section parser

### **Phase 4: Advanced Features**

- [ ] **4.1** Implement CHECKSUMS section parser (bundler 2.5.0+)
- [ ] **4.2** Add strict vs lenient parsing modes
- [ ] **4.3** Create version requirement parsing with semver
- [ ] **4.4** Implement platform normalization and validation
- [ ] **4.5** Add gem specification dependency parsing
- [ ] **4.6** Create source-specific option handling

### **Phase 5: API Design**

- [ ] **5.1** Implement low-level parsing API
- [ ] **5.2** Create high-level convenience methods
- [ ] **5.3** Add file loading utilities with proper error handling
- [ ] **5.4** Implement platform filtering and querying
- [ ] **5.5** Create dependency resolution helpers
- [ ] **5.6** Add round-trip serialization support

### **Phase 6: Testing & Validation**

- [ ] **6.1** Unit tests for all parsing functions
- [ ] **6.2** Integration tests with real bundler lockfiles
- [ ] **6.3** Property-based testing for round-trip consistency
- [ ] **6.4** Error condition testing with malformed inputs
- [ ] **6.5** Performance benchmarks with large lockfiles
- [ ] **6.6** Compatibility testing with bundler versions

### **Phase 7: Integration**

- [ ] **7.1** Add lockfile crate to rv workspace
- [ ] **7.2** Update rv CLI to use lockfile parser
- [ ] **7.3** Implement gemfile handling in config
- [ ] **7.4** Add lockfile commands to rv CLI
- [ ] **7.5** Create documentation and examples
- [ ] **7.6** Performance optimization and profiling

## Detailed Implementation Notes

### Crate Structure
```
rv-lockfile/
├── src/
│   ├── lib.rs              # Public API exports
│   ├── parser.rs           # Main LockfileParser implementation  
│   ├── types/
│   │   ├── mod.rs
│   │   ├── source.rs       # Source enum and implementations
│   │   ├── spec.rs         # LazySpecification 
│   │   ├── dependency.rs   # Dependency struct
│   │   └── platform.rs     # Platform enum
│   ├── sections/
│   │   ├── mod.rs
│   │   ├── git.rs          # GIT section parser
│   │   ├── gem.rs          # GEM section parser
│   │   ├── path.rs         # PATH section parser
│   │   ├── plugin.rs       # PLUGIN section parser
│   │   ├── dependencies.rs # DEPENDENCIES parser
│   │   ├── platforms.rs    # PLATFORMS parser
│   │   ├── ruby.rs         # RUBY VERSION parser
│   │   ├── bundled.rs      # BUNDLED WITH parser
│   │   └── checksums.rs    # CHECKSUMS parser
│   ├── error.rs            # Error types and handling
│   └── utils.rs            # Parsing utilities
├── tests/
│   ├── integration_test.rs
│   ├── fixtures/           # Real lockfile examples
│   └── property_tests.rs   # Property-based tests
└── benches/
    └── parsing_bench.rs    # Performance benchmarks
```

### Key Dependencies
```toml
[dependencies]
thiserror = "2.0"      # Error handling
semver = "1.0"         # Version parsing  
regex = "1.0"          # Pattern matching
serde = { version = "1.0", features = ["derive"], optional = true }
```

### Error Handling Strategy
```rust
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("Invalid indentation at line {line}: expected {expected} spaces, found {found}")]
    InvalidIndentation { line: usize, expected: usize, found: usize },
    
    #[error("Merge conflict detected at line {line}")]
    MergeConflict { line: usize },
    
    #[error("Invalid version format: {version}")]
    InvalidVersion { version: String },
    
    #[error("Unknown source type: {source_type}")]
    UnknownSourceType { source_type: String },
    
    #[error("Unexpected section: {section}")]
    UnexpectedSection { section: String },
}
```

### Testing Strategy
- **Unit Tests**: Each parsing function tested individually
- **Integration Tests**: Real bundler lockfiles from various projects
- **Property Tests**: Round-trip parsing, invalid input handling
- **Compatibility Tests**: Different bundler versions and formats
- **Performance Tests**: Large lockfiles, memory usage validation

### Performance Considerations
- String interning for repeated gem names
- Lazy loading for expensive operations
- Minimal allocations during parsing
- Efficient regex compilation and reuse
- Memory-mapped file reading for large lockfiles

## Success Criteria

1. **Compatibility**: Parse all valid bundler lockfiles correctly
2. **Performance**: Handle large lockfiles (1000+ gems) efficiently  
3. **Reliability**: Robust error handling and recovery
4. **Maintainability**: Clean, well-documented code
5. **Integration**: Seamless integration with rv CLI
6. **Testing**: Comprehensive test coverage with CI validation