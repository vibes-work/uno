$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")
$Target = if ($env:TARGET) { $env:TARGET } else { "x86_64-pc-windows-msvc" }
if ($Target -ne "x86_64-pc-windows-msvc") { throw "Expected x86_64-pc-windows-msvc" }
rustup target add $Target
if ($LASTEXITCODE -ne 0) { throw "rustup failed" }
cargo build --release --locked --target $Target --bins
if ($LASTEXITCODE -ne 0) { throw "Build failed" }
$Package = "dist/uno-$Target"
# Recreate staging so removed files cannot leak into a later package.
if (Test-Path -LiteralPath $Package) {
    Remove-Item -LiteralPath $Package -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $Package | Out-Null
Copy-Item "target/$Target/release/uno.exe" $Package -Force
Copy-Item "target/$Target/release/uno-server.exe" $Package -Force
Copy-Item README.md, CONTRIBUTING.md, LICENSE $Package -Force
Copy-Item docs (Join-Path $Package "docs") -Recurse -Force
Compress-Archive -Path $Package -DestinationPath "$Package.zip" -Force
Write-Host "Created $Package.zip (unsigned)"
