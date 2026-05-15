$ErrorActionPreference = "Stop"
$root = Resolve-Path (Join-Path $PSScriptRoot "..")
$manifest = Join-Path $root "server\Cargo.toml"
$outDir = Join-Path $root "bin"
$source = Join-Path $root "server\target\x86_64-pc-windows-gnu\release\canmored.exe"
$target = Join-Path $outDir "canmored.exe"

cargo +stable-x86_64-pc-windows-gnu build --locked --release --manifest-path $manifest --target x86_64-pc-windows-gnu
New-Item -ItemType Directory -Force -Path $outDir | Out-Null
Copy-Item -LiteralPath $source -Destination $target -Force
Write-Host "Built $target"
