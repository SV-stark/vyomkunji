# Validate the two extension trees and their manifests.
# Run from the repository root: powershell -File tools/check-extensions.ps1
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
$failures = New-Object System.Collections.Generic.List[string]

$chromeRoot = Join-Path $root "extension"
$firefoxRoot = Join-Path $root "extension-firefox"

foreach ($manifestPath in @(
  (Join-Path $chromeRoot "manifest.json"),
  (Join-Path $firefoxRoot "manifest.json")
)) {
  try {
    $null = Get-Content $manifestPath -Raw | ConvertFrom-Json
  } catch {
    $failures.Add("Invalid JSON in $manifestPath`: $($_.Exception.Message)")
  }
}

# Every file under the shared src tree must be byte-identical. The manifests
# and platform-specific packaging intentionally differ.
$chromeSrc = Join-Path $chromeRoot "src"
$firefoxSrc = Join-Path $firefoxRoot "src"
$chromeFiles = Get-ChildItem $chromeSrc -Recurse -File | ForEach-Object {
  $_.FullName.Substring($chromeSrc.Length + 1)
}
foreach ($relative in $chromeFiles) {
  $a = Join-Path $chromeSrc $relative
  $b = Join-Path $firefoxSrc $relative
  if (-not (Test-Path $b)) {
    $failures.Add("extension-firefox/src/$relative is missing")
    continue
  }
  $hashA = (Get-FileHash $a -Algorithm SHA256).Hash
  $hashB = (Get-FileHash $b -Algorithm SHA256).Hash
  if ($hashA -ne $hashB) {
    $failures.Add("extension and extension-firefox differ: src/$relative")
  }
}
$firefoxFiles = Get-ChildItem $firefoxSrc -Recurse -File | ForEach-Object {
  $_.FullName.Substring($firefoxSrc.Length + 1)
}
foreach ($relative in $firefoxFiles) {
  if (-not (Test-Path (Join-Path $chromeSrc $relative))) {
    $failures.Add("extension/src/$relative is missing")
  }
}

# Syntax-check every browser script. `node --check` is available in CI and on
# developer machines with Node installed; skip gracefully if Node is absent.
$node = Get-Command node -ErrorAction SilentlyContinue
if ($node) {
  foreach ($file in Get-ChildItem $chromeSrc -Recurse -File -Include *.js) {
    & node --check $file.FullName
    if ($LASTEXITCODE -ne 0) { $failures.Add("JavaScript syntax error: $($file.FullName)") }
  }
}

if ($failures.Count -gt 0) {
  Write-Host "Extension check failed:" -ForegroundColor Red
  foreach ($failure in $failures) { Write-Host "  - $failure" -ForegroundColor Red }
  exit 1
}

Write-Host "Extension trees are in sync and manifests parse." -ForegroundColor Green
