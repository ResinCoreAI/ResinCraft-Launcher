# ResinCraft Relay on Windows Server

The relay lets friends join servers hosted in ResinCraft Launcher at `<name>.rscraft.resincore.tech`. On
Windows Server it runs as the service **ResinCraft Relay** (`ResinCraftRelay`) from `C:\ResinCraft\relay`.

| File | What |
|---|---|
| `resincraft-relay.exe` | The relay (one program, nothing else to install) |
| `relay.toml.example` | The configuration; the install script turns it into `relay.toml` once |
| `scripts\install-relay.ps1` | Installs or updates the service (run again to update) |
| `scripts\uninstall-relay.ps1` | Removes the service (`-Purge` deletes the data too) |

Run the scripts in **PowerShell as Administrator**. If Windows blocks them: `Unblock-File .\scripts\*.ps1`.

## Ports

| Port | For | Open to |
|---|---|---|
| TCP 25565 | Players (Minecraft) | Everyone |
| TCP 7000 | ResinCraft Launcher (TLS) | Everyone |
| UDP 19200-19299 | Bedrock players (one port per Bedrock server online) | Everyone |
| TCP 443 | Admin dashboard (HTTPS, `/admin`) | Everyone, or nobody if you keep it local (see below) |
| TCP 7001 | Health check and metrics | This server only (no firewall rule) |
| TCP 80 | win-acme's certificate check (shared with IIS) | Everyone |

`install-relay.ps1` opens Windows Firewall for 25565, 7000, the Bedrock range (UDP) and the dashboard port. If
your hosting provider has its own firewall, open them there too. Nothing else on the server may use 25565 or
the Bedrock range. If IIS already serves HTTPS on 443, keep the dashboard on another port, for example
`listen = "0.0.0.0:7443"` (then it's `https://rscraft.resincore.tech:7443/admin`).

## 1. DNS

At the DNS provider for `resincore.tech`:

| Name | Type | Value | Proxy (Cloudflare) |
|---|---|---|---|
| `*.rscraft` | A | the server's IPv4 | **DNS only** (grey cloud) |
| `rscraft` | A | the server's IPv4 | Proxied (orange) is fine: only the dashboard uses it |

Add matching AAAA records if the server has IPv6. `*.rscraft` must be **DNS only**: the relay carries raw
Minecraft and TLS traffic, which Cloudflare's proxy can't. It covers every player address
(`name.rscraft.resincore.tech`) and **`relay.rscraft.resincore.tech`, where the launcher connects** (port 7000)
and the name on the certificate. Don't point the launcher at the bare `rscraft.resincore.tech`: behind
Cloudflare's proxy, port 7000 doesn't get through. ("relay" is a reserved name, so no player can take it.)

Check from any PC: `nslookup test.rscraft.resincore.tech 1.1.1.1` and `nslookup rscraft.resincore.tech 1.1.1.1`
should both show the server's address.

## 2. Certificate (win-acme)

The launcher only connects to a relay with a valid certificate, and the dashboard uses the same one.
[win-acme](https://www.win-acme.com) gets a free Let's Encrypt certificate and renews it by itself.

1. Download the win-acme "pluggable" x64 zip and unzip it to `C:\win-acme`.
2. In PowerShell as Administrator:

   ```powershell
   C:\win-acme\wacs.exe --source manual --host relay.rscraft.resincore.tech --validation selfhosting --store pemfiles --pemfilespath C:\ResinCraft\relay\certs --pemfilesname relay --installation none --accepttos --emailaddress you@example.com
   ```

   (Use your own email address; Let's Encrypt writes to it if renewals fail.)

That writes `C:\ResinCraft\relay\certs\relay-chain.pem` and `relay-key.pem`, which `relay.toml` points at.
win-acme checks the domain over port 80 with its own listener, which shares port 80 with IIS (both use
HTTP.sys), so the website keeps running and no IIS binding is needed. It adds a scheduled task that renews the
certificate about every 60 days, and the relay picks up the new files within a few minutes, without a
restart.

## 3. Install

```powershell
.\scripts\install-relay.ps1
```

It copies the relay to `C:\ResinCraft\relay` (keeping an existing `relay.toml`, `data\`, `certs\` and
`logs\`), registers and configures the service (starts with Windows, restarts by itself if it ever stops
unexpectedly), opens the firewall, asks for the **admin dashboard password**, checks the configuration and
certificate, and starts the relay. If the certificate isn't there yet, it says so and doesn't start; get the
certificate (step 2) and run the script again.

Look at `C:\ResinCraft\relay\relay.toml` once: `base_domain`, the ports and the `[limits]` are the ones to
check. After editing it: `Restart-Service ResinCraftRelay`.

## 4. Admin dashboard

Open **https://rscraft.resincore.tech/admin** and sign in with the admin password. (An older `relay.toml` with
`listen = "0.0.0.0:7443"` keeps working at `https://rscraft.resincore.tech:7443/admin`; change it to
`0.0.0.0:443` and run the install script again for the short address.)

- **Service switch** (sidebar): **On**, **Paused** (no new tunnels; running ones finish their session) or **Off**
  (every tunnel closes now), with an optional message players' launchers show.
- **Live tunnels**: who's online, since when, players, traffic, session time left and how often it was continued;
  **Extend**, **End now** and **Kick**.
- **Sessions**: 2 hours by default (`session_minutes` in Settings or `relay.toml`, 0 = no limit). Hosts get a
  Continue banner 10 and 1 minutes before the end, and may press Continue in the launcher from 15 minutes before
  (`session_continue_minutes`). Launcher 0.3.0 can't show it, so use 0 until hosts update.
- **Names**: search, rename or release names; reserve words nobody may take.
- **Bans**: ban a Minecraft account (UUID) or an IP / CIDR range for a preset time, until a date or permanently, with a
  reason. The banned player's launcher shows how long and why.
- **Usage**: this month's traffic per account against its cap. **Add data** gives an account more GB for the
  rest of this month; **Unlimited** takes away its data cap and session time limit (an *Unlimited* badge shows it;
  **Turn off unlimited** undoes it). Type a Minecraft name or UUID at the top for an account not in the list yet.
  Both are written to the events log.
- **Data top-ups**: hosts get 100 GB a month (`monthly_gb_per_account`). Once less than 1 GB is left
  (`topup_low_gb`), or after it ran out, a host can press **Get 30 GB more** in the launcher (`topup_gb`), up to
  3 times a month (`topups_per_month`; 0 = only you add data). All four are in Settings and `relay.toml`.
- **Settings**: the limits; changes apply at once and are kept in `data\relay.json` (Reset goes back to
  `relay.toml`).
- **Events**: the last 500 sign-ins, kicks, bans and admin actions, with IP addresses (they appear only there).

Five wrong passwords lock that address out for 15 minutes. Sign-ins last 12 hours.

**Change the password**: `.\scripts\install-relay.ps1 -ResetPassword`, or

```powershell
C:\ResinCraft\relay\resincraft-relay.exe --config C:\ResinCraft\relay\relay.toml set-admin-password
```

No restart is needed.

**Keep it private** (optional): in `relay.toml` set `[dashboard] listen = "127.0.0.1:7443"` and run the install
script again (it removes the firewall rule). Then open `https://localhost:7443/admin` on the server over Remote
Desktop (the browser warns that the certificate is for rscraft.resincore.tech; that's expected), or forward it
over SSH from your PC with `ssh -L 7443:127.0.0.1:7443 admin@rscraft.resincore.tech` and open
`https://localhost:7443/admin`. Alternatively keep it public and list your own addresses in `allow_ips`.

## Bedrock servers

Launcher 0.4.2 and later can share Bedrock servers through the relay too. Each Bedrock server online gets a
UDP port of its own from `bedrock_ports` (`19200-19299` by default; the same name gets the same port next
time when it's free). Players add `<name>.rscraft.resincore.tech` with that port under Servers → Add server;
the launcher shows the full address. A Java player who types a Bedrock server's name is told to use Bedrock.
`bedrock_ports = ""` turns Bedrock tunnels off. An `relay.toml` from before has no `bedrock_ports` line and
gets the default range: run the install script again so it opens the range in the firewall.

## 5. The launcher

ResinCraft Launcher finds this relay through two values compiled in at build time (in `keys.local.env` or as
environment variables in the release build):

```
RESINCRAFT_RELAY_URL=tls://relay.rscraft.resincore.tech:7000
RESINCRAFT_RELAY_DOMAIN=rscraft.resincore.tech
```

## Day to day

- **Status**: `Get-Service ResinCraftRelay`; the dashboard; or
  `C:\ResinCraft\relay\resincraft-relay.exe --config C:\ResinCraft\relay\relay.toml health`.
- **Logs**: `C:\ResinCraft\relay\logs\relay.YYYY-MM-DD.log`, one file per day, the last 30 kept. Follow live:
  `Get-Content C:\ResinCraft\relay\logs\relay.$(Get-Date -Format yyyy-MM-dd).log -Wait`. Player addresses
  aren't in the logs at the normal level.
- **Restart**: `Restart-Service ResinCraftRelay`. Stopping is graceful: launchers are told to reconnect (they
  keep their names), players get 10 seconds, the data is saved.
- **Update**: unzip the new package and run `.\scripts\install-relay.ps1` again. It stops the service, replaces
  the program, keeps `relay.toml`, `data\` and `certs\`, and starts it. By hand: `Stop-Service ResinCraftRelay`,
  replace `resincraft-relay.exe`, `Start-Service ResinCraftRelay`.
- **Backups**: copy `C:\ResinCraft\relay\relay.toml` and `C:\ResinCraft\relay\data\relay.json` (names, bans,
  this month's traffic; written every 30 seconds). Restoring: stop the service, put them back, start it.
- **Remove**: `.\scripts\uninstall-relay.ps1` (keeps the folder) or `.\scripts\uninstall-relay.ps1 -Purge`.

## When something's wrong

- **The service doesn't start**: run
  `C:\ResinCraft\relay\resincraft-relay.exe --config C:\ResinCraft\relay\relay.toml check` in PowerShell; it
  says what's wrong (a typo in relay.toml, a missing certificate, a port in use). The newest log file has the
  rest.
- **A port is in use**: `Get-NetTCPConnection -LocalPort 25565 -State Listen | Select OwningProcess`.
- **The launcher can't connect**: check that `rscraft.resincore.tech` resolves to the server, that TCP 7000 is
  open in the provider's firewall, and that the certificate is valid
  (`Test-NetConnection rscraft.resincore.tech -Port 7000`).
- **Players see "Unknown address"**: the `*.rscraft` record is missing, or proxied by Cloudflare.
- **More detail in the logs**: set the service's environment to `RUST_LOG=debug` (adds player addresses):
  `Set-ItemProperty HKLM:\SYSTEM\CurrentControlSet\Services\ResinCraftRelay -Name Environment -Type MultiString -Value 'RUST_LOG=debug'`,
  then restart the service. Remove the value again afterwards.
