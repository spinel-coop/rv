$RUBY_VERSION = "3.4.7"

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true
$RV = (Resolve-Path ".\target\debug\rvw.exe").Path

Write-Host "=== rv ruby list --installed-only ==="
$list = & $RV ruby list --installed-only | Out-String
Write-Host $list
if ($list -notmatch [regex]::Escape($RUBY_VERSION)) {
    throw "Expected $RUBY_VERSION in list output"
}
Write-Host "PASS: rv ruby list shows installed $RUBY_VERSION" -ForegroundColor Green


Write-Host "=== rv ruby run $RUBY_VERSION (install + execute) ==="
$output = & $RV ruby run $RUBY_VERSION -- -e "puts RUBY_DESCRIPTION" | Out-String
Write-Host $output
if ($output -notmatch [regex]::Escape($RUBY_VERSION)) {
    throw "Expected output to contain $RUBY_VERSION, got: $output"
}
Write-Host "PASS: rv ruby run installed and executed Ruby $RUBY_VERSION" -ForegroundColor Green


Write-Host "=== rv ruby list --installed-only --format json ==="
$json = & $RV ruby list --installed-only --format json | Out-String
Write-Host $json
if ($json -notmatch '"version"') {
    throw "Expected JSON output with 'version' field"
}
Write-Host "PASS: rv ruby list --format json outputs valid JSON" -ForegroundColor Green


Write-Host "=== rv ruby find $RUBY_VERSION ==="
$find = & $RV ruby find $RUBY_VERSION | Out-String
$find = $find.Trim()
Write-Host "  $find"
if ($find -notmatch "ruby") {
    throw "Expected path containing 'ruby', got: $find"
}
Write-Host "PASS: rv ruby find returns a path to ruby" -ForegroundColor Green


Write-Host "=== rv ruby pin $RUBY_VERSION ==="
& $RV ruby pin $RUBY_VERSION
$pin = Get-Content .ruby-version
Write-Host "  .ruby-version contains: $pin"
if ($pin -notmatch [regex]::Escape($RUBY_VERSION)) {
    throw "Expected .ruby-version to contain $RUBY_VERSION, got: $pin"
}
Write-Host "PASS: rv ruby pin writes correct .ruby-version" -ForegroundColor Green


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


Write-Host "=== rv cache dir ==="
$cache = & $RV cache dir | Out-String
$cache = $cache.Trim()
Write-Host "  $cache"
if (-not $cache) {
    throw "Expected non-empty cache directory path"
}
Write-Host "PASS: rv cache dir prints a non-empty path" -ForegroundColor Green

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


Write-Host "=== rv ruby run $RUBY_VERSION (re-install after uninstall) ==="
$reinstall = & $RV ruby run $RUBY_VERSION -- -e "puts 're-installed'" | Out-String
if ($reinstall -notmatch "re-installed") {
    throw "Re-install failed: $reinstall"
}
Write-Host "PASS: re-installs and runs after uninstall" -ForegroundColor Green

# Clean up .ruby-version left by ruby-pin.ps1
Remove-Item .ruby-version -ErrorAction SilentlyContinue


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
