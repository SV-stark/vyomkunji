# check-version.ps1 - fail when shipped version strings drift apart.
# Also verifies that a tag build's tag matches the Cargo package version.
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
$failures = New-Object System.Collections.Generic.List[string]

function Read-CargoVersion {
  $path = Join-Path $root "native-app\Cargo.toml"
  $match = [regex]::Match((Get-Content $path -Raw), '(?m)^version = "([^"]+)"')
  if (-not $match.Success) { throw "Could not read package version from $path" }
  return $match.Groups[1].Value
}

$expected = Read-CargoVersion
Write-Host "Cargo package version: $expected"

foreach ($rel in @("extension\manifest.json", "extension-firefox\manifest.json")) {
  $path = Join-Path $root $rel
  $match = [regex]::Match((Get-Content $path -Raw), '"version":\s*"([^"]+)"')
  if (-not $match.Success) {
    $failures.Add("$rel has no version field")
  } elseif ($match.Groups[1].Value -ne $expected) {
    $failures.Add("$rel is $($match.Groups[1].Value), expected $expected")
  }
}

$issPath = Join-Path $root "installer\vyomkunji.iss"
$match = [regex]::Match((Get-Content $issPath -Raw), '#define MyAppVersion "([^"]+)"')
if (-not $match.Success) {
  $failures.Add("installer\vyomkunji.iss has no MyAppVersion")
} elseif ($match.Groups[1].Value -ne $expected) {
  $failures.Add("installer\vyomkunji.iss is $($match.Groups[1].Value), expected $expected")
}

# Tag builds must not publish a differently-numbered binary.
if ($env:GITHUB_REF_TYPE -eq "tag" -and $env:GITHUB_REF_NAME) {
  $tagVersion = $env:GITHUB_REF_NAME -replace '^v', ''
  if ($tagVersion -ne $expected) {
    $failures.Add("tag $env:GITHUB_REF_NAME does not match package version $expected")
  }
}

if ($failures.Count -gt 0) {
  Write-Host "Version check failed:" -ForegroundColor Red
  foreach ($failure in $failures) { Write-Host "  - $failure" -ForegroundColor Red }
  exit 1
}

Write-Host "Version check passed." -ForegroundColor Green
