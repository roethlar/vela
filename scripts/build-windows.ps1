#!/usr/bin/env pwsh
# Build and verify both Windows installers with the pinned libmpv set.
# Put the CI artifact libmpv-windows-x86_64 in .libmpv/out/libmpv-windows-x86_64
# first (docs/development.md, "Windows installers with the set").
# Run: pwsh -NoProfile -File scripts/build-windows.ps1
# Requires PowerShell 7, x64 Windows/Rust/Python, Node/npm, MSVC and 7-Zip.
# Uses the standard Tauri output directory; no copy to dist/.
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 3.0

function Invoke-Checked {
    param([string] $Program, [string[]] $Arguments)
    & $Program @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "$Program failed with exit code $LASTEXITCODE"
    }
}

if (-not $IsWindows) {
    throw 'This build requires Windows and the x86_64-pc-windows-msvc Rust host.'
}
if ($env:CARGO_TARGET_DIR -or $env:CARGO_BUILD_TARGET) {
    throw 'Unset CARGO_TARGET_DIR and CARGO_BUILD_TARGET: this wrapper checks the standard host bundle directory.'
}

Push-Location (Join-Path $PSScriptRoot '..')
try {
    $rustHost = & rustc -vV
    if ($LASTEXITCODE -ne 0 -or $rustHost -notcontains 'host: x86_64-pc-windows-msvc') {
        throw 'Use the x86_64-pc-windows-msvc Rust toolchain for this DLL set.'
    }
    $set = '.libmpv/out/libmpv-windows-x86_64'
    $check = 'packaging/libmpv/check-package-windows.py'
    Invoke-Checked python @('-B', $check, '--set', $set, '--preflight')
    Invoke-Checked node @('scripts/check-js-toolchain.mjs')
    Invoke-Checked npm @('ci')
    # Remove only the two generated installer directories so an older package
    # cannot satisfy this build's inventory check.
    $bundleDir = 'src-tauri/target/release/bundle'
    foreach ($format in @('nsis', 'msi')) {
        $directory = Join-Path $bundleDir $format
        if (Test-Path -LiteralPath $directory) {
            Remove-Item -LiteralPath $directory -Recurse -Force
        }
    }
    Invoke-Checked npm @('run', 'tauri', '--', 'build', '--bundles', 'nsis,msi',
        '--config', 'src-tauri/tauri.libmpv-windows.conf.json')
    Invoke-Checked python @('-B', $check, '--set', $set, '--bundle-dir', $bundleDir)
    Write-Host "Both checked installers are in $bundleDir/{nsis,msi}."
} finally {
    Pop-Location
}
