# rv-cache

A caching system for the `rv` Ruby version manager.

## Features

- **Cache buckets** for organizing different types of data
- **CLI integration** with `--no-cache` and `--cache-dir` options  
- **Safe cleanup** with detailed reporting
- **Stable cache keys** using SeaHash
- **Timestamp-based invalidation**

## Usage

```rust
use rv_cache::{Cache, CacheBucket};

// Create cache
let cache = Cache::from_path("/path/to/cache");

// Access entries
let entry = cache.entry(CacheBucket::Ruby, "interpreter", "ruby-3.3.0.json");
```

## CLI Integration

```rust
#[derive(clap::Parser)]
struct App {
    #[command(flatten)]
    cache: CacheArgs,
}

let cache: Cache = app.cache.try_into()?;
```

## Cache Structure

```
~/.cache/rv/
├── ruby-v0/     # Ruby interpreter builds and metadata
```

## Optional Features

- `clap`: CLI argument parsing support

```toml
[dependencies]
rv-cache = { version = "0.1.0", features = ["clap"] }
```
