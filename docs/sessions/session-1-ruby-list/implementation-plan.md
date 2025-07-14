# Implementation Plan: `rv ruby list` Command

## Implementation Checklist

### Phase 1: Core Data Structures ✅
- [x] **1.1a** Create `Ruby` struct to represent a Ruby installation (engine, version, path)
- [x] **1.1b** Add default Ruby directories to `Config` struct (extend existing `ruby_dirs`)
- [x] **1.1c** Add JSON output flag to CLI args (`--json` or `--format json`)

### Phase 2: Ruby Discovery Logic ✅
- [x] **2.1a** Implement `discover_rubies(config: &Config) -> Vec<Ruby>` function
- [x] **2.1b** Add directory scanning logic for each `ruby_dirs` path
- [x] **2.1c** Implement Ruby installation validation (check for `bin/ruby`)
- [x] **2.1d** Add version parsing from directory names (ruby-3.1.4, jruby-9.4.0.0)
- [x] **2.1e** Add engine detection (ruby, jruby, truffleruby, etc.)

### Phase 3: Active Ruby Detection
- [ ] **3.1a** Implement `find_active_ruby() -> Option<String>` function
- [ ] **3.1b** Check for `.ruby-version` file in current directory
- [ ] **3.1c** Check global Ruby configuration
- [ ] **3.1d** Add PATH analysis for fallback detection

### Phase 4: Output Formatting ✅
- [x] **4.1a** Implement text output format with active marker (*)
- [x] **4.1b** Implement JSON output format using serde_json
- [x] **4.1c** Add Ruby sorting logic (engine first, then version)
- [x] **4.1d** Handle empty Ruby list with helpful message

### Phase 5: Error Handling & Polish ✅
- [x] **5.1a** Add proper error handling for directory access issues
- [x] **5.1b** Add informative messages when no Rubies found
- [x] **5.1c** Handle permission denied errors gracefully
- [ ] **5.1d** Add integration tests for various scenarios

### Phase 6: Integration ✅
- [x] **6.1a** Update `list_rubies()` function in main.rs with full implementation
- [x] **6.1b** Add required dependencies to Cargo.toml (serde, serde_json)
- [x] **6.1c** Update CLI argument parsing to handle output format flag
- [x] **6.1d** Test with real Ruby installations

## File Changes Required

### main.rs
- Update `list_rubies()` function (currently empty at line 76-78)
- Add JSON output flag to CLI args
- Import new modules and functions

### Cargo.toml  
- Add `serde = { version = "1.0", features = ["derive"] }`
- Add `serde_json = "1.0"`

### New files to create
- `src/ruby.rs` - Ruby struct and discovery logic
- `src/output.rs` - Output formatting functions

## Testing Strategy

### Manual Testing
- Test with empty Ruby directories
- Test with mixed Ruby engines (ruby, jruby)
- Test with different version formats
- Test JSON vs text output
- Test active Ruby detection

### Integration Points
- Ensure Config struct changes don't break existing code
- Verify CLI parsing still works correctly
- Check error handling matches existing patterns

## Dependencies

### Required Crates
- `serde` and `serde_json` for JSON output
- Standard library for filesystem operations
- Existing `clap` for CLI parsing

### System Requirements
- Read access to Ruby installation directories
- File system traversal capabilities

## Success Criteria
- [ ] `rv ruby list` shows all installed Ruby versions
- [ ] Active Ruby is clearly marked with `*`
- [ ] `rv ruby list --json` outputs valid JSON
- [ ] Graceful handling of missing or inaccessible directories
- [ ] Sorted output (engine, then version)
- [ ] Helpful messages when no Rubies found