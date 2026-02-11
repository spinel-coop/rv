# bin/integration-tests/windows/ci.ps1
# Tests rv ci — sets up a minimal project with Gemfile.lock, runs rv ci,
# and verifies binstub .bat wrappers work.
# Assumes Ruby is already installed via ruby-run.ps1.
#
# Usage: .\bin\integration-tests\windows\ci.ps1 <rv-binary-path>
#
# Note: This script creates a test-project directory in the current working
# directory and cleans it up on success.

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true
$RV = $args[0]
if (-not $RV) { throw "Usage: ci.ps1 <rv-binary-path>" }

# Resolve rv to absolute path before changing directories
# Append .exe if needed, since Resolve-Path requires the exact filename
if (-not (Test-Path $RV) -and (Test-Path "$RV.exe")) { $RV = "$RV.exe" }
$RV = (Resolve-Path $RV).Path

Write-Host "=== Set up test project ==="
$testDir = "test-project"
if (Test-Path $testDir) { Remove-Item -Recurse -Force $testDir }
New-Item -ItemType Directory -Path $testDir | Out-Null

# Copy fixture Gemfile and lockfile
Copy-Item crates\rv-lockfile\tests\inputs\Gemfile.minimal-ruby-project "$testDir\Gemfile"
Copy-Item crates\rv-lockfile\tests\inputs\Gemfile.minimal-ruby-project.lock "$testDir\Gemfile.lock"

Set-Location $testDir

# Pin Ruby version and configure Bundler for local gem install
Set-Content -Path .ruby-version -Value "3.4"
New-Item -ItemType Directory -Path .bundle | Out-Null
"---", 'BUNDLE_PATH: ".rv"' | Set-Content -Path .bundle\config

Write-Host "PASS: test project created" -ForegroundColor Green

Write-Host ""
Write-Host "=== rv ci ==="
& $RV ci
Write-Host "PASS: rv ci exits without error" -ForegroundColor Green

Write-Host ""
Write-Host "=== Test binstub .bat wrappers ==="
$binDir = Get-ChildItem -Path .rv -Recurse -Directory -Filter "bin" | Select-Object -First 1 -ExpandProperty FullName

Write-Host "  rake.bat --version:"
& "$binDir\rake.bat" --version
Write-Host "PASS: rake.bat works" -ForegroundColor Green

Write-Host "  rspec.bat --version:"
& "$binDir\rspec.bat" --version
Write-Host "PASS: rspec.bat works" -ForegroundColor Green

# Clean up — go back to repo root and remove test project
Set-Location ..
Remove-Item -Recurse -Force $testDir

Write-Host ""
Write-Host "PASS: rv ci integration test complete" -ForegroundColor Green
