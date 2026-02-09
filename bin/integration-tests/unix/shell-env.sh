#!/usr/bin/env bash
# bin/integration-tests/unix/shell-env.sh
# Tests rv shell env bash — verifies RUBY_ROOT and PATH in output.
# Assumes Ruby is already installed via ruby-run.sh.
#
# Usage: ./bin/integration-tests/unix/shell-env.sh <rv-binary-path>

set -euo pipefail

RV="${1:?Usage: shell-env.sh <rv-binary-path>}"

echo "=== rv shell env bash ==="
env_out=$("$RV" shell env bash)
echo "$env_out"
echo "$env_out" | grep -q "RUBY_ROOT"
echo "$env_out" | grep -q "export PATH="
echo "PASS: rv shell env bash outputs RUBY_ROOT and PATH exports"
