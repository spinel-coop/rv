$ErrorActionPreference = "Stop"

cargo clippy --all-targets --all-features @args -- -D warnings
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

if ($args.Count -gt 0 -and $args[0] -eq "--fix") {
    cargo fmt --all
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    if (Get-Command editorconfig-checker -ErrorAction SilentlyContinue) {
        editorconfig-checker --fix
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    } else {
        Write-Error "editorconfig-checker not found. Run win/setup.ps1 or: https://github.com/editorconfig-checker/editorconfig-checker/releases"
        exit 1
    }
} else {
    cargo fmt --all -- --check
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    if (Get-Command editorconfig-checker -ErrorAction SilentlyContinue) {
        editorconfig-checker
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    } else {
        Write-Error "editorconfig-checker not found. Run win/setup.ps1 or: https://github.com/editorconfig-checker/editorconfig-checker/releases"
        exit 1
    }
}
