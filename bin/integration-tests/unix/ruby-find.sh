#!/usr/bin/env bash
# bin/integration-tests/unix/ruby-find.sh
# Tests rv ruby find.
# Assumes Ruby is already installed via ruby-run.sh.
#
# Usage: ./bin/integration-tests/unix/ruby-find.sh <rv-binary-path>

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/_config.sh"

RV="${1:?Usage: ruby-find.sh <rv-binary-path>}"

echo "=== rv ruby find $RUBY_VERSION ==="
find_output=$("$RV" ruby find "$RUBY_VERSION")
echo "  $find_output"
echo "$find_output" | grep -q "ruby"
echo "PASS: rv ruby find returns a path to ruby"
