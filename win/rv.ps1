$ErrorActionPreference = "Stop"

$dir = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)

Write-Host "+ cargo build --release -q --bin rv"
Push-Location $dir
try {
    cargo build --release -q --bin rv
} finally {
    Pop-Location
}

& "$dir\target\release\rv.exe" @args
