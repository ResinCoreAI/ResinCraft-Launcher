# ResinCraft Relay

Friends join a server hosted in ResinCraft Launcher by typing `<name>.rscraft.resincore.tech` into Minecraft.
They install nothing, and the host forwards no ports.

```
 Minecraft ──TCP 25565──▶ ResinCraft Relay (server) ◀──TLS 7000 (yamux)── ResinCraft Launcher ──▶ local server
 "smp.rscraft.resincore.tech"  reads the handshake,       one connection,      on the host's PC
                               routes by name             a stream per player
                               admin dashboard: https://rscraft.resincore.tech/admin
 Minecraft Bedrock ──UDP 19200-19299──▶ (one port per Bedrock server, datagrams framed on a stream)
```

- **Players** connect to port 25565. The relay reads the Minecraft handshake (the server address the player
  typed), routes it by name and forwards the buffered bytes unchanged. When the host is offline, the relay
  answers the server list itself ("This server is offline").
- **The launcher** (the agent) keeps one TLS connection to port 7000, multiplexed with yamux. For every player,
  the relay opens a new stream on it, and the launcher joins that stream to the local server.
- **Sign-in:** the launcher sends the host's Minecraft access token once. The relay checks it with
  `api.minecraftservices.com/minecraft/profile`, forgets it and hands out its own session token. Mojang
  tokens are never stored or logged.
- **Names** belong to Minecraft accounts, first come, first served: `^[a-z0-9-]{3,24}$`, not starting or ending
  with a dash, no `--`, and not a reserved word. Names nobody brings online for `name_expiry_days` are freed.
- **Limits per account:** names, servers online at once, players per server, and traffic per month. Bans apply
  to Minecraft UUIDs and to IP addresses or ranges, forever or for a while.
- **Admin dashboard** on its own HTTPS port: live tunnels (kick), names (rename, release, reserved words), bans,
  usage, limits (changed live) and an events log.

The base domain is `base_domain` in the config; `rscraft.resincore.tech` is the default used throughout.

## Deploy

- **Windows Server**: see [deploy/windows/README.md](deploy/windows/README.md). The relay runs as a Windows
  service; win-acme provides the certificate next to IIS; `install-relay.ps1` does the rest.
- **Linux**: Docker or systemd, below.

Either way you need:

| DNS record | Type | Value | Notes |
|---|---|---|---|
| `*.rscraft.resincore.tech` | A (+ AAAA) | server IP | **DNS only** (grey cloud on Cloudflare): players' raw Minecraft traffic |
| `rscraft.resincore.tech` | A (+ AAAA) | server IP | **DNS only**: the launcher (7000) and the dashboard (443, `/admin`); the certificate's name. The wildcard doesn't cover the bare name |

| Port | For | Firewall |
|---|---|---|
| TCP 25565 | Players | open |
| TCP 7000 | Launchers (TLS) | open |
| UDP 19200-19299 | Bedrock players (`bedrock_ports`) | open |
| TCP 443 | Admin dashboard (HTTPS, `/admin`) | open, or closed if it listens on 127.0.0.1 |
| TCP 7001 | /healthz, /metrics, bearer-token admin API | closed (127.0.0.1 only) |

And the launcher build has to know the relay (in `keys.local.env` or the release build's environment):

```
RESINCRAFT_RELAY_URL=tls://rscraft.resincore.tech:7000
RESINCRAFT_RELAY_DOMAIN=rscraft.resincore.tech
```

Builds without them don't offer ResinCraft Relay. For development, the same variables set at runtime override
the built-in values (`tcp://127.0.0.1:7000` for a local relay without TLS); `RESINCRAFT_RELAY_CA` can name a
PEM file with an extra root certificate to trust.

## Build

```bash
cd relay
cargo test                  # unit tests, the end-to-end tests on 127.0.0.1, the dashboard's security tests
cargo build --release       # target/release/resincraft-relay(.exe)
```

On Windows this gives `target\release\resincraft-relay.exe`. For a static Linux binary:
`rustup target add x86_64-unknown-linux-musl`, then `cargo build --release --target x86_64-unknown-linux-musl`.
TLS uses ring, so no OpenSSL is needed anywhere.

## Configure

`relay.example.toml` (Linux) and `deploy/windows/relay.toml.example` (Windows) document every option.
Relative paths are next to the config file. The important ones:

- `base_domain = "rscraft.resincore.tech"`
- `[tls]` `cert` / `key`: the PEM certificate chain and private key for `rscraft.resincore.tech`. They're
  re-read when they change (checked every 5 minutes), so renewals need no restart.
- `[dashboard]` `listen = "0.0.0.0:7443"` (or `127.0.0.1:7443` to keep it local); without `[tls]` it may only
  listen on a loopback address.
- `log_file = "logs/relay.log"`: daily files (`relay.2026-10-18.log` …), `log_keep_days` of them.
- `[limits]`: defaults are 5 names, 3 servers online and 64 players each per account, 100 GB per account per
  month.

`resincraft-relay --config relay.toml check` validates the config, the certificate and the ports.

### The admin password

```bash
resincraft-relay --config relay.toml set-admin-password            # asks twice, no echo
echo 'a long password' | resincraft-relay --config relay.toml set-admin-password --stdin
```

It writes an Argon2id hash into `[dashboard] password_hash` (keeping the rest of the file as it was). A running
relay uses it at the next sign-in; no restart.

## Run on Linux

### Certificate

With Let's Encrypt, if `resincore.tech` is on Cloudflare (no port 80 needed):

```bash
sudo apt install certbot python3-certbot-dns-cloudflare
sudo install -m 0600 /dev/null /root/cloudflare.ini
echo "dns_cloudflare_api_token = <token with Zone → DNS → Edit>" | sudo tee /root/cloudflare.ini
sudo certbot certonly --dns-cloudflare --dns-cloudflare-credentials /root/cloudflare.ini -d rscraft.resincore.tech
```

(Or, with port 80 free for a minute: `sudo certbot certonly --standalone -d rscraft.resincore.tech`.)
Let's Encrypt keeps keys readable by root only, so `deploy/certbot-deploy-hook.sh` copies them for the relay
after every renewal. Set its `OWNER` (`resincraft-relay` for systemd, `65532` for Docker), then:

```bash
sudo install -m 0755 deploy/certbot-deploy-hook.sh /etc/letsencrypt/renewal-hooks/deploy/resincraft-relay.sh
sudo RENEWED_LINEAGE=/etc/letsencrypt/live/rscraft.resincore.tech /etc/letsencrypt/renewal-hooks/deploy/resincraft-relay.sh
```

### Firewall

```bash
sudo ufw allow 25565/tcp
sudo ufw allow 7000/tcp
sudo ufw allow 443/tcp       # unless the dashboard listens on 127.0.0.1
sudo ufw allow 19200:19299/udp  # Bedrock servers
```

### Docker

```bash
cd relay
mkdir -p config/tls data
cp relay.example.toml config/relay.toml   # edit; inside the container: admin_listen = "0.0.0.0:7001",
                                          # [tls] cert = "tls/fullchain.pem", key = "tls/privkey.pem",
                                          # data_file = "/var/lib/resincraft-relay/relay.json"
sudo chown -R 65532:65532 config data     # the image runs as uid 65532
docker compose build
docker compose run --rm relay set-admin-password
docker compose up -d
docker compose logs -f
```

Compose publishes 25565, 7000, 443 and the Bedrock range (UDP), and the local admin port on the host's 127.0.0.1 only. Point the certbot
deploy hook's `DEST` at `config/tls` (with `OWNER=65532`).

### systemd

```bash
cargo build --release --target x86_64-unknown-linux-musl
sudo useradd --system --no-create-home --shell /usr/sbin/nologin resincraft-relay
sudo install -m 0755 target/x86_64-unknown-linux-musl/release/resincraft-relay /usr/local/bin/
sudo install -d -o resincraft-relay -g resincraft-relay /etc/resincraft-relay /etc/resincraft-relay/tls
sudo install -m 0640 -g resincraft-relay relay.toml /etc/resincraft-relay/relay.toml
sudo resincraft-relay --config /etc/resincraft-relay/relay.toml set-admin-password
sudo install -m 0644 deploy/resincraft-relay.service /etc/systemd/system/
sudo systemctl daemon-reload && sudo systemctl enable --now resincraft-relay
journalctl -u resincraft-relay -f
```

## The admin dashboard

Open `https://rscraft.resincore.tech/admin` and sign in with the admin password. If it listens on 127.0.0.1,
forward it over SSH: `ssh -L 7443:127.0.0.1:7443 you@rscraft.resincore.tech`, then open
`https://localhost:7443/admin` (the browser will warn that the certificate is for rscraft.resincore.tech).

- **Service switch** (sidebar): **On**, **Paused** (no new tunnels; running ones finish their session) or **Off**
  (every tunnel closes now), with an optional message that launchers show. Kept across restarts.
- **Live tunnels**: address, Minecraft account and UUID, online since, players, traffic in and out, session time
  left and how often it was continued; **Extend** gives another session, **End now** ends it, **Kick**
  disconnects a server (its launcher doesn't reconnect by itself).
- **Names**: search; rename or release names; reserve or unreserve words.
- **Bans**: ban or unban a Minecraft UUID or an IP / CIDR range, with a reason and an optional duration in hours.
  Banning an account disconnects it at once.
- **Usage**: bandwidth per account this month against the cap, top users first, totals.
- **Settings**: view and change the limits. Changes apply at once and are kept in the data file (they win over
  `relay.toml` until "Reset to relay.toml"); `max_pending_handshakes` applies after a restart.
- **Events**: the last 500 sign-ins, sign-in failures, claims, kicks, bans and admin actions, with IP addresses.
  IP addresses appear only here (and in debug-level logs), never in the normal logs.

Security:
- The password is stored only as an Argon2id hash, in `relay.toml`.
- A sign-in gives a session cookie (HttpOnly, Secure, SameSite=Strict) for 12 hours (`session_hours`).
- Every change needs the session's CSRF token, and requests from other sites are refused.
- Sign-in itself needs a one-time token from the page.
- Five wrong passwords lock that address out for 15 minutes; 30 from all addresses together lock everyone out
  for 5 minutes.
- `allow_ips` can limit who may connect at all.
- Every admin action is logged with a timestamp.
- The page is served with a strict Content-Security-Policy. Mojang and Microsoft tokens never reach the
  relay's storage, so nothing can show them.

## Relay sessions and the on/off switch

- A session lasts `session_minutes` (default 120; 0 = no limit). At each of `session_warn_minutes` (default 10 and
  1 minutes before the end) the host's launcher shows a banner with **Continue** in the server's Console tab, a
  toast, flashes in the taskbar, and tells the players in chat once. **Continue**, or typing `continue` in the
  console, gives another full session (it can't be stacked in advance).
- Without Continue the tunnel and its player connections close when the time runs out. For 15 minutes afterwards
  the Minecraft server list says "The host's relay session ended", and the launcher offers **Start again**.
- **Paused**/**Off**: launchers on Automatic move on to the next way of sharing (bore.pub); a server set to the relay
  by hand waits and reconnects by itself when the relay is on again. The server list shows the admin's message.
- **Launcher 0.3.0** speaks the older protocol: it works, but its hosts get no warnings or Continue, so their
  sessions just end. Set `session_minutes = 0` until your players are on 0.3.1 or later.

## Day to day

### Health and metrics

```bash
curl -s localhost:7001/healthz      # {"ok":true,"tunnels":3,…}
curl -s localhost:7001/metrics      # Prometheus text: tunnels, players, bytes, sign-ins, refusals
```

`resincraft-relay health` does the same check for container health checks.

### Bearer-token admin API (scripts)

With `[admin] tokens` set, on the local admin port only:

```bash
T=<admin token>
curl -s -H "Authorization: Bearer $T" localhost:7001/admin/tunnels
curl -s -H "Authorization: Bearer $T" -d '{"uuid":"8667ba71-b85a-4004-af54-457a9734eed7","reason":"spam"}' localhost:7001/admin/ban
curl -s -H "Authorization: Bearer $T" -d '{"ip":"203.0.113.0/24"}' localhost:7001/admin/ban
curl -s -H "Authorization: Bearer $T" -d '{"name":"some-name"}' localhost:7001/admin/release
```

### Logs

To stdout (systemd's journal, `docker compose logs`) and, with `log_file`, to daily files. `log_format = "json"`
for collectors; `RUST_LOG=debug` for more detail. At the default info level the logs contain names, Minecraft
account names and counts, never player IP addresses or tokens.

### Restarts and upgrades

On SIGTERM (`systemctl restart`, `docker compose restart`) or a Windows service stop, the relay stops
accepting, tells every launcher to reconnect in a few seconds, gives connected players `drain_secs` (default 10)
to keep playing, saves its data and exits. Launchers reconnect by themselves and keep their names.

Upgrading: build or download the new binary, stop the relay, replace it, start it (Windows: run
`install-relay.ps1` again; Docker: `docker compose up -d --build`). Launchers and relays speak protocol version 1
(`proto::PROTOCOL_VERSION`); a relay that bumps it turns older launchers away with "update ResinCraft
Launcher", so update the relay and ship the launcher together.

### Backups

Everything lives in `relay.toml` and the `data_file` (reservations, hashed session tokens, bans, this month's
traffic, limits changed in the dashboard). The data file is written atomically every 30 seconds when something
changed. To restore, stop the relay, put the files back and start it. Losing the data file frees every name and
signs every launcher out once (they sign in again by themselves).

## Players' own domains

A host can use their own domain on top of `<name>.rscraft.resincore.tech`:

- **CNAME** `play.example.com` → `<name>.rscraft.resincore.tech` (DNS only). The relay sees `play.example.com` in
  the handshake, looks up its CNAME and routes it (`[custom_domains]`, cached for 5 minutes).
- **SRV** `_minecraft._tcp.example.com` → `0 5 25565 <name>.rscraft.resincore.tech`, for a domain without a
  subdomain, where a CNAME isn't allowed. Minecraft then sends the SRV target in the handshake.

The launcher shows the right records in the server's Settings and can create them through Cloudflare.

## Notes

- Pre-1.7 clients get a short "update Minecraft" reply to their server list ping.
- Players reach the host's server from 127.0.0.1, so IP bans on the Minecraft server itself don't work; ban IPs
  in the relay's dashboard instead. Each player stream starts with the player's address in its header line,
  for PROXY-protocol support in the launcher later.
- The relay doesn't decrypt anything: after the handshake it only copies bytes, and Minecraft's own encryption
  (online mode) runs end to end between player and server.
