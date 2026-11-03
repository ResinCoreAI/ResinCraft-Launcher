#!/bin/sh
# Rebuilds lan-offline.jar, which the launcher embeds (friends/offline.rs). Needs a JDK 17 or newer on PATH.
set -e
cd "$(dirname "$0")"
rm -rf out
javac --release 8 -d out src/net/resincraft/lanoffline/Agent.java
jar cfm lan-offline.jar MANIFEST.MF -C out .
rm -rf out
