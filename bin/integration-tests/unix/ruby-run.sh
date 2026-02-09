#!/usr/bin/env bash
# bin/integration-tests/unix/ruby-run.sh
# Downloads and runs Ruby via rv ruby run.
# This is the "setup" step — must run before other integration tests.
#
# Usage: ./bin/integration-tests/unix/ruby-run.sh <rv-binary-path>

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/_config.sh"

RV="${1:?Usage: ruby-run.sh <rv-binary-path>}"

echo "=== rv ruby run $RUBY_VERSION (install + execute) ==="
output=$("$RV" ruby run "$RUBY_VERSION" -- -e 'puts RUBY_DESCRIPTION')
echo "  $output"
echo "$output" | grep -q "$RUBY_VERSION"
echo "PASS: rv ruby run installed and executed Ruby $RUBY_VERSION"
