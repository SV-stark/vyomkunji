# bump-version.ps1 - update every shipped version string from one value.
# The Cargo package version is the source of truth; this script writes it to
# the extension manifests, the Inno Setup script, and the README asset names.
param(
  [Parameter(Mandatory = $true)]
  [string]$Version
)

$ErrorActionPreference = "Stop"
if ($Version -notmatch '^\d+\.\d+\.\d+([-.+][0-9A-Za-z.-]+)?$') {
  throw "Version must look like 1.2.3 (got '$Version')"
}

$root = Split-Path $PSScriptRoot -Parent
$utf8 = New-Object System.Text.UTF8Encoding($false)

function Write-Text([string]$Path, [string]$Text) {
  [System.IO.File]::WriteAllText($Path, $Text, $utf8)
}

# --- Cargo.toml (first `version =` line, i.e. the [package] version) ---------
$cargoPath = Join-Path $root "native-app\Cargo.toml"
$cargo = Get-Content $cargoPath -Raw
$cargo = [regex]::Replace($cargo, '(?m)^version = "[^"]+"', "version = `"$Version`"", 1)
Write-Text $cargoPath $cargo

# --- extension manifests ----------------------------------------------------
foreach ($rel in @("extension\manifest.json", "extension-firefox\manifest.json")) {
  $path = Join-Path $root $rel
  $json = Get-Content $path -Raw
  $json = [regex]::Replace($json, '"version":\s*"[^"]+"', "`"version`": `"$Version`"", 1)
  Write-Text $path $json
}

# --- Inno Setup -------------------------------------------------------------
$issPath = Join-Path $root "installer\vyomkunji.iss"
$iss = Get-Content $issPath -Raw
$iss = [regex]::Replace($iss, '#define MyAppVersion "[^"]+"', "#define MyAppVersion `"$Version`"", 1)
Write-Text $issPath $iss

# --- README asset names -----------------------------------------------------
$readmePath = Join-Path $root "README.md"
$readme = Get-Content $readmePath -Raw
$readme = [regex]::Replace($readme, 'vyomkunji-v\d+\.\d+\.\d+', "vyomkunji-v$Version")
$readme = [regex]::Replace($readme, 'vyomkunji-chrome-v\d+\.\d+\.\d+', "vyomkunji-chrome-v$Version")
$readme = [regex]::Replace($readme, 'vyomkunji-firefox-v\d+\.\d+\.\d+', "vyomkunji-firefox-v$Version")
Write-Text $readmePath $readme

Write-Host "Updated version to $Version"
Write-Host "Next: cargo check (refreshes Cargo.lock), update CHANGELOG.md, then tag v$Version."
