# Implementation Plan: Lockfile Parsing Crate

## Current Status

**✅ MAJOR PROGRESS**: Core lockfile parsing functionality is **80% complete** with all fundamental features working.

### What's Working:
- **Complete parser infrastructure** with state machine and section detection
- **All major section types**: GIT, GEM, PATH, PLUGIN, DEPENDENCIES, PLATFORMS, RUBY VERSION, BUNDLED WITH
- **Advanced error handling** with miette integration for precise diagnostics
- **Comprehensive testing** with 42 tests, 8 real lockfile fixtures, and snapshot testing
- **Platform handling** with full normalization and validation
- **Source polymorphism** with trait-based design
- **Strict/lenient parsing modes**

### Immediate Next Steps:
1. **CHECKSUMS section implementation** - The parser structure exists but needs the actual checksum parsing logic
2. **Complete gem dependency parsing** - Currently partially implemented, needs full dependency chain tracking
3. **File loading utilities** - Add convenient file I/O functions with proper error handling

### Integration Ready:
The crate is **ready for initial integration** into the rv CLI for basic lockfile operations.

---

## Implementation Checklist

### **Phase 1: Core Structure** ✅ COMPLETED

- [x] **1.1** Create new crate `rv-lockfile` with proper Cargo.toml
- [x] **1.2** Define core error types with miette integration (upgraded from thiserror)
- [x] **1.3** Implement basic Platform enum matching Gem::Platform
- [x] **1.4** Create Source enum with Git/Gem/Path/Plugin variants
- [x] **1.5** Define LazySpecification struct with all required fields
- [x] **1.6** Implement Dependency struct with version requirements
- [x] **1.7** Create main LockfileParser struct with all data fields

### **Phase 2: Parsing Infrastructure** ✅ COMPLETED

- [x] **2.1** Implement line-by-line parser with position tracking
- [x] **2.2** Create parsing state machine with section detection
- [x] **2.3** Add indentation validation logic (2/4/6 spaces)
- [x] **2.4** Implement merge conflict detection
- [x] **2.5** Create method dispatch system for section handlers
- [x] **2.6** Add comprehensive parse error reporting with miette context

### **Phase 3: Section Parsers** ✅ COMPLETED

- [x] **3.1** Implement GIT source section parser
- [x] **3.2** Implement GEM source section parser  
- [x] **3.3** Implement PATH source section parser
- [x] **3.4** Implement PLUGIN source section parser
- [x] **3.5** Create DEPENDENCIES section parser
- [x] **3.6** Implement PLATFORMS section parser
- [x] **3.7** Add RUBY VERSION section parser
- [x] **3.8** Create BUNDLED WITH section parser

### **Phase 4: Advanced Features** 🔄 PARTIALLY COMPLETED

- [ ] **4.1** Implement CHECKSUMS section parser (bundler 2.5.0+) - *Basic structure ready, needs implementation*
- [x] **4.2** Add strict vs lenient parsing modes
- [x] **4.3** Create version requirement parsing with semver
- [x] **4.4** Implement platform normalization and validation
- [ ] **4.5** Add gem specification dependency parsing - *Partially implemented, needs completion*
- [x] **4.6** Create source-specific option handling

### **Phase 5: API Design** 🔄 PARTIALLY COMPLETED

- [x] **5.1** Implement low-level parsing API (`parse_lockfile`, `parse_lockfile_strict`)
- [x] **5.2** Create high-level convenience methods (accessor methods)
- [ ] **5.3** Add file loading utilities with proper error handling - *TODO*
- [ ] **5.4** Implement platform filtering and querying - *TODO*
- [ ] **5.5** Create dependency resolution helpers - *TODO*
- [ ] **5.6** Add round-trip serialization support - *TODO*

### **Phase 6: Testing & Validation** ✅ COMPLETED

- [x] **6.1** Unit tests for all parsing functions (42 tests total)
- [x] **6.2** Integration tests with real bundler lockfiles (8 fixtures)
- [x] **6.3** Snapshot testing for consistent parsing output
- [x] **6.4** Error condition testing with malformed inputs
- [ ] **6.5** Performance benchmarks with large lockfiles - *TODO*
- [ ] **6.6** Compatibility testing with bundler versions - *TODO*

### **Phase 7: Integration** 🔄 STARTED

- [x] **7.1** Add lockfile crate to rv workspace
- [ ] **7.2** Update rv CLI to use lockfile parser - *TODO*
- [ ] **7.3** Implement gemfile handling in config - *TODO*
- [ ] **7.4** Add lockfile commands to rv CLI - *TODO*
- [ ] **7.5** Create documentation and examples - *TODO*
- [ ] **7.6** Performance optimization and profiling - *TODO*

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