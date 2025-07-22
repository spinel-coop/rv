# rv-gem-package

A Rust library for reading and analyzing Ruby gem (`.gem`) files.

## Features

- **Read-only gem access**: Open and inspect gem files without extraction
- **Streaming data access**: Efficiently access files within gems without loading everything into memory
- **Checksum verification**: Verify gem integrity using SHA1, SHA256, and SHA512 checksums
- **Comprehensive error handling**: Structured error types with detailed diagnostic information
- **Old format detection**: Identifies and rejects pre-2007 gem formats
- **Multiple data sources**: Read from files, memory, or any `Read + Seek` source

## Usage

### Basic Example

```rust
use rv_gem_package::Package;

// Open a gem file
let mut package = Package::open("path/to/gem.gem")?;

// Access the specification
let spec = package.spec()?;
println!("Gem: {} v{}", spec.name, spec.version);

// Verify checksums
package.verify()?;

// Access files within the gem
let mut data = package.data()?;
if let Some(file) = data.find_file("lib/my_gem.rb")? {
    let content = String::from_utf8_lossy(file.content());
    println!("File content: {}", content);
}
```

### Reading from Memory

```rust
use rv_gem_package::Package;
use std::io::Cursor;

let gem_data = std::fs::read("gem.gem")?;
let cursor = Cursor::new(gem_data);
let mut package = Package::from_source(cursor)?;

let spec = package.spec()?;
println!("Loaded {} from memory", spec.name);
```

### Checksum Verification

```rust
use rv_gem_package::{Package, ChecksumAlgorithm};

let mut package = Package::open("gem.gem")?;

// Verify all checksums
package.verify()?;

// Access checksum information
let checksums = package.checksums()?;
for algorithm in checksums.algorithms() {
    println!("Algorithm: {}", algorithm);
    if let Some(files) = checksums.files_for_algorithm(algorithm) {
        for file in files {
            if let Some(checksum) = checksums.get_checksum(algorithm, file) {
                println!("  {}: {}", file, checksum);
            }
        }
    }
}
```

## Examples

The crate includes a comprehensive example that demonstrates batch verification of gems:

```bash
# Verify gems in default Ruby cache locations
cargo run --example verify_gems

# Verify gems in specific directories
cargo run --example verify_gems -- /path/to/gems /another/path
```

See [`examples/README.md`](examples/README.md) for detailed usage information.

## Error Handling

The library provides structured error types with rich diagnostic information:

- **FormatError**: Issues with gem file format, encoding, or structure
- **ChecksumError**: Checksum verification failures with specific details
- **TarError**: Problems reading the tar archive structure
- **YamlParsing**: YAML parsing errors with full diagnostic context
- **OldFormatError**: Detection of pre-2007 gem format

```rust
use rv_gem_package::{Package, Error};

match Package::open("problematic.gem") {
    Ok(mut package) => {
        match package.verify() {
            Ok(()) => println!("Gem verified successfully"),
            Err(Error::ChecksumError(err)) => {
                // Handle checksum-specific error with structured fields
                eprintln!("Checksum verification failed: {}", err);
            }
            Err(e) => eprintln!("Other error: {}", e),
        }
    }
    Err(Error::OldFormatError) => {
        eprintln!("This gem uses an unsupported old format");
    }
    Err(e) => eprintln!("Failed to open gem: {}", e),
}
```

## Architecture

The library is structured around several key types:

- **`Package<S>`**: Main interface for accessing gem contents
- **`PackageSource`**: Trait for different data sources (files, memory, etc.)
- **`DataReader<R>`**: Streaming access to files within the gem
- **`Entry`**: Metadata about files within the gem
- **`Checksums`**: Checksum information and verification
- **`ChecksumAlgorithm`**: Supported checksum algorithms

## Testing

The crate includes comprehensive tests with real gem fixtures:

```bash
cargo test --all-features
```
