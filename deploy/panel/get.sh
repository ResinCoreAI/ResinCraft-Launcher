#!/bin/sh
# Downloads the newest ResinCraft Panel for this machine and installs or updates it (install.sh). Run as root:
#   curl -fsSL https://raw.githubusercontent.com/ResinCoreAI/ResinCraft-Launcher/main/deploy/panel/get.sh | sudo sh
# A version can be chosen with PANEL_VERSION=1.0.0. Releases are tagged panel-v<version> (panel-release.yml).
set -eu

REPO=ResinCoreAI/ResinCraft-Launcher

[ "$(id -u)" = 0 ] || { echo "Run this as root (… | sudo sh)."; exit 1; }
[ "$(uname -s)" = Linux ] || { echo "ResinCraft Panel runs on Linux."; exit 1; }

case "$(uname -m)" in
    x86_64 | amd64) ARCH=amd64 ;;
    aarch64 | arm64) ARCH=arm64 ;;
    *) echo "No ResinCraft Panel build for $(uname -m): there are amd64 and arm64 ones."; exit 1 ;;
esac
# Alpine and other musl systems get the build made for musl.
if ldd --version 2>&1 | grep -qi musl || [ -e /etc/alpine-release ]; then LINUX=linux-musl; else LINUX=linux; fi

fetch() { # <url> <file>
    if command -v curl >/dev/null 2>&1; then curl -fsSL "$1" -o "$2"; else wget -qO "$2" "$1"; fi
}

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

if [ -n "${PANEL_VERSION:-}" ]; then
    TAG="panel-v${PANEL_VERSION#v}"
else
    # The newest panel-v tag among the releases (the repository's "latest" release is the launcher's).
    fetch "https://api.github.com/repos/$REPO/releases?per_page=100" "$TMP/releases.json"
    TAG=$(grep -o '"tag_name": *"panel-v[^"]*"' "$TMP/releases.json" | sed 's/.*"\(panel-v[^"]*\)"/\1/' | sort -V | tail -n 1)
    [ -n "$TAG" ] || { echo "No ResinCraft Panel release found yet."; exit 1; }
fi

NAME="resincraft-panel_${TAG#panel-v}_${ARCH}_${LINUX}"
echo "Downloading $NAME…"
fetch "https://github.com/$REPO/releases/download/$TAG/$NAME.tar.gz" "$TMP/$NAME.tar.gz"
fetch "https://github.com/$REPO/releases/download/$TAG/$NAME.tar.gz.sha256" "$TMP/$NAME.tar.gz.sha256"
(cd "$TMP" && sha256sum -c "$NAME.tar.gz.sha256" >/dev/null) || { echo "The download is damaged (checksum mismatch). Try again."; exit 1; }
tar -C "$TMP" -xzf "$TMP/$NAME.tar.gz"

# Updating: stop the running service first, so the binary can be replaced, and start it again afterwards.
RESTART=
if command -v systemctl >/dev/null 2>&1 && systemctl is-active --quiet resincraft-panel 2>/dev/null; then
    systemctl stop resincraft-panel
    RESTART="systemctl start resincraft-panel"
elif command -v rc-service >/dev/null 2>&1 && rc-service resincraft-panel status >/dev/null 2>&1; then
    rc-service resincraft-panel stop
    RESTART="rc-service resincraft-panel start"
fi

sh "$TMP/$NAME/install.sh"
if [ -n "$RESTART" ]; then
    $RESTART
    echo "Updated to ${TAG#panel-v} and started again."
fi
