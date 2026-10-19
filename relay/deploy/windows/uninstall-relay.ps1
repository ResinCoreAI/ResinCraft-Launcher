<#
.SYNOPSIS
  Removes the ResinCraft Relay service and its firewall rules. Run in PowerShell as Administrator.

.DESCRIPTION
  Stops the service gracefully, removes it and the three firewall rules, and deletes resincraft-relay.exe.
  relay.toml, data\ (names, bans), certs\ and logs\ stay, so a later install-relay.ps1 picks up where it left
  off. Add -Purge to delete the whole folder too.

.EXAMPLE
  .\scripts\uninstall-relay.ps1
.EXAMPLE
  .\scripts\uninstall-relay.ps1 -Purge
#>
#Requires -RunAsAdministrator
[CmdletBinding()]
param(
  [string]$InstallDir = 'C:\ResinCraft\relay',
  [switch]$Purge
)

$ErrorActionPreference = 'Stop'
$ServiceName = 'ResinCraftRelay'

function Step($text) { Write-Host "==> $text" -ForegroundColor Cyan }
function Ok($text) { Write-Host "    $text" -ForegroundColor Green }

$svc = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
if ($svc) {
  Step 'Stopping and removing the service'
  if ($svc.Status -ne 'Stopped') {
    Stop-Service -Name $ServiceName
    $svc.WaitForStatus('Stopped', [TimeSpan]::FromSeconds(60))
  }
  & sc.exe delete $ServiceName | Out-Null
  Ok 'Removed.'
} else {
  Ok 'The service isn''t installed.'
}
if (Get-ScheduledTask -TaskName 'ResinCraft Relay' -ErrorAction SilentlyContinue) {
  Stop-ScheduledTask -TaskName 'ResinCraft Relay' -ErrorAction SilentlyContinue
  Unregister-ScheduledTask -TaskName 'ResinCraft Relay' -Confirm:$false
  Ok 'Removed the old scheduled task too.'
}

Step 'Removing the firewall rules'
foreach ($name in 'ResinCraftRelay-Players', 'ResinCraftRelay-Agent', 'ResinCraftRelay-Bedrock', 'ResinCraftRelay-Dashboard') {
  if (Get-NetFirewallRule -Name $name -ErrorAction SilentlyContinue) {
    Remove-NetFirewallRule -Name $name
    Ok "Removed $name."
  }
}

if (Test-Path -LiteralPath $InstallDir) {
  if ($Purge) {
    Step "Deleting $InstallDir (relay.toml, names, bans, certificates, logs)"
    Start-Sleep -Seconds 1
    Remove-Item -LiteralPath $InstallDir -Recurse -Force
    Ok 'Deleted.'
  } else {
    Step 'Removing the program'
    Remove-Item -LiteralPath (Join-Path $InstallDir 'resincraft-relay.exe') -Force -ErrorAction SilentlyContinue
    Ok "Kept $InstallDir (relay.toml, data\, certs\, logs\). Add -Purge to delete it."
  }
}
Write-Host ''
Write-Host 'ResinCraft Relay is uninstalled.' -ForegroundColor Green
