# ResinCraft Panel

ResinCraft Panel hosts Minecraft servers on a Linux machine, with or without a desktop. It runs the same server engine
as ResinCraft Launcher, with no window: it works with no account at all, and signing in with Microsoft lets the
launcher's Servers page and the web console run its servers from anywhere, through ResinCraft Relay.

It runs on amd64 and arm64 (a Raspberry Pi 5, for example), on systemd and OpenRC distributions, Alpine included.

## Install or update

```sh
curl -fsSL https://raw.githubusercontent.com/ResinCoreAI/ResinCraft-Launcher/main/deploy/panel/get.sh | sudo sh
```

The same command updates an installed panel: it stops the service, replaces the program and starts it again. Your
servers and settings stay. `PANEL_VERSION=1.0.0` before `sh` installs that version instead of the newest.

By hand: download the `.tar.gz` for your machine from the
[releases](https://github.com/ResinCoreAI/ResinCraft-Launcher/releases) (the ones named `ResinCraft Panel`):
`amd64` for most servers, `arm64` for ARM ones, `-musl` for Alpine. Unpack it and run `sudo sh install.sh`.

The installer adds a `resincraft` user, keeps everything in `/var/lib/resincraft-panel` and sets up the
`resincraft-panel` service. Start it with `systemctl enable --now resincraft-panel` (or
`rc-service resincraft-panel start` on OpenRC).

## Using it

Commands run as the `resincraft` user, for example `sudo -u resincraft resincraft-panel servers`:

| Command | What it does |
|---|---|
| `servers` | Lists the servers on this machine |
| `start <id>` / `stop <id>` | Starts or stops a server |
| `login` | Signs in with Microsoft, with a code to enter on another device, for ResinCraft Relay and the launcher |
| `accounts` | The accounts signed in here |
| `web <domain> <email>` | A website for the panel on your own domain, with HTTPS from Let's Encrypt (port 443 must be open) |
| `web <domain> --cert <file> --key <file> [--port <port>]` | The same with your own certificate |
| `web password` | A new website password (the old one stops working) |
| `web off` | Closes the website |
| `containers on` / `off` | Runs servers in Docker or Podman from their next start |
| `full-control on` / `off` | Lets you use files and Java settings from afar too (off by default) |
| `call <channel> [json…]` | Any engine request, for scripts |

Servers are made and managed from ResinCraft Launcher's Servers page (once signed in with the same account) or from
the panel's website.

## Remove it

```sh
sudo systemctl disable --now resincraft-panel   # or: rc-service resincraft-panel stop; rc-update del resincraft-panel
sudo rm -f /usr/local/bin/resincraft-panel /etc/systemd/system/resincraft-panel.service /etc/init.d/resincraft-panel
sudo userdel resincraft
```

`/var/lib/resincraft-panel` holds your servers and worlds: delete it too only when you no longer need them.

## Releasing (maintainers)

`.github/workflows/panel-release.yml` builds the four Linux files and publishes them when a panel tag is pushed:

```sh
git tag panel-v1.0.0
git push origin panel-v1.0.0
```

Panel releases are never marked as the repository's latest release, so the launcher's download button and updates
stay on the launcher's own releases.

NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.
