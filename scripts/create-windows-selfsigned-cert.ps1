# Optional local helper — CI can generate a self-signed cert from the
# WINDOWS_CODESIGN_PASSWORD secret instead (see .github/workflows/release.yml).
# SmartScreen will still warn; this only exercises Authenticode signing.
#
# Usage (PowerShell):
#   .\scripts\create-windows-selfsigned-cert.ps1
#   .\scripts\create-windows-selfsigned-cert.ps1 -Password 'YourPass' -OutDir .\.local-certs

param(
    [string]$Subject = "CN=PromptLab Self-Signed, O=YangYang Research, C=US",
    [string]$Password = "",
    [string]$OutDir = "",
    [int]$Years = 2
)

$ErrorActionPreference = "Stop"

if (-not $OutDir) {
    $OutDir = Join-Path $PSScriptRoot "..\.local-certs" | Resolve-Path -ErrorAction SilentlyContinue
    if (-not $OutDir) {
        $OutDir = Join-Path (Split-Path $PSScriptRoot -Parent) ".local-certs"
    }
}
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

if (-not $Password) {
    $Password = -join ((48..57) + (65..90) + (97..122) | Get-Random -Count 24 | ForEach-Object { [char]$_ })
}

$pfxPath = Join-Path $OutDir "promptlab-codesign-selfsigned.pfx"
$b64Path = Join-Path $OutDir "promptlab-codesign-selfsigned.b64.txt"
$passPath = Join-Path $OutDir "promptlab-codesign-selfsigned.password.txt"

Write-Host "Creating self-signed CodeSigningCert..."
$cert = New-SelfSignedCertificate `
    -Type CodeSigningCert `
    -Subject $Subject `
    -CertStoreLocation "Cert:\CurrentUser\My" `
    -KeyExportPolicy Exportable `
    -KeySpec Signature `
    -KeyLength 2048 `
    -HashAlgorithm SHA256 `
    -NotAfter (Get-Date).AddYears($Years)

$secure = ConvertTo-SecureString -String $Password -Force -AsPlainText
Export-PfxCertificate -Cert $cert -FilePath $pfxPath -Password $secure | Out-Null

$b64 = [Convert]::ToBase64String([IO.File]::ReadAllBytes($pfxPath))
Set-Content -Path $b64Path -Value $b64 -Encoding ascii
Set-Content -Path $passPath -Value $Password -Encoding ascii

Write-Host ""
Write-Host "Created:"
Write-Host "  PFX:      $pfxPath"
Write-Host "  Base64:   $b64Path"
Write-Host "  Password: $passPath"
Write-Host ""
Write-Host "Add GitHub Actions secrets:"
Write-Host "  WINDOWS_CERTIFICATE          <- contents of .b64.txt"
Write-Host "  WINDOWS_CERTIFICATE_PASSWORD <- contents of .password.txt"
Write-Host ""
Write-Host "Do NOT commit .local-certs/ or the .pfx."
Write-Host "Thumbprint: $($cert.Thumbprint)"
