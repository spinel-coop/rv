# bin/integration-tests/windows/all.ps1
# Runs all Windows integration tests in order.
#
# Usage: .\bin\integration-tests\windows\all.ps1 [rv-binary-path]
#        Defaults to .\target\release\rv.exe if not specified.

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

$repoRoot = Split-Path -Parent (Split-Path -Parent (Split-Path -Parent $PSScriptRoot))

Write-Host ">>> Building rv <<<" -ForegroundColor Cyan
& "$repoRoot\bin\powershell\build.ps1"

$RV = if ($args[0]) { $args[0] } else { "$repoRoot\target\release\rv.exe" }

$tests = @(
    "ruby-run"
    "ruby-list"
    "ruby-find"
    "ruby-pin"
    "shell-env"
    "cache-dir"
    "ruby-uninstall"
    "ci"
)

$passed = 0
$failed = 0

foreach ($test in $tests) {
    Write-Host ""
    Write-Host ">>> Running $test <<<" -ForegroundColor Cyan
    try {
        & "$PSScriptRoot\$test.ps1" $RV
        $passed++
    } catch {
        Write-Host "FAIL: $test - $_" -ForegroundColor Red
        $failed++
        break
    }
}

Write-Host ""
if ($failed -eq 0) {
    Write-Host "All $passed integration tests passed." -ForegroundColor Green
} else {
    Write-Host "$failed failed, $passed passed." -ForegroundColor Red
    exit 1
}
