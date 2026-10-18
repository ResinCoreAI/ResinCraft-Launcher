# Deploying ResinCraft on Windows Server

This zip has everything for one Windows Server (2019, 2022 or 2025):

| Folder | What |
|---|---|
| `site\` | The website (static files, with an IIS `web.config`) |
| `relay\` | The ResinCraft Relay (`resincraft-relay.exe`), its example config and its own README |
| `scripts\` | `install-site.ps1`, `install-relay.ps1`, `uninstall-relay.ps1` |

Work through the steps in order. Run every script in **PowerShell as Administrator**, from the unzipped folder (for example `C:\ResinCraft-Deploy`). If Windows blocks the scripts, run `Unblock-File .\scripts\*.ps1` first.

## 1. Before you start

- The server needs a public IPv4 address.
- If your hosting provider has its own firewall, open these TCP ports (the scripts open the Windows Firewall for you):
  - **80** and **443**: website and certificates;
  - **25565**: players;
  - **7000**: launchers connecting to the relay;
  - **7443**: admin dashboard.
  
  Port 7001 (health check) stays private on the server.
- Nothing else may use port 25565 on this server, because the relay listens there.

## 2. DNS records

Create these at the DNS provider for `resincore.tech` (or whichever domain you use):

| Name | Type | Value | Notes |
|---|---|---|---|
| your website host, e.g. `resincraft` | A | server IP | May be proxied (orange cloud) on Cloudflare |
| `*.rscraft` | A | server IP | **DNS only** (grey cloud): players connect with raw Minecraft traffic, which Cloudflare's proxy can't carry |
| `rscraft` | A | server IP | **DNS only**: used by the launcher and the admin dashboard. The wildcard doesn't cover the bare name |

## 3. Website

```powershell
.\scripts\install-site.ps1 -Domain resincraft.resincore.tech
```

This turns on IIS (static files only), copies the site to `C:\ResinCraft\site`, creates the IIS site **ResinCraft** on port 80 and opens the firewall. To update the site later, unzip a new package and run the same command again.

## 4. HTTPS for the website (win-acme)

1. Download win-acme from <https://www.win-acme.com> (the "pluggable" x64 zip) and unzip it to `C:\win-acme`.
2. Run `C:\win-acme\wacs.exe` as Administrator, choose **N: Create certificate (default settings)**, and pick the **ResinCraft** site.
3. win-acme gets a free Let's Encrypt certificate, adds the HTTPS binding in IIS, and renews it automatically through a scheduled task.

If the website host is proxied by Cloudflare, you can skip this and use Cloudflare's SSL mode **Full** instead. Using win-acme as well is still recommended (Full (strict)).

## 5. Relay

`relay\README.md` has the details. In short:

1. **Certificate.** win-acme shares port 80 with IIS, so no extra IIS setup is needed. Replace `you@example.com` with your email:
   ```powershell
   C:\win-acme\wacs.exe --source manual --host rscraft.resincore.tech --validation selfhosting --store pemfiles --pemfilespath C:\ResinCraft\relay\certs --pemfilesname relay --installation none --accepttos --emailaddress you@example.com
   ```
   This writes `certs\relay-chain.pem` and `certs\relay-key.pem`, which the relay's config already points at. win-acme renews the certificate on its own, and the relay picks up the new files by itself. Get the certificate **before** step 2: without it, the install script stops and tells you.
2. **Install.** Run `.\scripts\install-relay.ps1`. It:
   - copies the relay to `C:\ResinCraft\relay` (it keeps an existing `relay.toml`, data and certificates);
   - installs the Windows service **ResinCraft Relay** (`ResinCraftRelay`), which starts with Windows and restarts by itself after a crash;
   - opens the firewall for the ports in `relay.toml` (25565, 7000 and 7443);
   - asks you for an admin password if none is set yet. To change it later, run the script with `-ResetPassword`, or run `C:\ResinCraft\relay\resincraft-relay.exe --config C:\ResinCraft\relay\relay.toml set-admin-password` (no restart needed).
3. **Check it's running.** `Get-Service ResinCraftRelay` should say *Running*. Logs are in `C:\ResinCraft\relay\logs\relay.YYYY-MM-DD.log` and kept for 30 days.

## 6. Admin dashboard

Open `https://rscraft.resincore.tech:7443` and sign in with the admin password from step 5. Five wrong passwords lock that IP out for 15 minutes. To allow only your own IPs, set `allow_ips` in `relay.toml`.
- **Service switch:** On, Paused or Off, with a message for players' launchers (for maintenance).
- **Live tunnels:** see who's online and each session's time left; extend, end or kick tunnels.
- **Sessions:** 2 hours by default; hosts press Continue for more. Set it to 0 (no limit) while players are still on launcher 0.3.0, which can't show the Continue button.
- **Names and bans:** release or rename names, and ban accounts or IPs.
- **Usage and limits:** check bandwidth and change limits.
- **Events log.**

To keep the dashboard private, set its address to `127.0.0.1` in `relay.toml` and reach it over Remote Desktop or an SSH tunnel.

## 7. Point the launcher at your relay

The launcher learns the relay's address when it's built. Before building the next release, add these two values to `keys.local.env` (for local builds) and as GitHub repository secrets (for releases), the same way as the other keys:

```
RESINCRAFT_RELAY_URL=tls://rscraft.resincore.tech:7000
RESINCRAFT_RELAY_DOMAIN=rscraft.resincore.tech
```

Set the GitHub secrets by piping the value in, never with `gh secret set -f`, which mangles `$` characters:
```powershell
'tls://rscraft.resincore.tech:7000' | gh secret set RESINCRAFT_RELAY_URL -R ResinCoreAI/ResinCraft-Launcher
```
```powershell
'rscraft.resincore.tech' | gh secret set RESINCRAFT_RELAY_DOMAIN -R ResinCoreAI/ResinCraft-Launcher
```

## 8. Check everything

- `https://<your website host>/`: the site loads, and the Download button points to the newest release.
- `https://rscraft.resincore.tech:7443`: the admin dashboard asks for your password.
- In ResinCraft Launcher, host a server and pick **ResinCraft Relay** (or Automatic): the Internet card shows `yourname.rscraft.resincore.tech`.
- A friend adds that address in Minecraft and joins.

## Updating

- **Website:** run `install-site.ps1` again from a new zip.
- **Relay:** run `install-relay.ps1` again. It stops the service gracefully (launchers reconnect by themselves), replaces the exe, keeps `relay.toml`, data and certificates, and starts it again.
- **Removing the relay:** `.\scripts\uninstall-relay.ps1` (add `-Purge` to delete its data too).
