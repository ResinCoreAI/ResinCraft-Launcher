<#
.SYNOPSIS
  Installs or updates ResinCraft Relay as a Windows service. Run in PowerShell as Administrator.

.DESCRIPTION
  Safe to run again (that's how you update):
    - stops the running service (gracefully: launchers reconnect, data is saved);
    - copies resincraft-relay.exe to the install folder (C:\ResinCraft\relay), keeping relay.toml, data\,
      certs\ and logs\; the first time, relay.toml is created from relay.toml.example;
    - registers the "ResinCraft Relay" service (ResinCraftRelay): starts with Windows, runs as LocalSystem,
      restarts by itself if it ever stops unexpectedly;
    - opens Windows Firewall for the player, launcher and dashboard ports (read from relay.toml);
    - asks for the admin dashboard password if none is set yet (-ResetPassword to change it);
    - checks the config and certificate, then starts the service.

  From the deploy zip it finds the relay files in ..\relay\ next to this scripts\ folder; from the repository,
  in relay\target\release\ and relay\deploy\windows\.

.EXAMPLE
  .\scripts\install-relay.ps1
.EXAMPLE
  .\scripts\install-relay.ps1 -ResetPassword
#>
#Requires -RunAsAdministrator
[CmdletBinding()]
param(
  [string]$InstallDir = 'C:\ResinCraft\relay',
  [switch]$ResetPassword,
  [switch]$NoStart
)

$ErrorActionPreference = 'Stop'
$ServiceName = 'ResinCraftRelay'
$DisplayName = 'ResinCraft Relay'

function Step($text) { Write-Host "==> $text" -ForegroundColor Cyan }
function Ok($text) { Write-Host "    $text" -ForegroundColor Green }
function Note($text) { Write-Host "    $text" -ForegroundColor Yellow }

# Windows PowerShell 5.1 turns a program's redirected stderr into a terminating error under 'Stop'.
function Invoke-Quiet([scriptblock]$Block) {
  $old = $ErrorActionPreference
  $ErrorActionPreference = 'Continue'
  try { & $Block 2>&1 | Out-Null } finally { $ErrorActionPreference = $old }
}

function First-Existing([string[]]$paths) {
  foreach ($p in $paths) { if ($p -and (Test-Path -LiteralPath $p)) { return (Resolve-Path -LiteralPath $p).Path } }
  return $null
}

# ---- Find the files (deploy zip layout, then repository layout) ----
$exe = First-Existing @((Join-Path $PSScriptRoot '..\relay\resincraft-relay.exe'), (Join-Path $PSScriptRoot '..\..\target\release\resincraft-relay.exe'))
$example = First-Existing @((Join-Path $PSScriptRoot '..\relay\relay.toml.example'), (Join-Path $PSScriptRoot 'relay.toml.example'))
$readme = First-Existing @((Join-Path $PSScriptRoot '..\relay\README.md'), (Join-Path $PSScriptRoot 'README.md'))
if (-not $exe) { throw "resincraft-relay.exe wasn't found next to this script (..\relay\). Build it with: cargo build --release --manifest-path relay\Cargo.toml" }
if (-not $example) { throw "relay.toml.example wasn't found next to this script." }

$config = Join-Path $InstallDir 'relay.toml'
$target = Join-Path $InstallDir 'resincraft-relay.exe'

# ---- Stop the running service ----
$svc = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
if ($svc -and $svc.Status -ne 'Stopped') {
  Step 'Stopping the running relay (launchers reconnect by themselves)'
  Stop-Service -Name $ServiceName
  $svc.WaitForStatus('Stopped', [TimeSpan]::FromSeconds(60))
  Ok 'Stopped.'
}
# An older install may have used a scheduled task instead.
if (Get-ScheduledTask -TaskName $DisplayName -ErrorAction SilentlyContinue) {
  Step "Removing the old scheduled task '$DisplayName'"
  Stop-ScheduledTask -TaskName $DisplayName -ErrorAction SilentlyContinue
  Unregister-ScheduledTask -TaskName $DisplayName -Confirm:$false
  Ok 'Removed.'
}

# ---- Copy the files ----
Step "Copying the relay to $InstallDir"
foreach ($dir in @($InstallDir, (Join-Path $InstallDir 'data'), (Join-Path $InstallDir 'logs'), (Join-Path $InstallDir 'certs'))) {
  New-Item -ItemType Directory -Force -Path $dir | Out-Null
}
for ($try = 1; ; $try++) {
  try { Copy-Item -LiteralPath $exe -Destination $target -Force; break }
  catch { if ($try -ge 10) { throw }; Start-Sleep -Seconds 1 }   # the old process may still be letting go of the file
}
Ok "$(& $target --version) ($('{0:N1} MB' -f ((Get-Item $target).Length / 1MB)))"
Copy-Item -LiteralPath $example -Destination (Join-Path $InstallDir 'relay.toml.example') -Force
if ($readme) { Copy-Item -LiteralPath $readme -Destination (Join-Path $InstallDir 'README.md') -Force }
if (Test-Path -LiteralPath $config) {
  Ok 'Kept the existing relay.toml (the new example is relay.toml.example).'
} else {
  Copy-Item -LiteralPath $example -Destination $config
  Ok 'Created relay.toml from the example. Review it (base_domain, ports, limits).'
}

# ---- Read the ports and certificate paths from relay.toml ----
$text = Get-Content -LiteralPath $config -Raw
function Port-Of([string]$pattern, [int]$fallback) {
  $m = [regex]::Match($text, $pattern, 'Multiline')
  if ($m.Success) { return [int]$m.Groups[1].Value } else { return $fallback }
}
$playerPort = Port-Of '^\s*public_listen\s*=\s*\[\s*"[^"]*:(\d+)"' 25565
$agentPort = Port-Of '^\s*agent_listen\s*=\s*\[\s*"[^"]*:(\d+)"' 7000
# Bedrock servers' UDP ports ("19200-19299"; missing: the relay's default; "": none).
$bedrockMatch = [regex]::Match($text, '(?m)^\s*bedrock_ports\s*=\s*"([^"]*)"')
$bedrockPorts = if ($bedrockMatch.Success) { $bedrockMatch.Groups[1].Value.Trim() } else { '19200-19299' }
$dashSection = [regex]::Match($text, '(?ms)^\s*\[dashboard\](.*?)(^\s*\[|\z)').Groups[1].Value
$dashMatch = [regex]::Match($dashSection, '(?m)^\s*listen\s*=\s*"([^"]*):(\d+)"')
$dashPort = if ($dashMatch.Success) { [int]$dashMatch.Groups[2].Value } else { $null }
$dashPublic = $dashMatch.Success -and $dashMatch.Groups[1].Value -notmatch '^(127\.|\[?::1\]?$|localhost)'
function Path-Of([string]$key) {
  $m = [regex]::Match($text, "(?m)^\s*$key\s*=\s*`"([^`"]+)`"")
  if (-not $m.Success) { return $null }
  $p = $m.Groups[1].Value
  if ([IO.Path]::IsPathRooted($p)) { return $p } else { return (Join-Path $InstallDir $p) }
}
$certFile = Path-Of 'cert'
$keyFile = Path-Of 'key'

# ---- Service ----
Step "Registering the Windows service '$DisplayName'"
$binPath = "`"$target`" --config `"$config`" service"
if (-not (Get-Service -Name $ServiceName -ErrorAction SilentlyContinue)) {
  New-Service -Name $ServiceName -DisplayName $DisplayName -BinaryPathName $binPath -StartupType Automatic `
    -Description 'ResinCraft Relay: friends join servers hosted in ResinCraft Launcher at <name>.rscraft.resincore.tech.' | Out-Null
  Ok 'Created (starts with Windows, runs as LocalSystem).'
} else {
  Set-ItemProperty -Path "HKLM:\SYSTEM\CurrentControlSet\Services\$ServiceName" -Name ImagePath -Value $binPath
  Ok 'Already registered; command line refreshed.'
}
# Start a little after boot (network up), and restart after 5 s, 10 s, then every minute if it stops unexpectedly.
& sc.exe config $ServiceName start= delayed-auto | Out-Null
& sc.exe failure $ServiceName reset= 86400 actions= restart/5000/restart/10000/restart/60000 | Out-Null
& sc.exe failureflag $ServiceName 1 | Out-Null
Ok 'Recovery: restarts by itself if it stops unexpectedly.'

# ---- Firewall ----
Step 'Opening Windows Firewall'
$rules = @(
  @{ Name = 'ResinCraftRelay-Players'; Display = 'ResinCraft Relay: players (Minecraft)'; Port = $playerPort },
  @{ Name = 'ResinCraftRelay-Agent'; Display = 'ResinCraft Relay: launchers'; Port = $agentPort }
)
if ($dashPort -and $dashPublic) { $rules += @{ Name = 'ResinCraftRelay-Dashboard'; Display = 'ResinCraft Relay: admin dashboard'; Port = $dashPort } }
foreach ($r in $rules) {
  $existing = Get-NetFirewallRule -Name $r.Name -ErrorAction SilentlyContinue
  if ($existing) {
    Set-NetFirewallRule -Name $r.Name -LocalPort $r.Port | Out-Null
    Ok "$($r.Display): TCP $($r.Port) (rule already there)"
  } else {
    New-NetFirewallRule -Name $r.Name -DisplayName $r.Display -Direction Inbound -Protocol TCP -LocalPort $r.Port -Action Allow -Profile Any | Out-Null
    Ok "$($r.Display): TCP $($r.Port)"
  }
}
if ($bedrockPorts) {
  $existing = Get-NetFirewallRule -Name 'ResinCraftRelay-Bedrock' -ErrorAction SilentlyContinue
  if ($existing) {
    Set-NetFirewallRule -Name 'ResinCraftRelay-Bedrock' -LocalPort $bedrockPorts | Out-Null
  } else {
    New-NetFirewallRule -Name 'ResinCraftRelay-Bedrock' -DisplayName 'ResinCraft Relay: Bedrock players' -Direction Inbound -Protocol UDP -LocalPort $bedrockPorts -Action Allow -Profile Any | Out-Null
  }
  Ok "ResinCraft Relay: Bedrock players: UDP $bedrockPorts"
} else {
  Remove-NetFirewallRule -Name 'ResinCraftRelay-Bedrock' -ErrorAction SilentlyContinue
}
if ($dashPort -and -not $dashPublic) {
  Remove-NetFirewallRule -Name 'ResinCraftRelay-Dashboard' -ErrorAction SilentlyContinue
  Ok "Dashboard on this machine only (port $dashPort): no firewall rule."
}

# ---- Admin password ----
$hasPassword = [regex]::IsMatch($dashSection, '(?m)^\s*password_hash\s*=\s*"\$argon2')
if ($dashPort -and (-not $hasPassword -or $ResetPassword)) {
  Step 'Admin dashboard password'
  while ($true) {
    $first = Read-Host -AsSecureString 'New admin password (at least 10 characters)'
    $again = Read-Host -AsSecureString 'Again'
    $p1 = [Runtime.InteropServices.Marshal]::PtrToStringBSTR([Runtime.InteropServices.Marshal]::SecureStringToBSTR($first))
    $p2 = [Runtime.InteropServices.Marshal]::PtrToStringBSTR([Runtime.InteropServices.Marshal]::SecureStringToBSTR($again))
    if ($p1 -ne $p2) { Note 'The two passwords differ. Try again.'; continue }
    if ($p1.Length -lt 10) { Note 'Use at least 10 characters.'; continue }
    break
  }
  # Through stdin, never on the command line (other processes can read command lines).
  $OutputEncoding = New-Object System.Text.UTF8Encoding $false
  $p1 | & $target --config $config set-admin-password --stdin
  $code = $LASTEXITCODE
  $p1 = $null; $p2 = $null
  if ($code -ne 0) { throw 'Setting the admin password failed.' }
  Ok 'Saved (as an Argon2 hash in relay.toml).'
} elseif ($dashPort) {
  Ok 'Admin password already set (run with -ResetPassword to change it).'
}

# ---- Certificate ----
Step 'Checking the certificate'
$certsReady = $true
foreach ($f in @($certFile, $keyFile)) {
  if ($f -and -not (Test-Path -LiteralPath $f)) { $certsReady = $false; Note "Missing: $f" }
}
if (-not $certsReady) {
  Note 'Get the certificate with win-acme first (README.md, "Certificate"), for example:'
  Note "  wacs.exe --source manual --host rscraft.resincore.tech --validation selfhosting --store pemfiles --pemfilespath `"$InstallDir\certs`" --pemfilesname relay --installation none --accepttos --emailaddress you@example.com"
  Note 'Then run this script again, or: Start-Service ResinCraftRelay'
} else {
  Ok 'Certificate files are there.'
}

# ---- Check and start ----
if ($certsReady -and -not $NoStart) {
  Step 'Checking the config'
  & $target --config $config check
  if ($LASTEXITCODE -ne 0) { throw "relay.toml has a problem (see above). Fix it, then run this script again." }
  Step 'Starting the relay'
  Start-Service -Name $ServiceName
  (Get-Service -Name $ServiceName).WaitForStatus('Running', [TimeSpan]::FromSeconds(30))
  $healthy = $false
  for ($i = 0; $i -lt 10 -and -not $healthy; $i++) {
    Start-Sleep -Seconds 1
    Invoke-Quiet { & $target --config $config health }
    $healthy = ($LASTEXITCODE -eq 0)
  }
  if ($healthy) { Ok 'Running and healthy.' } else { Note "Started, but the health check didn't answer yet. Look at $InstallDir\logs\." }
}

# ---- Summary ----
$domain = ([regex]::Match($text, '(?m)^\s*base_domain\s*=\s*"([^"]+)"')).Groups[1].Value
Write-Host ''
Write-Host 'ResinCraft Relay' -ForegroundColor Green
Write-Host "  Players join      <name>.$domain (port $playerPort)"
Write-Host "  Launchers connect tls://$($domain):$agentPort"
if ($bedrockPorts) { Write-Host "  Bedrock players   <name>.$domain, UDP port from $bedrockPorts" }
if ($dashPort -eq 443) { Write-Host "  Admin dashboard   https://$domain/admin" } elseif ($dashPort) { Write-Host "  Admin dashboard   https://$($domain):$dashPort/admin" }
Write-Host "  Folder            $InstallDir (relay.toml, data\, certs\, logs\)"
Write-Host "  Service           Get-Service $ServiceName  |  Restart-Service $ServiceName"
