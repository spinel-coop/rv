$ErrorActionPreference = "Stop"

# Install rustup via winget if not present
if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) {
    Write-Host "Installing rustup via winget..."
    winget install Rustlang.Rustup --accept-source-agreements --accept-package-agreements
    # Refresh PATH so we can find rustup and cargo
    $env:Path = [System.Environment]::GetEnvironmentVariable("Path", "Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path", "User")

    Write-Host "Installing stable Rust toolchain..."
    rustup install stable
}

Write-Host "Using Rust: $(rustc --version)"

# Install cargo-binstall if not present
if (-not (Get-Command cargo-binstall -ErrorAction SilentlyContinue)) {
    Write-Host "Installing cargo-binstall..."
    Set-ExecutionPolicy Unrestricted -Scope Process
    Invoke-WebRequest -Uri "https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.ps1" | Invoke-Expression
}

# Install cargo-nextest if not present
if (-not (Get-Command cargo-nextest -ErrorAction SilentlyContinue)) {
    Write-Host "Installing cargo-nextest..."
    cargo binstall cargo-nextest -y
}

# Install cargo-insta if not present
if (-not (Get-Command cargo-insta -ErrorAction SilentlyContinue)) {
    Write-Host "Installing cargo-insta..."
    cargo binstall cargo-insta -y
}

# Install cargo-dist if not present
if (-not (Get-Command dist -ErrorAction SilentlyContinue)) {
    Write-Host "Installing cargo-dist..."
    cargo binstall cargo-dist -y
}

Write-Host "Setup complete!"
