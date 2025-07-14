# Design Decisions: `rv ruby list` Command

## Design Decisions

### 1. Discovery Mechanism
**Decision**: Scan predefined Ruby installation directories for valid Ruby installations.

**Rationale**: 
- Follows established patterns from chruby/chrb and other Ruby version managers
- Provides predictable behavior for users familiar with existing tools

### 2. Output Format
**Decision**: Support both human-readable text and machine-readable JSON output.

**Rationale**:
- Text format for interactive use and quick visual scanning
- JSON format for scripting and integration with other tools

### 3. Ruby Installation Validation
**Decision**: Validate directories contain actual Ruby installations before listing.

**Rationale**:
- Prevents listing incomplete or broken installations
- Ensures user only sees functional Ruby versions

### 4. Sorting and Display
**Decision**: Sort by engine type first, then by version within each engine.

**Rationale**:
- Groups similar Ruby implementations together
- Natural version ordering within each group

### 5. Active Version Detection
**Decision**: Follow chrb's precedence order for Ruby version detection.

**Rationale**:
- Uses proven patterns from chrb codebase
- Comprehensive detection with PATH fallback
- Supports both global and project-specific Ruby versions

### 6. Architecture and Testing
**Decision**: Use VFS abstraction with dependency injection for testable code.

**Rationale**:
- Unit tests should not depend on physical filesystem
- Enables fast, reliable test execution
- Clear separation of concerns between CLI, logic, and I/O

### 7. Terminal Output
**Decision**: Follow uv's professional CLI output patterns with colors and alignment.

**Rationale**:
- Professional appearance consistent with modern CLI tools
- Clear visual hierarchy and readability

## Alternatives Considered

### PATH-only discovery

**Rejected**: Would miss manually installed or versioned Rubies not in PATH.

### Physical filesystem in unit tests

**Rejected**: Creates flaky tests with filesystem dependencies and cleanup issues.

### Standalone `is_active_ruby` function

**Rejected**: Less idiomatic Rust API.
**Chosen**: `Ruby::is_active()` method for better encapsulation.

### Custom Ruby version detection logic

**Rejected**: Would reinvent proven patterns and potentially miss edge cases.
**Chosen**: Follow chrb's battle-tested detection precedence.