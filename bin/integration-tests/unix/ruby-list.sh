#!/usr/bin/env bash
# bin/integration-tests/unix/ruby-list.sh
# Tests rv ruby list and rv ruby list --format json.
# Assumes Ruby is already installed via ruby-run.sh.
#
# Usage: ./bin/integration-tests/unix/ruby-list.sh <rv-binary-path>

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/_config.sh"

RV="${1:?Usage: ruby-list.sh <rv-binary-path>}"

echo "=== rv ruby list --installed-only ==="
list=$("$RV" ruby list --installed-only)
echo "$list"
echo "$list" | grep -q "$RUBY_VERSION"
echo "PASS: rv ruby list shows installed $RUBY_VERSION"

echo ""
echo "=== rv ruby list --installed-only --format json ==="
json=$("$RV" ruby list --installed-only --format json)
echo "$json"
echo "$json" | grep -q '"version"'
echo "PASS: rv ruby list --format json outputs valid JSON"
