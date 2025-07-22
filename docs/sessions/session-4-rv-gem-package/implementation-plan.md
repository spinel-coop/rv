# Implementation Plan for rv-gem-package

## Implementation Checklist

### Phase 1: Project Setup ✅ COMPLETED
- [x] **1.1** Create new crate `rv-gem-package` in `crates/` directory
- [x] **1.2** Set up Cargo.toml with dependencies
- [x] **1.3** Create basic module structure (lib.rs, error.rs, package.rs)
- [x] **1.4** Set up error types with thiserror and miette

### Phase 2: Core Types and Structures ✅ COMPLETED  
- [x] **2.1** Define `Package` struct to represent an open .gem file
- [x] **2.2** Create `Entry` type for files within the gem
- [x] **2.3** Define `Checksums` type for checksum data
- [x] **2.4** Create `PackageSource` trait for different input sources (file, seekable IO)

### Phase 3: Basic Reading Functionality ✅ COMPLETED
- [x] **3.1** Implement `Package::open()` to open a .gem file
- [x] **3.2** Add tar reading to iterate over top-level entries
- [x] **3.3** Detect and error on old-style .gem format (MD5SUM)
- [x] **3.4** Implement metadata.gz extraction and parsing
- [x] **3.5** Add method to read checksums.yaml.gz

### Phase 4: Data Access (No Extraction) ✅ COMPLETED
- [x] **4.1** Implement `Package::data()` to iterate files in data.tar.gz
- [x] **4.2** Add streaming `Entry` iteration support
- [x] **4.3** Create method to read specific file contents by path
- [x] **4.4** Add file metadata (size, mode, type) to entries
- [x] **4.5** Ensure streaming works with seekable IO sources

### Phase 5: Verification ✅ COMPLETED
- [x] **5.1** Implement checksum calculation during reading
- [x] **5.2** Support all Ruby checksum algorithms (SHA256, SHA512, etc.)
- [x] **5.3** Add `Package::verify()` for checksum verification
- [x] **5.4** Create detailed error types for verification failures

### Phase 6: Testing
- [ ] **6.1** Create test fixtures: minimal .gem files with Ruby
- [ ] **6.2** Add test for old-format .gem detection
- [ ] **6.3** Add integration tests for reading real gems
- [ ] **6.4** Test streaming from both files and in-memory sources
- [ ] **6.5** Test error cases (corrupt files, bad checksums)

### Phase 7: Documentation and Polish
- [ ] **7.1** Write comprehensive API documentation
- [ ] **7.2** Add usage examples in docs
- [ ] **7.3** Run clippy and fix all warnings
- [ ] **7.4** Format code with rustfmt

## Detailed Implementation Notes

### Package Structure
```rust
pub struct Package<S: PackageSource> {
    source: S,
    spec: Option<Specification>,
    checksums: Option<HashMap<String, HashMap<String, String>>>,
}
```

### Key Methods
```rust
impl<S: PackageSource> Package<S> {
    // Open a .gem file
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Package<FileSource>>;
    
    // Get the specification
    pub fn spec(&mut self) -> Result<&Specification>;
    
    // Iterate over files in data.tar.gz
    pub fn data(&mut self) -> Result<DataReader>;
    
    // Verify checksums
    pub fn verify(&mut self) -> Result<()>;
}
```

### Streaming Design
- Use seekable IO when available to avoid buffering
- Stream data.tar.gz entries on demand
- Support multiple checksum algorithms (SHA256, SHA512, etc.)
- Lazy-load metadata only when requested

### Error Handling
- Detect old .gem format early and return clear error
- Provide specific errors for each failure mode
- Include file paths in error messages

### Performance Considerations
- Minimize memory usage through streaming
- Seek within tar archives when possible
- Cache parsed metadata after first access

## Next Steps After Implementation
1. Integration with rv CLI for gem analysis
2. Add extraction capabilities in future phase
3. Add progress reporting for large gems
4. Consider adding gem building functionality