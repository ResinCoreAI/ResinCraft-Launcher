#!/bin/sh
# Copies the renewed Let's Encrypt certificate to where the relay reads it, owned by the relay's user
# (Let's Encrypt keeps private keys readable by root only). The relay notices the new files within a few
# minutes, without a restart.
#
# Install: sudo install -m 0755 certbot-deploy-hook.sh /etc/letsencrypt/renewal-hooks/deploy/resincraft-relay.sh
# Settings (edit here):
#   DOMAIN  the certificate's name (certbot certonly … -d rscraft.resincore.tech)
#   DEST    the folder relay.toml's [tls] points at
#   OWNER   resincraft-relay (systemd) or 65532 (Docker image's nonroot user)
set -eu
DOMAIN="rscraft.resincore.tech"
DEST="/etc/resincraft-relay/tls"
OWNER="resincraft-relay"

# certbot sets RENEWED_LINEAGE for deploy hooks; skip other certificates.
if [ -n "${RENEWED_LINEAGE:-}" ] && [ "$(basename "$RENEWED_LINEAGE")" != "$DOMAIN" ]; then
    exit 0
fi
SRC="/etc/letsencrypt/live/$DOMAIN"
install -d -m 0750 -o "$OWNER" -g "$OWNER" "$DEST"
install -m 0644 -o "$OWNER" -g "$OWNER" "$SRC/fullchain.pem" "$DEST/fullchain.pem.new"
install -m 0600 -o "$OWNER" -g "$OWNER" "$SRC/privkey.pem" "$DEST/privkey.pem.new"
mv -f "$DEST/privkey.pem.new" "$DEST/privkey.pem"
mv -f "$DEST/fullchain.pem.new" "$DEST/fullchain.pem"
echo "resincraft-relay: certificate for $DOMAIN copied to $DEST"
