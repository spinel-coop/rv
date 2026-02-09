# bin/integration-tests/windows/shell-env.ps1
# Tests rv shell env powershell — verifies RUBY_ROOT and PATH in output.
# Assumes Ruby is already installed via ruby-run.ps1.
#
# Usage: .\bin\integration-tests\windows\shell-env.ps1 <rv-binary-path>

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true
$RV = $args[0]
if (-not $RV) { throw "Usage: shell-env.ps1 <rv-binary-path>" }

Write-Host "=== rv shell env powershell ==="
$env_out = & $RV shell env powershell | Out-String
Write-Host $env_out
if ($env_out -notmatch "RUBY_ROOT") {
    throw "Expected RUBY_ROOT in shell env output"
}
if ($env_out -notmatch "PATH") {
    throw "Expected PATH in shell env output"
}
Write-Host "PASS: rv shell env powershell outputs RUBY_ROOT and PATH" -ForegroundColor Green
