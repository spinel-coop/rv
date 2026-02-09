# bin/integration-tests/windows/ruby-find.ps1
# Tests rv ruby find.
# Assumes Ruby is already installed via ruby-run.ps1.
#
# Usage: .\bin\integration-tests\windows\ruby-find.ps1 <rv-binary-path>

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true
. "$PSScriptRoot\_config.ps1"

$RV = $args[0]
if (-not $RV) { throw "Usage: ruby-find.ps1 <rv-binary-path>" }

Write-Host "=== rv ruby find $RUBY_VERSION ==="
$find = & $RV ruby find $RUBY_VERSION | Out-String
$find = $find.Trim()
Write-Host "  $find"
if ($find -notmatch "ruby") {
    throw "Expected path containing 'ruby', got: $find"
}
Write-Host "PASS: rv ruby find returns a path to ruby" -ForegroundColor Green
