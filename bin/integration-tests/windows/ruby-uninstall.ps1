# bin/integration-tests/windows/ruby-uninstall.ps1
# Tests rv ruby uninstall — removes Ruby, verifies it's gone, re-installs.
# Assumes Ruby is already installed via ruby-run.ps1.
#
# Usage: .\bin\integration-tests\windows\ruby-uninstall.ps1 <rv-binary-path>

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true
$RV = $args[0]
if (-not $RV) { throw "Usage: ruby-uninstall.ps1 <rv-binary-path>" }

$RUBY_VERSION = "3.4.4"

Write-Host "=== rv ruby uninstall $RUBY_VERSION ==="
& $RV ruby uninstall $RUBY_VERSION
Write-Host "PASS: rv ruby uninstall exits without error" -ForegroundColor Green

Write-Host ""
Write-Host "=== Verify $RUBY_VERSION is no longer listed ==="
$list_after = & $RV ruby list --installed-only | Out-String
if ($list_after -match [regex]::Escape($RUBY_VERSION)) {
    throw "$RUBY_VERSION still appears in installed list after uninstall"
}
Write-Host "PASS: $RUBY_VERSION no longer listed after uninstall" -ForegroundColor Green

Write-Host ""
Write-Host "=== rv ruby run $RUBY_VERSION (re-install after uninstall) ==="
$reinstall = & $RV ruby run $RUBY_VERSION -- -e "puts 're-installed'" | Out-String
if ($reinstall -notmatch "re-installed") {
    throw "Re-install failed: $reinstall"
}
Write-Host "PASS: re-installs and runs after uninstall" -ForegroundColor Green

# Clean up .ruby-version left by ruby-pin.ps1
Remove-Item .ruby-version -ErrorAction SilentlyContinue
