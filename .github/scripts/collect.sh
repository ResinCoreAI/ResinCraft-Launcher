#!/usr/bin/env bash
# Puts one platform's downloads into out/release as ResinCraft.Launcher_<version>_<arch>_<os>.<ext>, with their update
# signatures next to them (release.yml, "Collect the files"). Env: PLATFORM (windows | macos | linux | linux-arm),
# BUNDLE (tauri's bundle folder), TAURI_SIGNING_PRIVATE_KEY[_PASSWORD] (to sign the .deb when the bundler didn't).
set -euo pipefail
shopt -s nullglob
version=$(jq -r .version src-tauri/tauri.conf.json)
case "$PLATFORM" in
  windows) arch=amd64; os=windows ;;
  macos) arch=universal; os=macos ;;
  linux) arch=amd64; os=linux ;;
  linux-arm) arch=arm64; os=linux ;;
  *) echo "::error::Unknown platform $PLATFORM"; exit 1 ;;
esac
stem="ResinCraft.Launcher_${version}_${arch}_${os}"
mkdir -p out/release
# take <file> <extension>: copies it (and its .sig) under the release name; there must be exactly one.
take() {
  local dest="out/release/$stem.$2"
  [ -e "$dest" ] && { echo "::error::More than one .$2 was built."; exit 1; }
  cp "$1" "$dest"
  if [ -f "$1.sig" ]; then cp "$1.sig" "$dest.sig"; fi
}

case "$os" in
  windows)
    for f in "$BUNDLE"/nsis/*-setup.exe; do take "$f" exe; done ;;
  macos)
    for f in "$BUNDLE"/dmg/*.dmg; do take "$f" dmg; done
    for f in "$BUNDLE"/macos/*.app.tar.gz; do take "$f" app.tar.gz; done ;;
  linux)
    for f in "$BUNDLE"/appimage/*.AppImage; do take "$f" AppImage; done
    for f in "$BUNDLE"/deb/*.deb; do take "$f" deb; done
    # The .deb updates installs made from it; sign it if the bundler didn't.
    if [ ! -f "out/release/$stem.deb.sig" ] && [ -n "${TAURI_SIGNING_PRIVATE_KEY:-}" ]; then
      npx tauri signer sign "out/release/$stem.deb" > /dev/null
    fi
    # The .tar.gz: the .deb's files as a plain folder, for distributions without dpkg or for running in place.
    work=$(mktemp -d)
    dpkg-deb -x "out/release/$stem.deb" "$work/root"
    pkg="$work/$stem"
    mkdir -p "$pkg"
    cp -a "$work/root/usr" "$pkg/usr"
    bin=$(ls "$pkg/usr/bin" | head -1)
    [ -n "$bin" ] || { echo "::error::The .deb has no program in usr/bin."; exit 1; }
    ln -s "usr/bin/$bin" "$pkg/resincraft"
    cat > "$pkg/README.txt" <<EOF
ResinCraft Launcher $version for Linux ($arch)

Run it:   ./resincraft
It needs WebKitGTK 4.1 and GTK 3 from your distribution, for example:
  Debian / Ubuntu / Mint:  sudo apt install libwebkit2gtk-4.1-0 libgtk-3-0
  Fedora:                  sudo dnf install webkit2gtk4.1 gtk3
  Arch:                    sudo pacman -S webkit2gtk-4.1 gtk3

This folder doesn't update itself: download the new .tar.gz from the releases page when there's a new version
(the .AppImage and the .deb update in place). Your instances and settings live in your home folder, not here.

NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.
EOF
    tar -C "$work" -czf "out/release/$stem.tar.gz" "$stem"
    rm -rf "$work" ;;
esac

ls -l out/release
n=$(ls out/release | grep -vc '\.sig$' || true)
if [ "$n" -eq 0 ]; then echo "::error::The build made no files to ship."; exit 1; fi
