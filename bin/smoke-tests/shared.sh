#!/bin/bash
# Shared functions for smoke tests

# Re-exec inside Docker on macOS (call this first)
macos_docker_reexec() {
    if [[ "$(uname -s)" == "Darwin" ]]; then
        echo "Detected macOS - running via Docker..."
        REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
        exec docker run -it --rm -v "$REPO_ROOT:/src" rust:latest "/src/bin/smoke-tests/$(basename "$0")"
    fi
}

# Install system packages (pass project-specific packages as arguments)
setup_packages() {
    local packages=("$@")

    # Use sudo if available (needed in GHA), skip if not (Docker runs as root)
    SUDO=""
    if command -v sudo &> /dev/null; then
        SUDO="sudo"
    fi

    export DEBIAN_FRONTEND=noninteractive
    $SUDO apt-get update
    $SUDO apt-get install -y --no-install-recommends \
        build-essential \
        git \
        ca-certificates \
        curl \
        libclang-dev \
        "${packages[@]}"
}

# Build/install rv
# In CI, rv is pre-built and already in PATH. Locally, build from source.
setup_rv() {
    echo ""
    if [[ -z "${CI:-}" ]]; then
        echo "Building rv from source..."
        "$(dirname "$0")/../build"
        export PATH="$(dirname "$0")/../../target/release:$PATH"
    else
        echo "Using pre-built rv from CI..."
    fi
}

# Print success message
smoke_test_success() {
    local project="$1"
    echo ""
    echo "Success: $project smoke test passed"
}
