# Signs one Windows binary or installer with Azure Trusted Signing.
#
# Tauri's `bundle > windows > signCommand` hook calls this once per produced
# binary DURING bundling, which is the only workable point in this repo: the
# release workflow's tauri-action step builds AND uploads to the draft release
# in a single action, so a signing pass afterwards would leave unsigned assets
# published. The hook is injected only by CI (see .github/workflows/release.yml)
# so local builds keep bundling unsigned.
#
# Credentials are never passed in. The TrustedSigning module reads
# AZURE_TENANT_ID, AZURE_CLIENT_ID and AZURE_CLIENT_SECRET itself through
# Azure's EnvironmentCredential; only the endpoint, account and profile are read
# here, and those ride environment variables too so no Azure resource is named
# in the tree.
#
# When signing fails, Tauri reports only "failed to run pwsh" and shows nothing
# this script said. So the reason is also left in a file under RUNNER_TEMP,
# which the workflow prints when the build step has failed. (What the signing
# service says can name the endpoint, the account or the profile; they are
# repository secrets, and GitHub masks secrets in what a step prints.)
#
# The signing itself runs in a child process of this script (`-Sign`). The
# module starts SignTool so that it writes straight to the console, past
# PowerShell's streams, and what SignTool says is the reason when it fails;
# only a pipe around the whole process catches it.

[CmdletBinding()]
param(
  # Tauri substitutes its "%1" placeholder with the real path before invoking.
  [Parameter(Mandatory = $true, Position = 0)]
  [string] $Path,

  # Given only by this script itself, to the child process that signs.
  [switch] $Sign
)

$ErrorActionPreference = 'Stop'

if ($Sign) {
  if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
    throw "Nothing to sign at $Path."
  }

  # A missing repository secret arrives as an empty-but-defined environment
  # variable, so every check here has to be for a non-empty value.
  foreach ($name in 'AZURE_SIGNING_ENDPOINT', 'AZURE_SIGNING_ACCOUNT', 'AZURE_CERT_PROFILE') {
    if ([string]::IsNullOrWhiteSpace([Environment]::GetEnvironmentVariable($name))) {
      throw "$name is unset or empty, so $Path cannot be signed."
    }
  }

  Import-Module TrustedSigning
  Write-Output "TrustedSigning module: $((Get-Module TrustedSigning | Select-Object -First 1).Version)"

  Invoke-TrustedSigning `
    -Endpoint $env:AZURE_SIGNING_ENDPOINT `
    -CodeSigningAccountName $env:AZURE_SIGNING_ACCOUNT `
    -CertificateProfileName $env:AZURE_CERT_PROFILE `
    -TimestampRfc3161 'http://timestamp.acs.microsoft.com' `
    -TimestampDigest 'SHA256' `
    -FileDigest 'SHA256' `
    -Files $Path

  # A signing tool that quietly does nothing still exits 0, so prove the signature
  # landed on this file before the bundler moves on to the next one.
  $signature = Get-AuthenticodeSignature -LiteralPath $Path
  if ($signature.Status -ne 'Valid') {
    throw "Authenticode status for $Path is '$($signature.Status)', expected 'Valid'."
  }

  Write-Output "Signed $Path ($($signature.SignerCertificate.Subject))"
  exit 0
}

# Everything the child process and the programs it starts write, kept line by
# line as it comes.
$said = [System.Collections.Generic.List[string]]::new()
$pwsh = (Get-Process -Id $PID).Path
# What the child writes to its standard error is output here, not an error of
# this script: only the child's exit code says whether signing worked.
$ErrorActionPreference = 'Continue'
& $pwsh -NoProfile -NonInteractive -File $PSCommandPath $Path -Sign 2>&1 |
  ForEach-Object { $said.Add("$_"); Write-Output "$_" }
$code = $LASTEXITCODE

if ($code -ne 0) {
  $reason = "Signing $(Split-Path -Leaf $Path) failed (the signing process ended with exit code $code)."
  $lines = @($reason)
  $lines += $said | Select-Object -Last 120 | ForEach-Object { "  $_" }
  if (-not [string]::IsNullOrWhiteSpace($env:RUNNER_TEMP)) {
    Add-Content -LiteralPath (Join-Path $env:RUNNER_TEMP 'vela-signing-failure.txt') -Value $lines
  }
  [Console]::Error.WriteLine($reason)
  exit 1
}
