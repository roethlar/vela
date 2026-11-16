#!/bin/bash
# Can this runner show a window and take a picture of the screen?
set -u
out="${PROBE_OUT:?}"
case "$(uname -s)" in
  Darwin)
    sw_vers > "$out/os.txt" 2>&1
    system_profiler SPDisplaysDataType > "$out/displays.txt" 2>&1
    open -a TextEdit
    sleep 6
    if screencapture -x "$out/screen.png" 2> "$out/screencapture.err"; then echo "screencapture: ok"; else echo "screencapture: failed"; fi
    ls -l "$out"
    ;;
  MINGW*|MSYS*|CYGWIN*)
    cat > "$out/probe.ps1" <<'PS'
$ErrorActionPreference = 'Stop'
$out = $env:PROBE_OUT
[System.Environment]::OSVersion | Out-File "$out/os.txt"
"interactive: $([Environment]::UserInteractive); session: $((Get-Process -Id $PID).SessionId)" | Out-File -Append "$out/os.txt"
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
$job = Start-Job -ScriptBlock {
  Add-Type -AssemblyName System.Windows.Forms
  $f = New-Object System.Windows.Forms.Form
  $f.Text = 'Vela probe'; $f.Width = 640; $f.Height = 360; $f.BackColor = 'DarkOrange'; $f.TopMost = $true
  $l = New-Object System.Windows.Forms.Label; $l.Text = 'a window drawn on the runner'; $l.AutoSize = $true; $l.Font = 'Segoe UI,20'
  $f.Controls.Add($l); $f.Add_Shown({ $f.Activate() })
  $t = New-Object System.Windows.Forms.Timer; $t.Interval = 12000; $t.Add_Tick({ $f.Close() }); $t.Start()
  [void]$f.ShowDialog()
}
Start-Sleep -Seconds 5
$b = [System.Windows.Forms.SystemInformation]::VirtualScreen
"screen: $($b.Width)x$($b.Height)" | Out-File -Append "$out/os.txt"
$bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($b.Left, $b.Top, 0, 0, $bmp.Size)
$bmp.Save("$out/screen.png", [System.Drawing.Imaging.ImageFormat]::Png)
"screenshot: ok" | Out-File -Append "$out/os.txt"
Wait-Job $job -Timeout 20 | Out-Null
PS
    pwsh -NoProfile -File "$out/probe.ps1" && echo "probe: ok" || echo "probe: failed"
    cat "$out/os.txt"
    ;;
  *) echo "not a macOS or Windows runner: $(uname -s)"; exit 2 ;;
esac
