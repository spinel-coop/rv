# Design Decisions for rv-gem-package

## Overview
The `rv-gem-package` crate will provide functionality to read and extract `.gem` files, which are the distribution format for Ruby packages. This crate is essential for installing gems and analyzing their contents.

## .gem File Format Structure

Based on analysis of Ruby's `Gem::Package` implementation, a `.gem` file is:
- A tar archive containing:
  - `metadata.gz` - Gzipped YAML containing the gem specification
  - `data.tar.gz` - Gzipped tar archive containing the actual gem files
  - `checksums.yaml.gz` - Gzipped YAML containing checksums for verification
  - Optional signature files (`.sig` extensions)

The format supports both new-style gems (described above) and old-style gems (containing "MD5SUM =" markers).

## Key Design Decisions

### 1. Read-Only Focus
**Decision**: Initially implement only reading capabilities, not building gems.

**Rationale**:
- Primary use case is installing and analyzing gems
- Building gems is a separate concern that can be added later
- Reduces initial complexity and scope

### 2. Tar Archive Handling
**Decision**: Use existing Rust tar crate rather than implementing custom tar reading.

**Alternatives Considered**:
- Custom tar implementation (like Ruby does)
- Using the `tar` crate

**Chosen**: Use the `tar` crate
- Well-tested and maintained
- Handles edge cases and various tar formats
- Reduces implementation complexity

### 3. Compression Handling
**Decision**: Use `flate2` crate for gzip compression/decompression.

**Rationale**:
- Standard in Rust ecosystem
- Compatible with Ruby's zlib usage
- Well-tested implementation

### 4. API Design
**Decision**: Provide both streaming and extracted APIs.

**Key APIs**:
- `Package::open(path)` - Open a .gem file for reading
- `Package::spec()` - Get the gem specification
- `Package::data()` - List files in the gem (data.tar.gz)
- `Package::verify()` - Verify checksums and signatures

**Rationale**:
- Streaming API for efficiency with large gems
- Mirrors Ruby's API for familiarity

### 5. Security and Verification
**Decision**: Implement checksum verification but defer signature verification.

**Rationale**:
- Checksum verification is essential for integrity
- Signature verification requires additional crypto dependencies
- Can be added in a later iteration

### 6. Error Handling
**Decision**: Use custom error types that integrate with miette.

**Error Types**:
- `FormatError` - Invalid gem format
- `ChecksumError` - Checksum mismatch
- `IoError` - File system errors
- `TarError` - Tar archive errors


### 8. Metadata Handling
**Decision**: Reuse `rv-gem-specification-yaml` for parsing metadata.

**Rationale**:
- Already implemented and tested
- Ensures consistency across crates
- Handles Ruby YAML compatibility

### 10. Testing Strategy
**Decision**: Use real .gem files for integration tests.

**Approach**:
- Create minimal test gems in Ruby
- Include various edge cases (empty gems, large files, symlinks)
- Snapshot test extracted contents
- Unit test individual components

## Open Questions for User

1. **Checksum Algorithms**: Ruby supports multiple checksum algorithms (SHA256, SHA512). Should we:
   - Support all algorithms Ruby does? YES
   - Start with SHA256 only?
   - Make it configurable?

2. **Memory Usage**: For large gems, should we:
   - Always stream file contents during extraction?
   - Provide options for in-memory vs streaming extraction?
   - Set size limits for in-memory operations?
   
   Neither, we will not yet implement extraction. We should be able to avoid buffering everything in-memory when the `.gem` contents is from a seekable IO

3. **Symbolic Links**: How should we handle symlinks in gems?
   - Extract them as symlinks (security risk)?
   - Convert to regular files?
   - Make it configurable?
   
   Neither, we will not yet implement extraction.

4. **Old Format Support**: Should we support old-style .gem files (pre-2007)?
   - They use a different format with MD5 checksums
   - Rarely seen in practice
   - Ruby still supports them

   Recognize them, but return an error explaining they are not yet supported

5. **Performance Optimization**: Should we prioritize:
   - Fast extraction (parallel decompression)?
   - Low memory usage?
   - Streaming capabilities?

   Streaming

## Dependencies
- `tar` - Tar archive reading
- `flate2` - Gzip compression
- `rv-gem-types` - Gem specification types
- `rv-gem-specification-yaml` - YAML parsing
- `sha2` - Checksum verification
- `thiserror` - Error definitions
- `miette` - Error reporting

## Future Enhancements
- Gem building capabilities
- Signature verification
- (Parallel) extraction
- Caching extracted gems
- Progress reporting during extraction
