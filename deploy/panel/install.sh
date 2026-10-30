#!/bin/sh
# Installs ResinCraft Panel on a Linux machine, with or without a desktop. Run as root, from the folder holding the
# resincraft-panel binary:  sudo sh install.sh
# It adds a resincraft user, keeps its data in /var/lib/resincraft-panel and starts a service (systemd or OpenRC).
set -eu

BIN=/usr/local/bin/resincraft-panel
DATA=/var/lib/resincraft-panel
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

[ "$(id -u)" = 0 ] || { echo "Run this as root (sudo sh install.sh)."; exit 1; }
[ -f "$HERE/resincraft-panel" ] || { echo "Put the resincraft-panel binary next to this script."; exit 1; }

# Java comes with the panel on most systems; tar and curl are for server downloads. Alpine's libc can't run the Java the
# panel downloads, so it gets its own.
if command -v apt-get >/dev/null 2>&1; then
    apt-get update -qq && apt-get install -y -qq tar gzip ca-certificates
elif command -v dnf >/dev/null 2>&1; then
    dnf install -y -q tar gzip ca-certificates
elif command -v yum >/dev/null 2>&1; then
    yum install -y -q tar gzip ca-certificates
elif command -v zypper >/dev/null 2>&1; then
    zypper --non-interactive install tar gzip ca-certificates
elif command -v pacman >/dev/null 2>&1; then
    pacman -Sy --noconfirm --needed tar gzip ca-certificates
elif command -v apk >/dev/null 2>&1; then
    apk add --no-cache tar gzip ca-certificates openjdk21-jre-headless
elif command -v xbps-install >/dev/null 2>&1; then
    xbps-install -Sy tar gzip ca-certificates
elif command -v emerge >/dev/null 2>&1; then
    echo "Gentoo: tar and gzip are part of the base system; skipping packages."
else
    echo "No package manager I know: make sure tar, gzip and CA certificates are installed."
fi

install -m 755 "$HERE/resincraft-panel" "$BIN"

if ! id resincraft >/dev/null 2>&1; then
    if command -v useradd >/dev/null 2>&1; then
        useradd --system --home-dir "$DATA" --shell /usr/sbin/nologin resincraft
    else
        adduser -S -D -h "$DATA" -s /sbin/nologin resincraft
    fi
fi
mkdir -p "$DATA"
chown -R resincraft "$DATA"
# Containers are optional (resincraft-panel containers on). The docker group can run anything as root on this machine.
if getent group docker >/dev/null 2>&1; then
    usermod -aG docker resincraft 2>/dev/null || addgroup resincraft docker
fi

# Podman without Docker: its rootless mode needs newuidmap, which NoNewPrivileges stops, and sub-IDs for the user.
NO_NEW_PRIVILEGES=true
if command -v podman >/dev/null 2>&1 && ! command -v docker >/dev/null 2>&1; then
    NO_NEW_PRIVILEGES=false
    grep -q '^resincraft:' /etc/subuid 2>/dev/null || usermod --add-subuids 200000-265535 --add-subgids 200000-265535 resincraft
fi

if command -v systemctl >/dev/null 2>&1 && [ -d /run/systemd/system ]; then
    cat >/etc/systemd/system/resincraft-panel.service <<EOF
[Unit]
Description=ResinCraft Panel
After=network-online.target
Wants=network-online.target

[Service]
User=resincraft
Environment=RESINCRAFT_PANEL_DATA=$DATA
ExecStart=$BIN run
Restart=on-failure
RestartSec=5
# The servers get their time to save.
TimeoutStopSec=90
# The website's HTTPS port (resincraft-panel web).
AmbientCapabilities=CAP_NET_BIND_SERVICE
NoNewPrivileges=$NO_NEW_PRIVILEGES
ProtectSystem=strict
ReadWritePaths=$DATA
PrivateTmp=true

[Install]
WantedBy=multi-user.target
EOF
    systemctl daemon-reload
    START="systemctl enable --now resincraft-panel"
elif command -v rc-update >/dev/null 2>&1; then
    cat >/etc/init.d/resincraft-panel <<EOF
#!/sbin/openrc-run
description="ResinCraft Panel"
command="$BIN"
command_args="run"
command_user="resincraft"
command_background=true
pidfile="/run/resincraft-panel.pid"
capabilities="^cap_net_bind_service"
export RESINCRAFT_PANEL_DATA="$DATA"
depend() { need net; }
EOF
    chmod 755 /etc/init.d/resincraft-panel
    rc-update add resincraft-panel default
    START="rc-service resincraft-panel start"
else
    START="RESINCRAFT_PANEL_DATA=$DATA $BIN run   (no service manager found: start it your own way)"
fi

echo
echo "Installed. Next:"
echo "  1. Start it: $START"
echo "  2. To use ResinCraft Relay and the launcher's Servers page: sudo -u resincraft RESINCRAFT_PANEL_DATA=$DATA $BIN login"
echo "  3. For a website on your own domain: sudo -u resincraft RESINCRAFT_PANEL_DATA=$DATA $BIN web <domain> <email>"
