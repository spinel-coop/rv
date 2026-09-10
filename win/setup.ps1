$ErrorActionPreference = "Stop"

# Install Visual Studio Build Tools (C++ workload) if link.exe is not present
if (-not (Get-Command link.exe -ErrorAction SilentlyContinue)) {
    Write-Host "Installing Visual Studio Build Tools with C++ workload..."
    winget install Microsoft.VisualStudio.2022.BuildTools --accept-source-agreements --accept-package-agreements `
        --override "--wait --quiet --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
    Write-Host "Build Tools installed. You may need to restart your terminal for the changes to take effect."
}

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

# Install editorconfig-checker if not present
if (-not (Get-Command editorconfig-checker -ErrorAction SilentlyContinue)) {
    Write-Host "Installing editorconfig-checker..."
    curl.exe -L --proto "=https" --tlsv1.2 -sSf -o $env:TEMP\editorconfig-checker-windows-amd64.tar.gz https://github.com/editorconfig-checker/editorconfig-checker/releases/download/v4.0.1/editorconfig-checker-windows-amd64.tar.gz
    tar.exe -xzf $env:TEMP\editorconfig-checker-windows-amd64.tar.gz -C $env:TEMP
    New-Item -ItemType Directory -Force -Path "$env:USERPROFILE\.cargo\bin" | Out-Null
    Copy-Item "$env:TEMP\editorconfig-checker.exe" "$env:USERPROFILE\.cargo\bin\editorconfig-checker.exe" -Force
    Write-Host "editorconfig-checker installed to $env:USERPROFILE\.cargo\bin\editorconfig-checker.exe"
}

Write-Host "Setup complete!"
