$ErrorActionPreference = "Stop"

cargo build --release --workspace --exclude rv-fuzz
