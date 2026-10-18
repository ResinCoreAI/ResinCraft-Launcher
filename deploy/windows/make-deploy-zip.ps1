<#
.SYNOPSIS
  Builds the website and the relay and packs everything for Windows Server into one zip.

.DESCRIPTION
  Run on the development PC from the ResinCraft repo:
      .\deploy\windows\make-deploy-zip.ps1 -SiteUrl https://resincraft.resincore.tech

  Output: dist\ResinCraft-Deploy-<date>.zip containing
      site\        the website (static files + IIS web.config)
      relay\       resincraft-relay.exe, relay.toml.example, README
      scripts\     install-site.ps1, install-relay.ps1, uninstall-relay.ps1
      DEPLOY-WINDOWS.md
#>
[CmdletBinding()]
param(
  [string]$SiteUrl = 'https://resincraft.resincore.tech',
  [string]$OutDir
)

$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
if (-not $OutDir) { $OutDir = Join-Path $repo 'dist' }
$stamp = Get-Date -Format 'yyyyMMdd'
$stage = Join-Path ([IO.Path]::GetTempPath()) "ResinCraft-Deploy-$stamp"
$zip = Join-Path $OutDir "ResinCraft-Deploy-$stamp.zip"

function Step($text) { Write-Host "==> $text" -ForegroundColor Cyan }

# npm and cargo print progress on stderr. Windows PowerShell 5.1 would treat that as an error
# (when output is redirected), so run them with errors relaxed and judge by the exit code.
function Invoke-Native([string]$What, [scriptblock]$Command) {
  $old = $ErrorActionPreference
  $ErrorActionPreference = 'Continue'
  try { & $Command 2>&1 | ForEach-Object { "$_" } | Out-Host } finally { $ErrorActionPreference = $old }
  if ($LASTEXITCODE) { throw "$What failed (exit code $LASTEXITCODE)" }
}

if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Force -Path $stage, (Join-Path $stage 'scripts'), $OutDir | Out-Null

# ---- Website ----
Step "Building the website for $SiteUrl"
Push-Location (Join-Path $repo 'site')
try {
  if (-not (Test-Path 'node_modules')) { Invoke-Native 'npm ci' { npm ci --no-audit --no-fund } }
  $env:SITE_URL = $SiteUrl
  $env:NEXT_TELEMETRY_DISABLED = '1'
  Invoke-Native 'The website build' { npm run build }
} finally {
  Remove-Item Env:SITE_URL -ErrorAction SilentlyContinue
  Pop-Location
}
Copy-Item (Join-Path $repo 'site\out') (Join-Path $stage 'site') -Recurse
Copy-Item (Join-Path $PSScriptRoot 'install-site.ps1') (Join-Path $stage 'scripts')

# ---- Relay ----
$relayDeploy = Join-Path $repo 'relay\deploy\windows'
$relayOk = $false
if (Test-Path (Join-Path $relayDeploy 'install-relay.ps1')) {
  Step 'Building resincraft-relay.exe (release)'
  $manifest = Join-Path $repo 'relay\Cargo.toml'
  Invoke-Native 'The relay build' { cargo build --release --manifest-path $manifest }
  $exe = Join-Path $repo 'relay\target\release\resincraft-relay.exe'
  $relayStage = Join-Path $stage 'relay'
  New-Item -ItemType Directory -Force -Path $relayStage | Out-Null
  Copy-Item $exe $relayStage
  Get-ChildItem $relayDeploy -File | Where-Object { $_.Extension -ne '.ps1' } | Copy-Item -Destination $relayStage
  Get-ChildItem $relayDeploy -Filter '*.ps1' | Copy-Item -Destination (Join-Path $stage 'scripts')
  $relayOk = $true
} else {
  Write-Warning "relay\deploy\windows\install-relay.ps1 isn't there yet, so the zip has the website only."
}

# ---- Guide ----
Copy-Item (Join-Path $PSScriptRoot 'DEPLOY-WINDOWS.md') $stage

Step "Writing $zip"
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip -CompressionLevel Optimal
Remove-Item $stage -Recurse -Force

$size = '{0:N1} MB' -f ((Get-Item $zip).Length / 1MB)
Write-Host ''
Write-Host "Done: $zip ($size)" -ForegroundColor Green
if (-not $relayOk) { Write-Host 'Relay not included yet.' -ForegroundColor Yellow }
