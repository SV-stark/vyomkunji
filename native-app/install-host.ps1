# install-host.ps1 - register the native messaging host for Chrome and/or Firefox.
param(
  [string]$ExePath = "H:\target\release\signer-host.exe",
  [ValidateSet("Chrome", "Firefox", "Both")]
  [string]$Browser = "Both",
  [string]$ChromeExtensionId = "REPLACE_ME",
  [string]$FirefoxExtensionUuid = "REPLACE_ME"
)
$ErrorActionPreference = "Stop"

function Register-Chrome {
  $manifest = Get-Content (Join-Path $PSScriptRoot "com.signer.host.json") -Raw
  $manifest = $manifest -replace "REPLACE_WITH_ABSOLUTE_PATH_TO_signer-host.exe", ($ExePath -replace "\\","\\\\")
  $manifest = $manifest -replace "REPLACE_WITH_EXTENSION_ID", $ChromeExtensionId
  $out = Join-Path $PSScriptRoot "com.signer.host.installed.json"
  $manifest | Set-Content $out
  $reg = "HKCU:\Software\Google\Chrome\NativeMessagingHosts\com.signer.host"
  New-Item -Path $reg -Force | Out-Null
  Set-ItemProperty -Path $reg -Name "(Default)" -Value $out
  Write-Host "[Chrome] registered -> $out"
}

function Register-Firefox {
  $manifest = Get-Content (Join-Path $PSScriptRoot "com.signer.host.firefox.json") -Raw
  $manifest = $manifest -replace "REPLACE_WITH_ABSOLUTE_PATH_TO_signer-host.exe", ($ExePath -replace "\\","\\\\")
  $manifest = $manifest -replace "REPLACE_WITH_FIREFOX_EXTENSION_UUID", $FirefoxExtensionUuid
  $out = Join-Path $PSScriptRoot "com.signer.host.firefox.installed.json"
  $manifest | Set-Content $out
  $reg = "HKCU:\Software\Mozilla\NativeMessagingHosts\com.signer.host"
  New-Item -Path $reg -Force | Out-Null
  Set-ItemProperty -Path $reg -Name "(Default)" -Value $out
  Write-Host "[Firefox] registered -> $out"
}

if ($Browser -in @("Chrome","Both")) { Register-Chrome }
if ($Browser -in @("Firefox","Both")) { Register-Firefox }
