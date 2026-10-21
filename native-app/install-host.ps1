# install-host.ps1 - register the native messaging host for Chrome and/or Firefox.
param(
  [string]$ExePath = "",
  [ValidateSet("Chrome", "Firefox", "Both")]
  [string]$Browser = "Both",
  [string]$ChromeExtensionId = "cnmiodibehhadaglhadkalhgjjeiblfp",
  [string]$FirefoxExtensionId = "vyomkunji-g2c@vyomkunji.local"
)
$ErrorActionPreference = "Stop"

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

function Register-Chrome {
  foreach ($hostName in @("com.vyomkunji.host", "com.signer.host")) {
    $src = if (Test-Path (Join-Path $PSScriptRoot "$hostName.json")) {
      Join-Path $PSScriptRoot "$hostName.json"
    } else {
      Join-Path $PSScriptRoot "com.signer.host.json"
    }
    $manifest = Get-Content $src -Raw
    $manifest = $manifest -replace "REPLACE_WITH_ABSOLUTE_PATH_TO_signer-host.exe", ($ExePath -replace "\\","\\\\")
    $manifest = $manifest -replace "REPLACE_WITH_EXTENSION_ID", $ChromeExtensionId
    $out = Join-Path $PSScriptRoot "$hostName.installed.json"
    $manifest | Set-Content $out
    $reg = "HKCU:\Software\Google\Chrome\NativeMessagingHosts\$hostName"
    New-Item -Path $reg -Force | Out-Null
    Set-ItemProperty -Path $reg -Name "(Default)" -Value $out
    Write-Host "[Chrome] registered $hostName -> $out"
  }
}

function Register-Firefox {
  foreach ($hostName in @("com.vyomkunji.host", "com.signer.host")) {
    $src = if (Test-Path (Join-Path $PSScriptRoot "$hostName.firefox.json")) {
      Join-Path $PSScriptRoot "$hostName.firefox.json"
    } else {
      Join-Path $PSScriptRoot "com.signer.host.firefox.json"
    }
    $manifest = Get-Content $src -Raw
    $manifest = $manifest -replace "REPLACE_WITH_ABSOLUTE_PATH_TO_signer-host.exe", ($ExePath -replace "\\","\\\\")
    $manifest = $manifest -replace "REPLACE_WITH_FIREFOX_EXTENSION_ID", $FirefoxExtensionId
    $out = Join-Path $PSScriptRoot "$hostName.firefox.installed.json"
    $manifest | Set-Content $out
    $reg = "HKCU:\Software\Mozilla\NativeMessagingHosts\$hostName"
    New-Item -Path $reg -Force | Out-Null
    Set-ItemProperty -Path $reg -Name "(Default)" -Value $out
    Write-Host "[Firefox] registered $hostName -> $out"
  }
}

if ($Browser -in @("Chrome","Both")) { Register-Chrome }
if ($Browser -in @("Firefox","Both")) { Register-Firefox }
