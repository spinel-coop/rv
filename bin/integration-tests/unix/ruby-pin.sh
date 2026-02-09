#!/usr/bin/env bash
# bin/integration-tests/unix/ruby-pin.sh
# Tests rv ruby pin — writes and verifies .ruby-version file.
# Assumes Ruby is already installed via ruby-run.sh.
#
# Usage: ./bin/integration-tests/unix/ruby-pin.sh <rv-binary-path>

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/_config.sh"

RV="${1:?Usage: ruby-pin.sh <rv-binary-path>}"

echo "=== rv ruby pin $RUBY_VERSION ==="
"$RV" ruby pin "$RUBY_VERSION"
pin=$(cat .ruby-version)
echo "  .ruby-version contains: $pin"
echo "$pin" | grep -q "$RUBY_VERSION"
echo "PASS: rv ruby pin writes correct .ruby-version"

# Note: .ruby-version is intentionally left in place for subsequent tests
# (e.g. shell-env.sh needs it). Cleaned up by ruby-uninstall.sh.
