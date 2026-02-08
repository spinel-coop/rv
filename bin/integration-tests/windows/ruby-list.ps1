# bin/integration-tests/windows/ruby-list.ps1
# Tests rv ruby list and rv ruby list --format json.
# Assumes Ruby is already installed via ruby-run.ps1.
#
# Usage: .\bin\integration-tests\windows\ruby-list.ps1 <rv-binary-path>

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true
$RV = $args[0]
if (-not $RV) { throw "Usage: ruby-list.ps1 <rv-binary-path>" }

$RUBY_VERSION = "3.4.4"

Write-Host "=== rv ruby list --installed-only ==="
$list = & $RV ruby list --installed-only | Out-String
Write-Host $list
if ($list -notmatch [regex]::Escape($RUBY_VERSION)) {
    throw "Expected $RUBY_VERSION in list output"
}
Write-Host "PASS: rv ruby list shows installed $RUBY_VERSION" -ForegroundColor Green

Write-Host ""
Write-Host "=== rv ruby list --installed-only --format json ==="
$json = & $RV ruby list --installed-only --format json | Out-String
Write-Host $json
if ($json -notmatch '"version"') {
    throw "Expected JSON output with 'version' field"
}
Write-Host "PASS: rv ruby list --format json outputs valid JSON" -ForegroundColor Green
