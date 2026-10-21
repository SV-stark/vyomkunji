# install-host.ps1 - register the native messaging host for Chrome and/or Firefox.
param(
  [string]$ExePath = "",
  [ValidateSet("Chrome", "Firefox", "Both")]
  [string]$Browser = "Both",
  [string]$ChromeExtensionId = "cnmiodibehhadaglhadkalhgjjeiblfp",
  [string]$FirefoxExtensionId = "vyomkunji-g2c@vyomkunji.local",
  [switch]$Unregister
)
$ErrorActionPreference = "Stop"

$HostNames = @("com.vyomkunji.host", "com.signer.host")

function Unregister-All {
  foreach ($hostName in $HostNames) {
    foreach ($root in @(
      "HKCU:\Software\Google\Chrome\NativeMessagingHosts",
      "HKCU:\Software\Mozilla\NativeMessagingHosts",
      "HKCU:\Software\Microsoft\Edge\NativeMessagingHosts"
    )) {
      $key = Join-Path $root $hostName
      if (Test-Path $key) {
        Remove-Item -Path $key -Recurse -Force -ErrorAction SilentlyContinue
        Write-Host "[unregister] removed $key"
      }
    }
    foreach ($suffix in @(".installed.json", ".firefox.installed.json")) {
      $file = Join-Path $PSScriptRoot "$hostName$suffix"
      if (Test-Path $file) {
        Remove-Item -Path $file -Force -ErrorAction SilentlyContinue
      }
    }
  }
}

if ($Unregister) {
  Unregister-All
  exit 0
}

if (-not $ExePath) {
  $candidates = @(
    (Join-Path $PSScriptRoot "..\target\release\vyomkunji-host.exe"),
    (Join-Path $PSScriptRoot "target\release\vyomkunji-host.exe"),
    (Join-Path $PSScriptRoot "..\target\release\signer-host.exe"),
    (Join-Path $PSScriptRoot "target\release\signer-host.exe")
  )
  if ($env:CARGO_TARGET_DIR) {
    $candidates = @(
      (Join-Path $env:CARGO_TARGET_DIR "release\vyomkunji-host.exe"),
      (Join-Path $env:CARGO_TARGET_DIR "release\signer-host.exe")
    ) + $candidates
  }
  foreach ($c in $candidates) {
    if (Test-Path $c) {
      $ExePath = (Resolve-Path $c).Path
      break
    }
  }
  if (-not $ExePath) {
    $ExePath = (Join-Path $PSScriptRoot "..\target\release\vyomkunji-host.exe")
  }
}

if (-not $ExePath -or -not (Test-Path $ExePath)) {
  throw "Native host executable not found: $ExePath"
}

function Write-Utf8NoBom([string]$Path, [string]$Text) {
  # Chrome and Firefox expect a UTF-8 manifest without a BOM. Windows
  # PowerShell's Set-Content -Encoding UTF8 would add one.
  [System.IO.File]::WriteAllText($Path, $Text, (New-Object System.Text.UTF8Encoding($false)))
}

function Register-Chrome {
  foreach ($hostName in $HostNames) {
    $src = Join-Path $PSScriptRoot "$hostName.json"
    if (-not (Test-Path $src)) {
      Write-Host "[Chrome] skipping $hostName (manifest $src not found)"
      continue
    }
    $manifest = Get-Content $src -Raw
    $manifest = $manifest -replace "REPLACE_WITH_ABSOLUTE_PATH_TO_signer-host.exe", ($ExePath -replace "\\","\\\\")
    $manifest = $manifest -replace "REPLACE_WITH_EXTENSION_ID", $ChromeExtensionId
    $out = Join-Path $PSScriptRoot "$hostName.installed.json"
    Write-Utf8NoBom $out $manifest
    $reg = "HKCU:\Software\Google\Chrome\NativeMessagingHosts\$hostName"
    New-Item -Path $reg -Force | Out-Null
    Set-ItemProperty -Path $reg -Name "(Default)" -Value $out
    Write-Host "[Chrome] registered $hostName -> $out"
  }
}

function Register-Firefox {
  foreach ($hostName in $HostNames) {
    $src = Join-Path $PSScriptRoot "$hostName.firefox.json"
    if (-not (Test-Path $src)) {
      Write-Host "[Firefox] skipping $hostName (manifest $src not found)"
      continue
    }
    $manifest = Get-Content $src -Raw
    $manifest = $manifest -replace "REPLACE_WITH_ABSOLUTE_PATH_TO_signer-host.exe", ($ExePath -replace "\\","\\\\")
    $manifest = $manifest -replace "REPLACE_WITH_FIREFOX_EXTENSION_ID", $FirefoxExtensionId
    $out = Join-Path $PSScriptRoot "$hostName.firefox.installed.json"
    Write-Utf8NoBom $out $manifest
    $reg = "HKCU:\Software\Mozilla\NativeMessagingHosts\$hostName"
    New-Item -Path $reg -Force | Out-Null
    Set-ItemProperty -Path $reg -Name "(Default)" -Value $out
    Write-Host "[Firefox] registered $hostName -> $out"
  }
}

if ($Browser -in @("Chrome","Both")) { Register-Chrome }
if ($Browser -in @("Firefox","Both")) { Register-Firefox }
