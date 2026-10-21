# Sign Windows binaries when a code-signing certificate is configured.
# The certificate is supplied as a base64-encoded PFX so no file is committed.
param(
  [Parameter(Mandatory = $true)]
  [string[]]$Path
)

$ErrorActionPreference = "Stop"

if (-not $env:WINDOWS_CERT_PFX_BASE64) {
  Write-Warning "WINDOWS_CERT_PFX_BASE64 is not set; skipping code signing."
  exit 0
}
if (-not $env:WINDOWS_CERT_PASSWORD) {
  throw "WINDOWS_CERT_PFX_BASE64 is set but WINDOWS_CERT_PASSWORD is missing."
}

$pfx = Join-Path ($(if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { $env:TEMP })) "vyomkunji-signing.pfx"
try {
  [System.IO.File]::WriteAllBytes($pfx, [Convert]::FromBase64String($env:WINDOWS_CERT_PFX_BASE64))

  $signtool = (Get-Command signtool.exe -ErrorAction SilentlyContinue).Source
  if (-not $signtool) {
    $kits = "${env:ProgramFiles(x86)}\Windows Kits\10\bin"
    if (Test-Path $kits) {
      $signtool = Get-ChildItem $kits -Recurse -Filter signtool.exe |
        Where-Object { $_.FullName -match "\\x64\\" } |
        Sort-Object FullName -Descending |
        Select-Object -First 1 -ExpandProperty FullName
    }
  }
  if (-not $signtool) { throw "signtool.exe not found on this runner." }

  foreach ($file in $Path) {
    if (-not (Test-Path $file)) { throw "Cannot sign missing file: $file" }
    & $signtool sign /fd SHA256 /f $pfx /p $env:WINDOWS_CERT_PASSWORD `
      /tr https://timestamp.digicert.com /td SHA256 $file
    if ($LASTEXITCODE -ne 0) { throw "signtool failed for $file" }
    Write-Host "Signed $file"
  }
} finally {
  if (Test-Path $pfx) { Remove-Item $pfx -Force }
}
