# Probe W2: does Vela's own embedded player work on Windows? A debug Vela,
# built from this checkout with probe M2's hook (probes/m2-vela/hook.py,
# applied on the runner only), plays the end-to-end suite's mock Jellyfin
# clip in its own window twice, driven by a script in its page
# (probes/m2-vela/drive.js), with screenshots every two seconds. The libmpv
# set comes from CI's newest successful run on main. The runner is
# disposable: Vela's configuration goes into its real roaming AppData, as
# the Windows end-to-end suite's does. Hosted Windows draws Direct3D 11 with
# WARP, the software adapter.
$ErrorActionPreference = 'Continue'
$here = $PSScriptRoot
$repo = (Resolve-Path (Join-Path $here '../..')).Path
$m2 = Join-Path $repo 'probes/m2-vela'
$out = $env:PROBE_OUT
New-Item -ItemType Directory -Force $out | Out-Null
Set-Location $repo
function Fail([string]$message) {
  "W2: $message" | Tee-Object -FilePath (Join-Path $out 'result.txt') -Append | Write-Host
  exit 1
}

& {
  Get-CimInstance Win32_OperatingSystem | Select-Object Caption, Version, BuildNumber
  Get-CimInstance Win32_VideoController | Select-Object Name, DriverVersion, CurrentHorizontalResolution, CurrentVerticalResolution
} | Out-File (Join-Path $out 'system.txt')

# The set.
$set = Join-Path $repo '.libmpv/out/libmpv-windows-x86_64'
$runId = gh run list --repo $env:GITHUB_REPOSITORY --workflow ci.yml --branch main --status success --limit 1 --json databaseId --jq '.[0].databaseId'
if (-not $runId) { Fail 'no successful CI run on main' }
New-Item -ItemType Directory -Force $set | Out-Null
gh run download $runId --repo $env:GITHUB_REPOSITORY -n libmpv-windows-x86_64 -D $set
if ($LASTEXITCODE -ne 0) { Fail "could not download the set from CI run $runId" }
"the set from CI run $runId" | Out-File -Append (Join-Path $out 'system.txt')
Get-ChildItem $set | Out-File -Append (Join-Path $out 'system.txt')

# Tools: the repository's npm, ffmpeg for the clip.
$tools = Join-Path $out 'tools.log'
npm install --global npm@12.0.2 *> $tools
node scripts/check-js-toolchain.mjs *>> $tools
if ($LASTEXITCODE -ne 0) { Fail 'the JS toolchain check' }
choco install ffmpeg --yes --no-progress --limit-output *>> $tools
$env:PATH = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' + $env:PATH
ffmpeg -version *>> $tools
if ($LASTEXITCODE -ne 0) { Fail 'ffmpeg' }

# A debug Vela with the hook, its page embedded (as the end-to-end suite builds it).
python -B (Join-Path $m2 'hook.py') $repo *>> $tools
if ($LASTEXITCODE -ne 0) { Fail 'the hook' }
npm ci --no-audit --no-fund *> (Join-Path $out 'npm-ci.log')
if ($LASTEXITCODE -ne 0) { Fail 'npm ci' }
npx tauri build --debug --no-bundle *> (Join-Path $out 'build.log')
if ($LASTEXITCODE -ne 0) { Get-Content (Join-Path $out 'build.log') -Tail 60; Fail 'the debug build' }
$bin = Join-Path $repo 'src-tauri/target/debug/vela.exe'
if (-not (Test-Path $bin)) { Fail "no debug binary at $bin" }

# The mock server; Vela's configuration in its real roaming AppData.
$root = Join-Path $env:RUNNER_TEMP 'w2-root'
$configDir = Join-Path $env:APPDATA 'vela\vela\config'
Remove-Item -Recurse -Force (Join-Path $env:APPDATA 'vela') -ErrorAction SilentlyContinue
$mock = Start-Process -FilePath node -ArgumentList @((Join-Path $m2 'serve.mjs'), $root, $configDir) `
  -RedirectStandardOutput (Join-Path $out 'mock.log') -RedirectStandardError (Join-Path $out 'mock-err.log') -PassThru -WindowStyle Hidden
$config = Join-Path $configDir 'config.json'
for ($i = 0; $i -lt 120 -and -not (Test-Path $config); $i++) { Start-Sleep -Seconds 1 }
if (-not (Test-Path $config)) { Get-Content (Join-Path $out 'mock-err.log'); Fail 'the mock server did not write the configuration' }

# Screenshots every two seconds.
$shots = Start-Job -ArgumentList $out -ScriptBlock {
  param($out)
  Add-Type -AssemblyName System.Windows.Forms, System.Drawing
  for ($i = 1; $i -le 45; $i++) {
    try {
      $bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
      $bitmap = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height
      $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
      $graphics.CopyFromScreen($bounds.Location, [System.Drawing.Point]::Empty, $bounds.Size)
      $bitmap.Save((Join-Path $out ('shot-{0:D2}.png' -f $i)))
      $graphics.Dispose(); $bitmap.Dispose()
    } catch { }
    Start-Sleep -Seconds 2
  }
}

# Vela.
$env:VELA_LIBMPV_DIR = $set
$env:VELA_PROBE_SCRIPT = Join-Path $m2 'drive.js'
$env:RUST_BACKTRACE = '1'
$log = Join-Path $out 'vela.log'
$vela = Start-Process -FilePath $bin -RedirectStandardError $log -RedirectStandardOutput (Join-Path $out 'vela-out.log') -PassThru
for ($i = 0; $i -lt 120; $i++) {
  if ((Test-Path $log) -and (Select-String -Path $log -Pattern 'probe: "(DONE|ERROR)' -Quiet)) { break }
  if ($vela.HasExited) { break }
  Start-Sleep -Seconds 1
}
Start-Sleep -Seconds 2
$alive = if ($vela.HasExited) { "no (exit $($vela.ExitCode))" } else { 'yes' }
Stop-Process -Id $vela.Id -Force -ErrorAction SilentlyContinue
Stop-Job $shots -ErrorAction SilentlyContinue
Stop-Process -Id $mock.Id -Force -ErrorAction SilentlyContinue
Copy-Item (Join-Path $root 'mpv.log') (Join-Path $out 'mpv.log') -ErrorAction SilentlyContinue
Copy-Item (Join-Path $configDir 'config.json') $out -ErrorAction SilentlyContinue

Select-String -Path $log -Pattern '^probe' | ForEach-Object Line | Out-File (Join-Path $out 'notes.txt')
"Vela still running at the end: $alive" | Out-File -Append (Join-Path $out 'result.txt')
if (-not (Select-String -Path $log -Pattern 'probe: "DONE' -Quiet)) { Fail 'the drive script did not finish; see notes.txt and vela.log' }
if (-not (Select-String -Path $log -Pattern 'second play: PLAYER CLOSED' -Quiet)) { Fail 'the second play did not close' }
"W2: two plays opened and closed in Vela's window; judge the screenshots" | Tee-Object -FilePath (Join-Path $out 'result.txt') -Append
exit 0
