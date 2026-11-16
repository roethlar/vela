#!/bin/bash
# Built and run only on the interactive Windows platform-probe runner.
set -euo pipefail
fail() { echo "w0-embed: $*" >&2; exit 1; }
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*) ;;
  *) fail "this probe requires Windows (Git Bash)" ;;
esac
probe_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
out="$(cygpath -au "${PROBE_OUT:?}")"
mkdir -p "$out" "$probe_dir/download"
# Native programs need native paths even when the caller supplied /d/... .
export PROBE_OUT="$(cygpath -am "$out")"
export CARGO_TARGET_DIR="$(cygpath -am "$probe_dir/target")"
export W0_DOWNLOAD="$(cygpath -am "$probe_dir/download")"
export W0_EXE="$(cygpath -am "$probe_dir/target/release/w0-embed.exe")"

curl -fsSL --retry 2 https://api.github.com/repos/zhongfly/mpv-winbuild/releases/latest \
  -o "$out/release.json" || fail "could not read the public latest-release API (no token is used)"
if ! pwsh -NoProfile -Command '
  $ErrorActionPreference = "Stop"
  $release = Get-Content -Raw -LiteralPath "$env:PROBE_OUT/release.json" | ConvertFrom-Json
  $assets = @($release.assets | Where-Object { $_.name -like "mpv-dev-x86_64-2*.7z" })
  if ($assets.Count -ne 1) {
    throw "Expected one plain x86_64 mpv-dev-x86_64-2*.7z asset in release $($release.tag_name); found $($assets.Count). Available: $($release.assets.name -join ", ")"
  }
  if (!$release.tag_name -or !$assets[0].browser_download_url) { throw "Incomplete release metadata" }
  "release: $($release.tag_name)`nasset: $($assets[0].name)" |
    Set-Content -Encoding utf8 -LiteralPath "$env:PROBE_OUT/libmpv.txt"
  $assets[0].browser_download_url |
    Set-Content -Encoding utf8 -LiteralPath "$env:PROBE_OUT/libmpv-url.txt"
'; then
  fail "could not select the libmpv release asset"
fi
asset_url="$(tr -d '\r\n' < "$out/libmpv-url.txt")"
curl -fsSL --retry 2 "$asset_url" -o "$probe_dir/download/mpv.7z" \
  || fail "could not download $asset_url"
if ! 7z x -y "-o$W0_DOWNLOAD/libmpv" "$W0_DOWNLOAD/mpv.7z" > "$out/extract.log" 2>&1; then
  tail -60 "$out/extract.log"
  fail "libmpv extraction failed"
fi
if ! (cd "$probe_dir" && cargo build --release) > "$out/build.log" 2>&1; then
  tail -60 "$out/build.log"
  fail "cargo build failed"
fi
if ! pwsh -NoProfile -Command '
  $ErrorActionPreference = "Stop"
  $dlls = @(Get-ChildItem -LiteralPath "$env:W0_DOWNLOAD/libmpv" -Recurse -File -Filter libmpv-2.dll)
  if ($dlls.Count -ne 1) { throw "Expected one libmpv-2.dll; found $($dlls.Count)" }
  Copy-Item -LiteralPath $dlls[0].FullName -Destination (Join-Path (Split-Path $env:W0_EXE) "libmpv-2.dll")
'; then
  fail "could not install libmpv-2.dll beside the probe executable"
fi

cat > "$out/run-variant.ps1" <<'PS'
param([ValidateSet('toplevel', 'child')][string]$Variant)
$ErrorActionPreference = 'Stop'
$out = $env:PROBE_OUT
try {
  $process = Start-Process -FilePath $env:W0_EXE -ArgumentList $Variant -PassThru `
    -RedirectStandardOutput "$out/stdout-$Variant.log" -RedirectStandardError "$out/stderr-$Variant.log"
} catch {
  "$Variant start_failed: $_" | Add-Content -LiteralPath "$out/result.txt"
  Write-Error $_
  exit 1
}
$clock = [System.Diagnostics.Stopwatch]::StartNew()
$screenshot = 'ok'
try {
  Start-Sleep -Seconds 8
  # Same whole-desktop CopyFromScreen method as probes/screenshot/run.sh.
  Add-Type -AssemblyName System.Windows.Forms, System.Drawing
  $bounds = [System.Windows.Forms.SystemInformation]::VirtualScreen
  $bitmap = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height
  try {
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
      $graphics.CopyFromScreen($bounds.Left, $bounds.Top, 0, 0, $bitmap.Size)
      $bitmap.Save("$out/screen-$Variant.png", [System.Drawing.Imaging.ImageFormat]::Png)
    } finally { $graphics.Dispose() }
  } finally { $bitmap.Dispose() }
} catch {
  $screenshot = 'failed'
  Write-Warning "Screenshot for ${Variant}: $_"
}
# A 20-second total deadline includes the initial screenshot delay.
$remaining = [Math]::Max(0, 20000 - [int]$clock.ElapsedMilliseconds)
$timedOut = !$process.WaitForExit($remaining)
if ($timedOut) {
  Write-Warning "$Variant did not exit in 20 seconds; killing its process tree"
  try { $process.Kill($true) } catch {
    # It may have exited between WaitForExit and Kill.
    if (!$process.HasExited) {
      "$Variant exit=unavailable timeout=True kill_failed: $_" | Add-Content -LiteralPath "$out/result.txt"
      throw
    }
  }
  if (!$process.WaitForExit(2000)) {
    "$Variant exit=unavailable timeout=True reap_failed" | Add-Content -LiteralPath "$out/result.txt"
    throw "Could not reap $Variant after killing it"
  }
}
$code = $process.ExitCode
"$Variant exit=$code timeout=$timedOut screenshot=$screenshot" | Add-Content -LiteralPath "$out/result.txt"
$process.Dispose()
if ($timedOut -or $code -ne 0 -or $screenshot -ne 'ok') { exit 1 }
PS

: > "$out/result.txt"
status=0
for variant in toplevel child; do
  # Start-Process launches asynchronously; PowerShell owns the native Windows PID
  # so timeout/kill also works when Bash's PID is different from the native PID.
  pwsh -NoProfile -File "$out/run-variant.ps1" -Variant "$variant" || status=1
done
cat "$out/result.txt"
exit "$status"
