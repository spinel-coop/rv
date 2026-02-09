# bin/integration-tests/windows/cache-dir.ps1
# Tests rv cache dir — verifies it prints a non-empty directory path.
#
# Usage: .\bin\integration-tests\windows\cache-dir.ps1 <rv-binary-path>

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true
$RV = $args[0]
if (-not $RV) { throw "Usage: cache-dir.ps1 <rv-binary-path>" }

Write-Host "=== rv cache dir ==="
$cache = & $RV cache dir | Out-String
$cache = $cache.Trim()
Write-Host "  $cache"
if (-not $cache) {
    throw "Expected non-empty cache directory path"
}
Write-Host "PASS: rv cache dir prints a non-empty path" -ForegroundColor Green
