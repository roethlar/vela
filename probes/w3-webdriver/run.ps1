# Probe W3: why does the Windows end-to-end suite get no WebDriver session?
# In its first full run (e2e-windows.yml, 43964966316) Edge WebDriver waited
# for Vela's WebView2 and POST /session timed out in every scenario. Here a
# debug Vela built from this checkout is started six ways, one at a time,
# with no scenario around it:
#   A, B: through tauri-driver and Edge WebDriver, as the suite does, with
#         and without the suite's pin of the WebView2 runtime folder
#         (WEBVIEW2_BROWSER_EXECUTABLE_FOLDER);
#   C, D: through Edge WebDriver alone, with its verbose log, pinned and not;
#   E, F: Vela alone, asked for a DevTools port through WebView2's own
#         environment variable, its standard error kept, pinned and not.
# For each: the session's answer (or the error), the processes (vela.exe,
# msedgewebview2.exe and the drivers, with their command lines) every three
# seconds, a screenshot, and the Application event log's errors.
$ErrorActionPreference = 'Continue'
$here = $PSScriptRoot
$repo = (Resolve-Path (Join-Path $here '../..')).Path
$out = $env:PROBE_OUT
New-Item -ItemType Directory -Force $out | Out-Null
Set-Location $repo
function Note([string]$message) {
  "W3: $message" | Tee-Object -FilePath (Join-Path $out 'result.txt') -Append | Write-Host
}
function Shot([string]$file) {
  try {
    Add-Type -AssemblyName System.Windows.Forms, System.Drawing
    $bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    $bitmap = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $graphics.CopyFromScreen($bounds.Location, [System.Drawing.Point]::Empty, $bounds.Size)
    $bitmap.Save($file)
    $graphics.Dispose(); $bitmap.Dispose()
  } catch { "screenshot failed: $_" | Out-File -Append (Join-Path $out 'result.txt') }
}
function StopOurs {
  Get-Process vela, msedgedriver, tauri-driver -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
  Start-Sleep -Seconds 2
}

& {
  Get-CimInstance Win32_OperatingSystem | Select-Object Caption, Version, BuildNumber
  whoami /user
} | Out-File (Join-Path $out 'system.txt')

$tools = Join-Path $out 'tools.log'
npm install --global npm@12.0.2 *> $tools
node scripts/check-js-toolchain.mjs *>> $tools
if ($LASTEXITCODE -ne 0) { Note 'the JS toolchain check failed'; exit 1 }
cargo install tauri-driver --locked *>> $tools
$tauriDriver = Join-Path $env:USERPROFILE '.cargo\bin\tauri-driver.exe'
if (-not (Test-Path $tauriDriver)) { Note 'no tauri-driver'; exit 1 }
$fetched = pwsh -NoProfile -NonInteractive -File tests/e2e/fetch-driver.ps1 2>> $tools
if ($LASTEXITCODE -ne 0) { Note 'fetch-driver.ps1 failed (tools.log)'; exit 1 }
$edge = $fetched | ConvertFrom-Json
Note "Edge WebDriver $($edge.version); WebView2 runtime folder $($edge.runtimeDir)"

npm ci --no-audit --no-fund *> (Join-Path $out 'npm-ci.log')
if ($LASTEXITCODE -ne 0) { Note 'npm ci failed'; exit 1 }
npx tauri build --debug --no-bundle *> (Join-Path $out 'build.log')
$bin = Join-Path $repo 'src-tauri\target\debug\vela.exe'
if (-not (Test-Path $bin)) { Get-Content (Join-Path $out 'build.log') -Tail 40; Note "no debug binary at $bin"; exit 1 }

function Attempt([string]$name, [string]$how, [bool]$pinned) {
  $dir = Join-Path $out $name
  New-Item -ItemType Directory -Force $dir | Out-Null
  StopOurs
  Remove-Item -Recurse -Force (Join-Path $env:APPDATA 'vela') -ErrorAction SilentlyContinue
  if ($pinned) { $env:WEBVIEW2_BROWSER_EXECUTABLE_FOLDER = $edge.runtimeDir }
  else { Remove-Item Env:WEBVIEW2_BROWSER_EXECUTABLE_FOLDER -ErrorAction SilentlyContinue }
  Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue
  $env:TAURI_WEBVIEW_AUTOMATION = 'true'
  $env:RUST_BACKTRACE = '1'
  $started = Get-Date
  $sampler = Start-Job -ArgumentList $dir -ScriptBlock {
    param($dir)
    for ($i = 0; $i -lt 26; $i++) {
      $t = Get-Date -Format 'HH:mm:ss'
      Get-CimInstance Win32_Process |
        Where-Object { $_.Name -in 'vela.exe', 'msedgewebview2.exe', 'msedgedriver.exe', 'tauri-driver.exe', 'WerFault.exe' } |
        ForEach-Object { "$t $($_.ProcessId)<-$($_.ParentProcessId) $($_.Name) $($_.CommandLine)" } |
        Out-File -Append (Join-Path $dir 'processes.txt')
      "-- $t" | Out-File -Append (Join-Path $dir 'processes.txt')
      Start-Sleep -Seconds 3
    }
  }
  $driver = $null; $vela = $null; $port = 0
  if ($how -eq 'tauri-driver') {
    $port = 4444
    $driver = Start-Process -FilePath $tauriDriver -ArgumentList @('--native-driver', $edge.driver, '--port', "$port") `
      -RedirectStandardOutput (Join-Path $dir 'tauri-driver-out.log') -RedirectStandardError (Join-Path $dir 'tauri-driver-err.log') -PassThru -WindowStyle Hidden
    $body = @{ capabilities = @{ alwaysMatch = @{ 'tauri:options' = @{ application = $bin } } } } | ConvertTo-Json -Depth 6
  } elseif ($how -eq 'edgedriver') {
    $port = 4446
    $driver = Start-Process -FilePath $edge.driver -ArgumentList @("--port=$port", '--verbose', "--log-path=$(Join-Path $dir 'msedgedriver.log')") `
      -RedirectStandardOutput (Join-Path $dir 'msedgedriver-out.log') -RedirectStandardError (Join-Path $dir 'msedgedriver-err.log') -PassThru -WindowStyle Hidden
    $body = @{ capabilities = @{ alwaysMatch = @{ browserName = 'webview2'; 'ms:edgeChromium' = $true; 'ms:edgeOptions' = @{ binary = $bin; args = @() } } } } | ConvertTo-Json -Depth 6
  } else {
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9333'
    $vela = Start-Process -FilePath $bin -RedirectStandardError (Join-Path $dir 'vela-err.log') -RedirectStandardOutput (Join-Path $dir 'vela-out.log') -PassThru
  }
  if ($driver) {
    Start-Sleep -Seconds 3
    $body | Out-File (Join-Path $dir 'request.json')
    try {
      $response = Invoke-WebRequest -Uri "http://127.0.0.1:$port/session" -Method Post -Body $body -ContentType 'application/json' -TimeoutSec 75
      $response.Content | Out-File (Join-Path $dir 'session.json')
      $session = ($response.Content | ConvertFrom-Json).value.sessionId
      $title = (Invoke-WebRequest -Uri "http://127.0.0.1:$port/session/$session/title" -TimeoutSec 20).Content
      Note "$name ($how, pinned=$pinned): a session after $([int]((Get-Date) - $started).TotalSeconds) s; title $title"
      Start-Sleep -Seconds 3
      Shot (Join-Path $dir 'desktop.png')
      Invoke-WebRequest -Uri "http://127.0.0.1:$port/session/$session" -Method Delete -TimeoutSec 30 | Out-Null
    } catch {
      Note "$name ($how, pinned=$pinned): no session after $([int]((Get-Date) - $started).TotalSeconds) s: $($_.Exception.Message)"
      Shot (Join-Path $dir 'desktop.png')
    }
  } else {
    $answer = $null
    for ($i = 0; $i -lt 40 -and -not $answer; $i++) {
      Start-Sleep -Seconds 1
      try { $answer = (Invoke-WebRequest -Uri 'http://127.0.0.1:9333/json/version' -TimeoutSec 3).Content } catch { }
      if ($vela.HasExited) { break }
    }
    Start-Sleep -Seconds 2
    Shot (Join-Path $dir 'desktop.png')
    $state = if ($vela.HasExited) { "exited with $($vela.ExitCode)" } else { 'running' }
    if ($answer) {
      $answer | Out-File (Join-Path $dir 'devtools-version.json')
      Note "$name (Vela alone, pinned=$pinned): DevTools answered; Vela $state"
    } else {
      Note "$name (Vela alone, pinned=$pinned): no DevTools answer in 40 s; Vela $state"
    }
    Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue
  }
  Stop-Job $sampler -ErrorAction SilentlyContinue
  Get-WinEvent -FilterHashtable @{ LogName = 'Application'; StartTime = $started; Level = 1, 2, 3 } -ErrorAction SilentlyContinue |
    Select-Object TimeCreated, ProviderName, Id, Message | Format-List | Out-File (Join-Path $dir 'events.txt')
  if ($driver) { Stop-Process -Id $driver.Id -Force -ErrorAction SilentlyContinue }
  StopOurs
}

Attempt 'A' 'tauri-driver' $true
Attempt 'B' 'tauri-driver' $false
Attempt 'C' 'edgedriver' $true
Attempt 'D' 'edgedriver' $false
Attempt 'E' 'alone' $true
Attempt 'F' 'alone' $false
Note 'done; read result.txt and each attempt''s folder'
exit 0
