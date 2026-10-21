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

# Manifest V3 shape. Chrome runs the background script as a service worker,
# Firefox runs the same script as an event page; both are Manifest V3.
$chromeManifest = Get-Content (Join-Path $chromeRoot "manifest.json") -Raw | ConvertFrom-Json
$firefoxManifest = Get-Content (Join-Path $firefoxRoot "manifest.json") -Raw | ConvertFrom-Json

foreach ($pair in @(
  @{ Name = "extension"; Manifest = $chromeManifest },
  @{ Name = "extension-firefox"; Manifest = $firefoxManifest }
)) {
  $manifest = $pair.Manifest
  $label = $pair.Name
  if ($manifest.manifest_version -ne 3) {
    $failures.Add("$label manifest_version is $($manifest.manifest_version), expected 3")
  }
  if ($null -eq $manifest.action) {
    $failures.Add("$label has no MV3 action (browser_action is MV2-only)")
  }
  if ($null -eq $manifest.host_permissions -or $manifest.host_permissions.Count -eq 0) {
    $failures.Add("$label has no host_permissions")
  }
  foreach ($permission in @($manifest.permissions)) {
    if ($permission -match "://") {
      $failures.Add("$label lists the host pattern $permission in permissions; it belongs in host_permissions")
    }
  }
  foreach ($entry in @($manifest.web_accessible_resources)) {
    if ($null -eq $entry.resources -or $null -eq $entry.matches) {
      $failures.Add("$label web_accessible_resources must use the MV3 object form (resources + matches)")
    }
  }
}

if ($null -eq $chromeManifest.background.service_worker) {
  $failures.Add("extension background must use the MV3 service_worker")
}
if ($null -ne $chromeManifest.background.scripts) {
  $failures.Add("extension background must not declare MV2 scripts")
}
if ($null -eq $firefoxManifest.background.scripts -or $firefoxManifest.background.scripts.Count -eq 0) {
  $failures.Add("extension-firefox background must declare the MV3 event page scripts")
}
if ($null -ne $firefoxManifest.background.service_worker) {
  $failures.Add("extension-firefox must not use service_worker; Firefox runs an event page")
}
if ($null -eq $firefoxManifest.browser_specific_settings) {
  $failures.Add("extension-firefox is missing browser_specific_settings")
}
if ($null -ne $firefoxManifest.key) {
  $failures.Add("extension-firefox must not carry the Chrome signing key")
}
if ($chromeManifest.version -ne $firefoxManifest.version) {
  $failures.Add("extension version $($chromeManifest.version) differs from extension-firefox $($firefoxManifest.version)")
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
