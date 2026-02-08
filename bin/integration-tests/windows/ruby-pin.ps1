# bin/integration-tests/windows/ruby-pin.ps1
# Tests rv ruby pin — writes and verifies .ruby-version file.
# Assumes Ruby is already installed via ruby-run.ps1.
#
# Usage: .\bin\integration-tests\windows\ruby-pin.ps1 <rv-binary-path>

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true
. "$PSScriptRoot\_config.ps1"

$RV = $args[0]
if (-not $RV) { throw "Usage: ruby-pin.ps1 <rv-binary-path>" }

Write-Host "=== rv ruby pin $RUBY_VERSION ==="
& $RV ruby pin $RUBY_VERSION
$pin = Get-Content .ruby-version
Write-Host "  .ruby-version contains: $pin"
if ($pin -notmatch [regex]::Escape($RUBY_VERSION)) {
    throw "Expected .ruby-version to contain $RUBY_VERSION, got: $pin"
}
Write-Host "PASS: rv ruby pin writes correct .ruby-version" -ForegroundColor Green

# Note: .ruby-version is intentionally left in place for subsequent tests
# (e.g. shell-env.ps1 needs it). Cleaned up by ruby-uninstall.ps1.
