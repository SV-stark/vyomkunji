# VyomKunji one-click local installer.
# Registers the native messaging host for Chrome and/or Firefox using the
# bundled manifests. Run from the extracted release folder:
#   .\install.ps1 -Browser Both
param(
    [ValidateSet("Chrome", "Firefox", "Both")]
    [string]$Browser = "Both"
)

$ErrorActionPreference = "Stop"
$root = $PSScriptRoot
$native = Join-Path $root "native-app"
if (-not (Test-Path (Join-Path $native "install-host.ps1"))) {
    # Allow running from a flat release layout.
    $native = $root
}

$exe = Join-Path $native "vyomkunji-host.exe"
if (-not (Test-Path $exe)) {
    throw "vyomkunji-host.exe not found next to this script. Extract the full release archive first."
}

Write-Host "Registering native host from $exe"
& (Join-Path $native "install-host.ps1") -ExePath $exe -Browser $Browser

Write-Host ""
Write-Host "VyomKunji native host registered."
Write-Host ""
Write-Host "Next steps:"
Write-Host "  1. (Chrome/Edge) Open chrome://extensions, enable Developer mode, 'Load unpacked',"
Write-Host "     and select the 'extension' folder from this archive."
Write-Host "     The extension ID is fixed (cnmiodibehhadaglhadkalhgjjeiblfp) thanks to the"
Write-Host "     bundled manifest key, so no manual ID passing is required."
Write-Host "  2. (Firefox) Open about:debugging#/runtime/this-firefox and load"
Write-Host "     extension-firefox/manifest.json as a temporary add-on."
Write-Host "  3. Plug in your DSC token and run vyomkunji.exe."
