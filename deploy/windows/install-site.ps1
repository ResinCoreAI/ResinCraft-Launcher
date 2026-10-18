<#
.SYNOPSIS
  Installs (or updates) the ResinCraft website on Windows Server with IIS.

.DESCRIPTION
  Run from the unzipped ResinCraft-Deploy folder, in PowerShell as Administrator:
      .\scripts\install-site.ps1 -Domain resincraft.resincore.tech

  - Turns on IIS with only what a static site needs.
  - Copies the website to C:\ResinCraft\site (mirrors it, so old files are removed on update).
  - Creates the IIS site "ResinCraft" on port 80 for -Domain (and www.<Domain> with -WithWww).
  Safe to run again: it updates the files and leaves your IIS bindings and certificates alone.
  HTTPS comes next with win-acme; see DEPLOY-WINDOWS.md.
#>
[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)][string]$Domain,
  [string]$Path = 'C:\ResinCraft\site',
  [string]$SiteName = 'ResinCraft',
  [switch]$WithWww
)

$ErrorActionPreference = 'Stop'

$principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
  throw 'Run this script in PowerShell as Administrator.'
}

$source = Join-Path (Split-Path $PSScriptRoot -Parent) 'site'
if (-not (Test-Path (Join-Path $source 'index.html'))) {
  throw "Website files not found in $source. Run this from the unzipped ResinCraft-Deploy folder."
}

Write-Host '1/4  Turning on IIS (static files only)...' -ForegroundColor Cyan
$features = 'Web-Server', 'Web-Static-Content', 'Web-Default-Doc', 'Web-Http-Errors', 'Web-Stat-Compression', 'Web-Http-Logging', 'Web-Mgmt-Console'
if (Get-Command Install-WindowsFeature -ErrorAction SilentlyContinue) {
  $result = Install-WindowsFeature -Name $features
  if ($result.RestartNeeded -eq 'Yes') { Write-Warning 'Windows wants a restart to finish installing IIS. Restart, then run this script again.' }
} else {
  # Windows 10/11 (for testing): the same pieces as optional features.
  'IIS-WebServerRole', 'IIS-WebServer', 'IIS-StaticContent', 'IIS-DefaultDocument', 'IIS-HttpErrors', 'IIS-HttpCompressionStatic', 'IIS-ManagementConsole' |
    ForEach-Object { Enable-WindowsOptionalFeature -Online -FeatureName $_ -All -NoRestart | Out-Null }
}
Import-Module WebAdministration

Write-Host "2/4  Copying the website to $Path..." -ForegroundColor Cyan
New-Item -ItemType Directory -Force -Path $Path | Out-Null
robocopy $source $Path /MIR /NFL /NDL /NJH /NJS /NP | Out-Null
if ($LASTEXITCODE -ge 8) { throw "Copying the website failed (robocopy exit code $LASTEXITCODE)." }
icacls $Path /grant 'IIS_IUSRS:(OI)(CI)RX' /T /Q | Out-Null

Write-Host "3/4  Setting up the IIS site '$SiteName'..." -ForegroundColor Cyan
$hosts = @($Domain)
if ($WithWww) { $hosts += "www.$Domain" }
if (-not (Get-Website -Name $SiteName -ErrorAction SilentlyContinue)) {
  New-Website -Name $SiteName -PhysicalPath $Path -Port 80 -HostHeader $Domain | Out-Null
} else {
  Set-ItemProperty "IIS:\Sites\$SiteName" -Name physicalPath -Value $Path
}
foreach ($h in $hosts) {
  if (-not (Get-WebBinding -Name $SiteName -Protocol http -HostHeader $h -Port 80 -ErrorAction SilentlyContinue)) {
    New-WebBinding -Name $SiteName -Protocol http -Port 80 -HostHeader $h
  }
}
Start-Website -Name $SiteName -ErrorAction SilentlyContinue

Write-Host '4/4  Opening the firewall for HTTP and HTTPS...' -ForegroundColor Cyan
foreach ($rule in @(@{ Name = 'ResinCraft website HTTP'; Port = 80 }, @{ Name = 'ResinCraft website HTTPS'; Port = 443 })) {
  if (-not (Get-NetFirewallRule -DisplayName $rule.Name -ErrorAction SilentlyContinue)) {
    New-NetFirewallRule -DisplayName $rule.Name -Direction Inbound -Protocol TCP -LocalPort $rule.Port -Action Allow | Out-Null
  }
}

Write-Host ''
Write-Host "Done. http://$Domain/ now serves the ResinCraft website (once DNS points here)." -ForegroundColor Green
Write-Host 'Next: add HTTPS with win-acme, as described in DEPLOY-WINDOWS.md (step 4).'
