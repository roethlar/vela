# WebView2's installed runtime, not the unrelated Edge browser's version.
# https://learn.microsoft.com/microsoft-edge/webview2/concepts/distribution
# https://learn.microsoft.com/microsoft-edge/webview2/how-to/webdriver
$ErrorActionPreference = 'Stop'
if (-not $IsWindows -or -not [Environment]::Is64BitProcess) { throw 'Requires x64 Windows PowerShell 7.' }
$runtimeId = '{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
$installs = @(
    @{ key = "HKCU:\Software\Microsoft\EdgeUpdate\Clients\$runtimeId"; base = [Environment]::GetFolderPath('LocalApplicationData') },
    @{ key = "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\$runtimeId"; base = ${env:ProgramFiles(x86)} }
)
$runtimes = @(foreach ($install in $installs) {
    $version = (Get-ItemProperty -LiteralPath $install.key -Name pv -ErrorAction SilentlyContinue).pv
    if ($version -match '^\d+\.\d+\.\d+\.\d+$' -and $version -ne '0.0.0.0') {
        $folder = Join-Path $install.base "Microsoft/EdgeWebView/Application/$version"
        $runtime = Join-Path $folder 'msedgewebview2.exe'
        if (Test-Path -LiteralPath $runtime) {
            if ((Get-Item -LiteralPath $runtime).VersionInfo.FileVersion -ne $version) { throw "WebView2 registry/file version mismatch at $runtime" }
            @{ version = $version; folder = $folder }
        }
    }
})
if ($runtimes.Count -eq 0) { throw 'No installed Evergreen WebView2 runtime found in its registered location.' }
$selected = $runtimes | Sort-Object { [version]$_.version } -Descending | Select-Object -First 1
$version = $selected.version
$destination = Join-Path $PSScriptRoot "vendor/msedgedriver/$version"
$driver = Join-Path $destination 'msedgedriver.exe'
if (-not (Test-Path -LiteralPath $driver)) {
    New-Item -ItemType Directory -Force -Path $destination | Out-Null
    $archive = Join-Path $destination 'edgedriver_win64.zip'
    # Exact runtime version, official HTTPS distribution. Do not silently
    # substitute the installed browser's driver or the latest driver.
    $url = "https://msedgedriver.microsoft.com/$version/edgedriver_win64.zip"
    Invoke-WebRequest -Uri $url -OutFile $archive
    Expand-Archive -LiteralPath $archive -DestinationPath $destination -Force
}
# Microsoft distributes a signed executable. Verify its Authenticode chain
# and publisher before ever executing it, including a cached download. The
# SHA256 below is an artifact identity, not an independent integrity proof.
$signature = Get-AuthenticodeSignature -LiteralPath $driver
if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch '(^|,\s*)O=Microsoft Corporation(,|$)') {
    throw "Microsoft Edge WebDriver signature rejected: $($signature.Status)"
}
$reported = & $driver --version
if ($LASTEXITCODE -ne 0 -or $reported -notmatch "^MSEdgeDriver $([regex]::Escape($version))(\s|$)") {
    throw "Edge WebDriver does not match WebView2 $version : $reported"
}
$record = @{
    driver = $driver; version = $version; runtimeDir = $selected.folder
    sha256 = (Get-FileHash -LiteralPath $driver -Algorithm SHA256).Hash
    signer = $signature.SignerCertificate.Subject; signature = [string]$signature.Status
    url = "https://msedgedriver.microsoft.com/$version/edgedriver_win64.zip"
}
$record | ConvertTo-Json -Compress
