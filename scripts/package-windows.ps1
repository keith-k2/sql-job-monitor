# Builds the Windows MSI from Cargo.toml and copies it to dist/.
param(
    [string]$BinPath = "$env:LOCALAPPDATA\sql-job-monitor\wix314"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$version = (Select-String -Path "$root\Cargo.toml" -Pattern '^version = "([^"]+)"').Matches[0].Groups[1].Value
$cargo = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"
if (-not (Test-Path $cargo)) {
    $cargo = "cargo"
}

if (Test-Path $BinPath) {
    & $cargo wix --bin-path $BinPath
} else {
    & $cargo wix
}

New-Item -ItemType Directory -Force (Join-Path $root "dist") | Out-Null
$msi = Join-Path $root "target\wix\sql-job-monitor-$version-x86_64.msi"
Copy-Item $msi (Join-Path $root "dist\sql-job-monitor-$version-x86_64.msi") -Force
Write-Output (Join-Path $root "dist\sql-job-monitor-$version-x86_64.msi")
