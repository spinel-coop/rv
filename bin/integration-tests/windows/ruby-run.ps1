# bin/integration-tests/windows/ruby-run.ps1
# Downloads and runs Ruby via rv ruby run (RubyInstaller2).
# This is the "setup" step — must run before other integration tests.
#
# Usage: .\bin\integration-tests\windows\ruby-run.ps1 <rv-binary-path>

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true
$RV = $args[0]
if (-not $RV) { throw "Usage: ruby-run.ps1 <rv-binary-path>" }

$RUBY_VERSION = "3.4.4"

Write-Host "=== rv ruby run $RUBY_VERSION (install + execute) ==="
$output = & $RV ruby run $RUBY_VERSION -- -e "puts RUBY_DESCRIPTION" | Out-String
Write-Host $output
if ($output -notmatch [regex]::Escape($RUBY_VERSION)) {
    throw "Expected output to contain $RUBY_VERSION, got: $output"
}
Write-Host "PASS: rv ruby run installed and executed Ruby $RUBY_VERSION" -ForegroundColor Green
