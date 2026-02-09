#!/usr/bin/env bash
# bin/integration-tests/unix/ruby-uninstall.sh
# Tests rv ruby uninstall — removes Ruby, verifies it's gone, re-installs.
# Assumes Ruby is already installed via ruby-run.sh.
#
# Usage: ./bin/integration-tests/unix/ruby-uninstall.sh <rv-binary-path>

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/_config.sh"

RV="${1:?Usage: ruby-uninstall.sh <rv-binary-path>}"

echo "=== rv ruby uninstall $RUBY_VERSION ==="
"$RV" ruby uninstall "$RUBY_VERSION"
echo "PASS: rv ruby uninstall exits without error"

echo ""
echo "=== Verify $RUBY_VERSION is no longer listed ==="
list_after=$("$RV" ruby list --installed-only 2>/dev/null || true)
if echo "$list_after" | grep -q "$RUBY_VERSION.*installed"; then
  echo "FAIL: $RUBY_VERSION still appears as installed after uninstall"
  exit 1
fi
echo "PASS: $RUBY_VERSION no longer listed after uninstall"

echo ""
echo "=== rv ruby run $RUBY_VERSION (re-install after uninstall) ==="
"$RV" ruby run "$RUBY_VERSION" -- -e 'puts "re-installed"' | grep -q "re-installed"
echo "PASS: re-installs and runs after uninstall"

# Clean up .ruby-version left by ruby-pin.sh
rm -f .ruby-version
