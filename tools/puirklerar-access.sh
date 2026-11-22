#!/usr/bin/env bash
# CI: lets cargo fetch PuirkleRAR (a private repository) with a read-only deploy key from $KEY.
# Cargo fetches git dependencies with the git program (.cargo/config.toml), which then uses this key over SSH for
# that one repository only.
set -euo pipefail
mkdir -p ~/.ssh
printf '%s\n' "$KEY" > ~/.ssh/puirklerar
chmod 600 ~/.ssh/puirklerar
# GitHub's published host key (https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/githubs-ssh-key-fingerprints).
echo 'github.com ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl' >> ~/.ssh/known_hosts
cat >> ~/.ssh/config <<'CFG'
Host puirklerar.github.com
  HostName github.com
  User git
  IdentityFile ~/.ssh/puirklerar
  IdentitiesOnly yes
  HostKeyAlias github.com
CFG
git config --global url."git@puirklerar.github.com:Puirkle/PuirkleRAR".insteadOf "https://github.com/Puirkle/PuirkleRAR"
