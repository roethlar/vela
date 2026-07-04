#!/usr/bin/env pwsh
# Build Vela's installable bundle for whatever this host can actually produce.
#
# Tauri does not cross-compile in practice, so each machine builds for itself:
#   Windows -> NSIS installer     (src-tauri/target/release/bundle/nsis, *-setup.exe)
#   macOS   -> .app inside a .dmg (universal by default; dmg/ + macos/)
#   Linux, Debian/Ubuntu family -> AppImage (.../bundle/appimage)
#   Linux, Arch family          -> native pacman package via `npm run build:arch`
#                                  (packaging/arch/*.pkg.tar.zst)
#
# Why Linux splits by distro: linuxdeploy's AppImage tooling assumes a Debian-
# family layout (and an older glibc). On Arch it breaks - modern gdk-pixbuf has
# no external loader dir for the GTK plugin to copy, and RELR relocations defeat
# its bundled `strip`. The repo's Arch package (packaging/arch/PKGBUILD) is the
# supported path there; portable AppImages are built on ubuntu in CI. Pass
# --bundles to override the host default (e.g. force appimage, or build deb,rpm).
#
# This script does NOT change the version. A build is only meaningfully unique
# when the source is, so the version is bumped when the code changes (run
# scripts/bump.sh as part of a code change), not here at build time.
#
# Run from anywhere; it cd's to the repo root.
#
# mpv is intentionally NOT bundled - Vela detects it at runtime and offers to
# install it, keeping these packages small and the player user-updatable.
#
# Usage:
#   pwsh scripts/build.ps1                 # build the host's default bundle
#   pwsh scripts/build.ps1 --native        # macOS: host arch only (skip universal)
#   pwsh scripts/build.ps1 --bundles deb,rpm,appimage   # override the bundle targets

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 3.0

function Show-Help {
  @'
Build Vela's installable bundle for whatever this host can actually produce.

Tauri does not cross-compile in practice, so each machine builds for itself:
  Windows -> NSIS installer     (src-tauri/target/release/bundle/nsis, *-setup.exe)
  macOS   -> .app inside a .dmg (universal by default; dmg/ + macos/)
  Linux, Debian/Ubuntu family -> AppImage (.../bundle/appimage)
  Linux, Arch family          -> native pacman package via `npm run build:arch`
                                 (packaging/arch/*.pkg.tar.zst)

Why Linux splits by distro: linuxdeploy's AppImage tooling assumes a Debian-
family layout (and an older glibc). On Arch it breaks - modern gdk-pixbuf has
no external loader dir for the GTK plugin to copy, and RELR relocations defeat
its bundled `strip`. The repo's Arch package (packaging/arch/PKGBUILD) is the
supported path there; portable AppImages are built on ubuntu in CI. Pass
--bundles to override the host default (e.g. force appimage, or build deb,rpm).

This script does NOT change the version. A build is only meaningfully unique
when the source is, so the version is bumped when the code changes (run
scripts/bump.sh as part of a code change), not here at build time.

Run from anywhere; it cd's to the repo root.

mpv is intentionally NOT bundled - Vela detects it at runtime and offers to
install it, keeping these packages small and the player user-updatable.

Usage:
  pwsh scripts/build.ps1                 # build the host's default bundle
  pwsh scripts/build.ps1 --native        # macOS: host arch only (skip universal)
  pwsh scripts/build.ps1 --bundles deb,rpm,appimage   # override the bundle targets
'@
}

function Invoke-Checked {
  param(
    [Parameter(Mandatory = $true)]
    [string] $FilePath,

    [string[]] $ArgumentList = @(),

    [switch] $SuppressOutput
  )

  if ($SuppressOutput) {
    & $FilePath @ArgumentList | Out-Null
  } else {
    & $FilePath @ArgumentList
  }

  if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
  }
}

function Enable-AppImageWorkarounds {
  $env:APPIMAGE_EXTRACT_AND_RUN = '1'
  $env:NO_STRIP = '1'
}

function Test-ArchLinuxFamily {
  $osRelease = '/etc/os-release'
  if (-not (Test-Path -LiteralPath $osRelease -PathType Leaf)) {
    return $false
  }

  foreach ($line in Get-Content -LiteralPath $osRelease) {
    if ($line -match '^(ID|ID_LIKE)=.*\barch\b') {
      return $true
    }
  }

  return $false
}

function Test-PlatformVariable {
  param([string] $Name)

  $variable = Get-Variable -Name $Name -Scope Global -ErrorAction SilentlyContinue
  return ($null -ne $variable -and [bool] $variable.Value)
}

function Get-HostOs {
  if ((Test-PlatformVariable 'IsWindows') -or $env:OS -eq 'Windows_NT') {
    return 'windows'
  }
  if (Test-PlatformVariable 'IsMacOS') {
    return 'macos'
  }
  if (Test-PlatformVariable 'IsLinux') {
    return 'linux'
  }

  $uname = Get-Command uname -ErrorAction SilentlyContinue
  if ($null -ne $uname) {
    switch -Wildcard (& uname -s) {
      'Linux*' { return 'linux' }
      'Darwin*' { return 'macos' }
      'MINGW*' { return 'windows' }
      'MSYS*' { return 'windows' }
      'CYGWIN*' { return 'windows' }
    }
  }

  throw 'Unsupported host OS'
}

function Test-ArtifactName {
  param(
    [string] $Name,
    [string[]] $Patterns
  )

  foreach ($pattern in $Patterns) {
    if ($Name -like $pattern) {
      return $true
    }
  }

  return $false
}

function Convert-BundlesValue {
  param([object] $Value)

  if ($Value -is [array]) {
    return (($Value | ForEach-Object { [string] $_ }) -join ',')
  }

  return [string] $Value
}

function Normalize-Arguments {
  param([object[]] $RawArgs)

  $normalized = @()

  foreach ($rawArg in $RawArgs) {
    if ($rawArg -is [array]) {
      $parts = @($rawArg | ForEach-Object { [string] $_ })
      if ($parts.Count -eq 0) {
        continue
      }

      if ($parts[0].StartsWith('--bundles=')) {
        $valueParts = @()
        $firstValue = $parts[0].Substring(('--bundles=').Length)
        if ($firstValue.Length -gt 0) {
          $valueParts += $firstValue
        }
        if ($parts.Count -gt 1) {
          $valueParts += $parts[1..($parts.Count - 1)]
        }
        $normalized += "--bundles=$($valueParts -join ',')"
      } else {
        $normalized += ($parts -join ',')
      }
    } else {
      $normalized += [string] $rawArg
    }
  }

  return $normalized
}

Set-Location -LiteralPath (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..'))

$universal = $true
$bundles = ''
$parsedArgs = @(Normalize-Arguments -RawArgs $args)

$argIndex = 0
while ($argIndex -lt $parsedArgs.Count) {
  $arg = $parsedArgs[$argIndex]

  if ($arg -eq '--native') {
    $universal = $false
  } elseif ($arg -eq '--bundles') {
    $argIndex++
    if ($argIndex -ge $parsedArgs.Count -or [string]::IsNullOrWhiteSpace((Convert-BundlesValue $parsedArgs[$argIndex]))) {
      [Console]::Error.WriteLine('--bundles needs a value')
      exit 2
    }
    $bundles = Convert-BundlesValue $parsedArgs[$argIndex]
  } elseif ($arg.StartsWith('--bundles=')) {
    $bundles = $arg.Substring(('--bundles=').Length)
    if ([string]::IsNullOrWhiteSpace($bundles)) {
      [Console]::Error.WriteLine('--bundles needs a value')
      exit 2
    }
  } elseif ($arg -eq '-h' -or $arg -eq '--help') {
    Show-Help
    exit 0
  } else {
    [Console]::Error.WriteLine("Unknown option: $arg (try --help)")
    exit 2
  }

  $argIndex++
}

$os = Get-HostOs
$mode = 'tauri'
$extraArgs = @()
$bundleDir = 'src-tauri/target/release/bundle'
$subdirs = @()

switch ($os) {
  'linux' {
    if (-not [string]::IsNullOrEmpty($bundles)) {
      $subdirs = @('appimage', 'deb', 'rpm')
      Enable-AppImageWorkarounds
    } elseif (Test-ArchLinuxFamily) {
      $mode = 'arch'
      $bundleDir = 'packaging/arch'
      $subdirs = @('.')
    } else {
      $bundles = 'appimage'
      $subdirs = @('appimage')
      Enable-AppImageWorkarounds
    }
  }
  'windows' {
    if ([string]::IsNullOrEmpty($bundles)) {
      $bundles = 'nsis'
    }
    $subdirs = @('nsis')
  }
  'macos' {
    if ([string]::IsNullOrEmpty($bundles)) {
      $bundles = 'dmg'
    }
    $subdirs = @('dmg', 'macos')

    if ($universal) {
      $extraArgs = @('--target', 'universal-apple-darwin')
      $bundleDir = 'src-tauri/target/universal-apple-darwin/release/bundle'

      if ($null -ne (Get-Command rustup -ErrorAction SilentlyContinue)) {
        Invoke-Checked -FilePath 'rustup' -ArgumentList @('target', 'add', 'aarch64-apple-darwin', 'x86_64-apple-darwin') -SuppressOutput
      }
    }
  }
}

$nodeModulesMarker = Join-Path 'node_modules' '.package-lock.json'
$depsMissing = -not (Test-Path -LiteralPath 'node_modules' -PathType Container)
$depsStale = $false

if (Test-Path -LiteralPath 'package-lock.json' -PathType Leaf) {
  if (-not (Test-Path -LiteralPath $nodeModulesMarker -PathType Leaf)) {
    $depsStale = $true
  } else {
    $packageLock = Get-Item -LiteralPath 'package-lock.json'
    $installedLock = Get-Item -LiteralPath $nodeModulesMarker -Force
    $depsStale = $packageLock.LastWriteTimeUtc -gt $installedLock.LastWriteTimeUtc
  }
}

if ($depsMissing -or $depsStale) {
  Write-Host '==> JS deps missing or stale; running npm install'
  Invoke-Checked -FilePath 'npm' -ArgumentList @('install')
}

if ($mode -eq 'arch') {
  Write-Host '==> Arch host: building native pacman package (npm run build:arch)'
  Invoke-Checked -FilePath 'npm' -ArgumentList @('run', 'build:arch')
} else {
  $extraArgsText = ''
  if ($extraArgs.Count -gt 0) {
    $extraArgsText = "   $($extraArgs -join ' ')"
  }
  Write-Host "==> Host: $os   bundles: $bundles$extraArgsText"
  Invoke-Checked -FilePath 'npm' -ArgumentList (@('run', 'tauri', '--', 'build', '--bundles', $bundles) + $extraArgs)
}

Write-Host
Write-Host "==> Artifacts in ${bundleDir}:"
$found = $false
$artifactPatterns = @(
  '*.AppImage',
  '*.dmg',
  '*.app',
  '*-setup.exe',
  '*.msi',
  '*.deb',
  '*.rpm',
  '*.pkg.tar.zst',
  '*.pkg.tar.xz'
)

foreach ($subdir in $subdirs) {
  if ($subdir -eq '.') {
    $dir = $bundleDir
  } else {
    $dir = Join-Path $bundleDir $subdir
  }

  if (-not (Test-Path -LiteralPath $dir -PathType Container)) {
    continue
  }

  $artifacts = Get-ChildItem -LiteralPath $dir -Force |
    Where-Object { Test-ArtifactName -Name $_.Name -Patterns $artifactPatterns } |
    Sort-Object -Property Name

  foreach ($artifact in $artifacts) {
    Write-Host "    $(Join-Path $dir $artifact.Name)"
    $found = $true
  }
}

if (-not $found) {
  Write-Host '    (nothing matched - check the build output above)'
}
