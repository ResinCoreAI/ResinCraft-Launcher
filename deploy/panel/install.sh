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
NoNewPrivileges=true
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
echo "Installed. Two steps left:"
echo "  1. Sign in:  sudo -u resincraft RESINCRAFT_PANEL_DATA=$DATA $BIN login"
echo "  2. Start it: $START"
