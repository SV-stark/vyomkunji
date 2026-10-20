# Keep the Firefox extension's shared source in sync with the Chrome extension.
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
$src = Join-Path $root "extension\src"
$dst = Join-Path $root "extension-firefox\src"
New-Item -ItemType Directory -Force -Path $dst | Out-Null
Copy-Item -Path (Join-Path $src "*") -Destination $dst -Recurse -Force
Copy-Item -Path (Join-Path $root "extension\icons\*") -Destination (Join-Path $root "extension-firefox\icons") -Recurse -Force
Write-Host "Synced extension/src -> extension-firefox/src"
