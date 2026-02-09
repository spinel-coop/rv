$ErrorActionPreference = "Stop"

if ($args.Count -gt 0 -and $args[0] -eq "--fix") {
    cargo fmt --all
} else {
    cargo fmt --all -- --check
}

cargo clippy --all-targets --all-features @args -- -D warnings
