# Implementation Plan: Command Structure Scaffolding

## Implementation Checklist

### Phase 1: Analyze Command Structure ✅
- [x] **1.1a** Review README command list and group by functional domains
- [x] **1.1b** Identify shared functionality patterns across command groups
- [x] **1.1c** Design scalable module hierarchy

### Phase 2: Create Module Structure ✅
- [x] **2.1a** Create command group modules (ruby, tool, script, app, gem)
- [x] **2.1b** Set up shared utility modules (installation, config, etc.)
- [x] **2.1c** Create placeholder command implementations with proper signatures
- [x] **2.1d** Update main.rs to route to new command structure

### Phase 3: CLI Argument Structure ✅
- [x] **3.1a** Define top-level CLI structure with all command groups
- [x] **3.1b** Create argument structures for each command group
- [x] **3.1c** Add placeholder argument definitions for all planned commands
- [x] **3.1d** Ensure help text generation works for all commands

### Phase 4: Shared Infrastructure ✅
- [x] **4.1a** Create installation module for Ruby/tool downloads (placeholder)
- [x] **4.1b** Enhance configuration management for all command types
- [x] **4.1c** Create error types for each command domain (miette integration)
- [x] **4.1d** Set up logging and user feedback systems (placeholder)

### Phase 5: Integration and Testing ✅
- [x] **5.1a** Wire all commands through main.rs dispatch
- [x] **5.1b** Add placeholder implementations that show help or "not implemented"
- [x] **5.1c** Test CLI help generation and argument parsing
- [x] **5.1d** Verify project builds and all commands are accessible

### Phase 6: Documentation ✅
- [x] **6.1a** Document module organization and conventions
- [x] **6.1b** Create implementation guides for future command development
- [x] **6.1c** Update README if needed to reflect current state

## Command Groups Analysis

### Ruby Version Management
- `rv ruby list` ✅ (already implemented)
- `rv ruby install`
- `rv ruby uninstall` 
- `rv ruby pin`

### Tool Management (Gem CLIs)
- `rvx <tool>` / `rv tool run <tool>`
- `rv tool install <tool>`
- `rv tool uninstall <tool>`

### Script Management
- `rv run <script>`
- `rv add --script <gem>`
- `rv remove --script <gem>`

### Application Management
- `rv init`
- `rv install`
- `rv add <gem>`
- `rv remove <gem>`
- `rv upgrade`
- `rv tree`

### Gem Development
- `rv gem <name>`
- `rv build`
- `rv publish`

## Module Structure Design

```
src/
├── main.rs                 # CLI entry point and routing
├── config.rs              # Configuration management
├── error.rs               # Top-level error types
├── commands/
│   ├── mod.rs             # Command routing
│   ├── ruby/              # Ruby version management
│   │   ├── mod.rs         # Ruby command args and routing
│   │   ├── list.rs        # ✅ Already implemented
│   │   ├── install.rs     # Download and install Ruby versions
│   │   ├── uninstall.rs   # Remove Ruby installations
│   │   └── pin.rs         # Set project Ruby version
│   ├── tool/              # Tool/gem CLI management
│   │   ├── mod.rs         # Tool command args and routing
│   │   ├── run.rs         # Execute tool with auto-install
│   │   ├── install.rs     # Install tool permanently
│   │   └── uninstall.rs   # Remove installed tool
│   ├── script/            # Script execution
│   │   ├── mod.rs         # Script command args and routing
│   │   ├── run.rs         # Execute script with dependency resolution
│   │   ├── add.rs         # Add script dependency
│   │   └── remove.rs      # Remove script dependency
│   ├── app/               # Application management
│   │   ├── mod.rs         # App command args and routing
│   │   ├── init.rs        # Initialize new Ruby project
│   │   ├── install.rs     # Install project dependencies
│   │   ├── add.rs         # Add gem dependency
│   │   ├── remove.rs      # Remove gem dependency
│   │   ├── upgrade.rs     # Upgrade dependencies
│   │   └── tree.rs        # Show dependency tree
│   └── gem/               # Gem development
│       ├── mod.rs         # Gem command args and routing
│       ├── new.rs         # Create new gem
│       ├── build.rs       # Build gem package
│       └── publish.rs     # Publish to registry
├── ruby.rs                # Ruby installation types and utilities
├── installation/          # Shared installation logic
│   ├── mod.rs             # Installation types and traits
│   ├── download.rs        # Download and verification
│   ├── extract.rs         # Archive extraction
│   └── registry.rs        # Package registry interaction
└── utils/                 # Shared utilities
    ├── mod.rs             # Utility exports
    ├── process.rs         # Process execution helpers
    └── fs.rs              # Filesystem utilities
```

## Success Criteria

- [x] All planned commands are accessible via CLI help
- [x] Project builds successfully with placeholder implementations
- [x] Clear module organization that supports independent command development
- [x] Shared infrastructure ready for Ruby/tool installation logic
- [x] Documentation guides for implementing individual commands
- [x] Consistent error handling and user experience patterns