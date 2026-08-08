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

[CmdletBinding()]
param(
  # Tauri substitutes its "%1" placeholder with the real path before invoking.
  [Parameter(Mandatory = $true, Position = 0)]
  [string] $Path
)

$ErrorActionPreference = 'Stop'

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
