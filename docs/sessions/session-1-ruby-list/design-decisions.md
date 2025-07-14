# Design Decisions: `rv ruby list` Command

## Overview
Implementation of the `rv ruby list` command to display available Ruby versions and installations managed by `rv`.

## Design Decisions

### 1. Discovery Mechanism
**Decision**: Scan predefined Ruby installation directories for valid Ruby installations.

**Rationale**: 
- Follows established patterns from chruby/chrb and other Ruby version managers
- Provides predictable behavior for users familiar with existing tools
- Allows for multiple Ruby installation sources (system, homebrew, custom builds)

**Implementation**:
- Default directories: `~/.rubies`, `/opt/rubies`, `/usr/local/rubies`
- Configurable via `Config.ruby_dirs` field (already exists in main.rs:17)
- Directory scanning with validation of Ruby installations

### 2. Output Format
**Decision**: Support both human-readable text and machine-readable JSON output.

**Rationale**:
- Text format for interactive use and quick visual scanning
- JSON format for scripting and integration with other tools
- Follows Unix philosophy of being both human and machine friendly

**Implementation**:
- Default: Human-readable format with active version marker
- Flag: `--json` for structured output
- Active version marked with `*` in text mode

### 3. Ruby Installation Validation
**Decision**: Validate directories contain actual Ruby installations before listing.

**Rationale**:
- Prevents listing incomplete or broken installations
- Ensures user only sees functional Ruby versions
- Improves reliability and user experience

**Validation Criteria**:
- Directory contains `bin/ruby` executable
- Parse version information from directory name or ruby binary
- Support multiple Ruby engines (ruby, jruby, truffleruby)

### 4. Sorting and Display
**Decision**: Sort by engine type first, then by version within each engine.

**Rationale**:
- Groups similar Ruby implementations together
- Natural version ordering within each group
- Consistent with other version managers

**Display Format**:
```
* ruby-3.1.4
  ruby-3.2.0
  jruby-9.4.0.0
  truffleruby-22.3.0
```

### 5. Active Version Detection
**Decision**: Determine active Ruby from environment and configuration.

**Rationale**:
- Users need to know which Ruby is currently in use
- Follows established patterns from other version managers
- Supports both global and project-specific Ruby versions

**Detection Order**:
1. Project-specific `.ruby-version` file
2. Global configuration
3. Environment variables (PATH analysis)

### 6. Error Handling
**Decision**: Graceful degradation with informative error messages.

**Rationale**:
- Ruby installation directories may not exist
- Permissions issues should be handled gracefully
- Users should understand why no Rubies are found

**Error Cases**:
- No Ruby directories exist: Clear message explaining setup
- Permission denied: Inform user about access issues
- No valid Rubies found: Suggest installation methods

## Alternatives Considered

### Alternative 1: Database-based tracking
**Rejected**: Would require maintaining installation state, adding complexity without clear benefits.

### Alternative 2: PATH-only discovery
**Rejected**: Would miss manually installed or versioned Rubies not in PATH.

### Alternative 3: Single output format
**Rejected**: Limits scriptability and integration potential.

## uv Tool Inspiration

### Advanced CLI Patterns from uv
**Key insights from `uv python list`**:

**Rich Filtering Options**:
- `--only-installed`: Show only installed versions (exclude downloads)
- `--only-downloads`: Show only available downloads
- `--all-versions`: Include all patch versions, not just latest
- `--all-platforms`: Show versions for different architectures

**Detailed Output Information**:
- Full installation paths displayed alongside versions
- Symlink targets shown where applicable
- Clear distinction between installed vs downloadable versions
- Rich metadata in JSON format (version_parts, arch, implementation)

**Professional JSON Schema**:
```json
{
  "key": "ruby-3.1.4-macos-aarch64",
  "version": "3.1.4", 
  "version_parts": {"major": 3, "minor": 1, "patch": 4},
  "path": "/opt/rubies/ruby-3.1.4/bin/ruby",
  "symlink": null,
  "implementation": "ruby",
  "arch": "aarch64",
  "os": "macos"
}
```

**Enhanced rv Design Decisions**:
1. **Filtering Support**: Add `--installed-only` flag to match uv patterns
2. **Path Display**: Show full installation paths in text output  
3. **Rich JSON**: Include structured version parsing and system metadata
4. **Download Integration**: Future compatibility with `rv ruby install` download listings

## Dependencies
- `clap` for CLI argument parsing (already included)
- `serde` and `serde_json` for rich JSON output (inspired by uv)
- Standard library for filesystem operations
- Consider `semver` crate for version parsing and comparison

## Integration Points
- Uses existing `Config` struct from config.rs
- Leverages existing CLI structure in main.rs
- Will inform `rv ruby install` and `rv ruby pin` commands
- JSON schema designed for future download/install integration