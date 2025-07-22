# rv-gem-package Examples

## verify_gems

This example demonstrates how to use `rv-gem-package` to open and verify multiple gem files.

### Usage

```bash
# Verify gems in default locations (~/.gem/ruby/*/cache/)
cargo run --example verify_gems

# Verify gems in specific directories
cargo run --example verify_gems -- /path/to/gems /another/path

# Verify specific gem files
cargo run --example verify_gems -- path/to/specific.gem
```

### Features

- **Automatic Discovery**: Finds gems in `~/.gem/ruby/*/cache/` by default
- **Batch Processing**: Processes multiple directories and files efficiently
- **Comprehensive Verification**: 
  - Opens each gem file
  - Reads and validates the specification
  - Verifies checksums against the gem's internal checksums
- **Clear Output**: Shows verification results with gem name and version
- **Error Reporting**: Detailed error messages for failed verifications
- **Summary Statistics**: Total processed, successful, and failed counts

### Output Example

```
📁 Scanning: /Users/user/.gem/ruby/3.0.0/cache
   Found 145 .gem files
✅ actioncable v7.0.4
✅ actionmailbox v7.0.4
❌ corrupt-gem v1.0.0: Checksum verification failed: checksum mismatch for file 'data.tar.gz' using SHA256: expected abc123, got def456
✅ actionpack v7.0.4

📊 Summary:
   Total gems found: 145
   Successfully verified: 144
   Failed verification: 1
```

### Error Handling

The example handles various error conditions:

- **Old Format Gems**: Detects and reports pre-2007 gem format
- **Corruption**: Identifies corrupted or incomplete gem files
- **Checksum Mismatches**: Reports specific checksum verification failures
- **Missing Files**: Handles cases where expected files are missing
- **I/O Errors**: Gracefully handles file system access issues

The tool exits with code 1 if any gems fail verification, making it suitable for use in CI/CD pipelines.