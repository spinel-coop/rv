# bin/integration-tests/windows/ruby-find.ps1
# Tests rv ruby find.
# Assumes Ruby is already installed via ruby-run.ps1.
#
# Usage: .\bin\integration-tests\windows\ruby-find.ps1 <rv-binary-path>

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true
$RV = $args[0]
if (-not $RV) { throw "Usage: ruby-find.ps1 <rv-binary-path>" }

$RUBY_VERSION = "3.4.4"

Write-Host "=== rv ruby find $RUBY_VERSION ==="
$find = & $RV ruby find $RUBY_VERSION | Out-String
$find = $find.Trim()
Write-Host "  $find"
if ($find -notmatch "ruby") {
    throw "Expected path containing 'ruby', got: $find"
}
Write-Host "PASS: rv ruby find returns a path to ruby" -ForegroundColor Green
