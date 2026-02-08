#!/usr/bin/env bash
# bin/integration-tests/unix/cache-dir.sh
# Tests rv cache dir — verifies it prints a non-empty directory path.
#
# Usage: ./bin/integration-tests/unix/cache-dir.sh <rv-binary-path>

set -euo pipefail

RV="${1:?Usage: cache-dir.sh <rv-binary-path>}"

echo "=== rv cache dir ==="
cache=$("$RV" cache dir)
echo "  $cache"
[ -n "$cache" ]
echo "PASS: rv cache dir prints a non-empty path"
