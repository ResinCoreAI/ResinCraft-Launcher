# ResinCraft Launcher

A modern, friendly Minecraft: Java Edition launcher by **Puirkle**.

A small native app built with Tauri 2 and Rust, with a React interface. It talks directly to the public Mojang, Microsoft, Fabric, Quilt, Forge, NeoForge, Modrinth, CurseForge and Adoptium APIs. No telemetry and no ResinCraft account.

## Features

| | |
|---|---|
| **Instances** | Separate installs per version or modpack, each with its own mods, worlds, Java, memory and JVM arguments. Groups, your own order (drag to arrange), icons, banners and colours. Duplicate, open folder, delete to the Recycle Bin. |
| **Any version** | Every release and snapshot from Mojang's manifest, back to the classic alphas (legacy asset layouts handled). |
| **Mod loaders** | Fabric, Quilt, Forge and NeoForge, with a loader version picker. |
| **Discover** | Mods, modpacks, resource packs and shaders from Modrinth, CurseForge, FTB, ATLauncher, Technic and GitHub releases, filtered to the instance's version and loader. "All" searches every site at once; paste a link to install it directly. |
| **Smart installs** | Pick any version, install several at once, required dependencies added automatically, duplicate-mod check across sources. |
| **Mod management** | Enable/disable, remove, recognise hand-added jars by hash, check for and apply updates (one or all). |
| **Modpacks** | Import Modrinth `.mrpack`, CurseForge, FTB, ATLauncher and Technic packs; export an instance as `.mrpack` or a CurseForge `.zip`. |
| **Java** | Auto-detects installed Javas (including the official launcher's runtimes), picks the right major version per game, and downloads Eclipse Temurin when none fits. |
| **Accounts** | Microsoft sign-in via device code (no password ever enters the app), tokens encrypted with Windows' data protection, skins, capes and name changes, multiple accounts. Offline profiles for singleplayer and LAN, no Microsoft account needed. |
| **Server hosting** | Vanilla (releases and snapshots), Paper, Purpur, Folia, Spigot, Leaf, Fabric, Quilt, Forge, NeoForge, Arclight (mods and plugins together) and Bedrock Dedicated Server, or a server from a modpack. Console, players, plugin and mod manager with updates (Modrinth, Hangar, SpigotMC, dev.bukkit.org, CurseForge, GitHub, or a link), optimised startup flags, and "Bedrock players can join" (Geyser + Floodgate). Friends join through the router, playit.gg, ResinCraft Relay (`<name>.rscraft.resincore.tech`) or bore.pub, or your own domain. |
| **Bedrock Edition** | Shows the installed Minecraft for Windows and Minecraft Preview with their worlds, opens them, and joins your Bedrock servers. |
| **Files** | A file manager and editor on every instance and server. |
| **Play** | Download progress, live colour-coded log console with filter/copy, crash notice, play-time tracking, minimize/hide launcher while playing. |
| **Updates** | Signed in-app updates from GitHub releases. |
| **Languages** | English and 15 more, including the engine's error messages. |

## Getting started

Needs Node.js 24, Rust (stable, MSVC on Windows) and, on Windows, the Visual Studio C++ Build Tools. WebView2 ships with Windows 10/11.

```bash
npm install
npm run dev
```

- `npm run dev` starts the UI on Vite (port 1420) and the Rust engine in `src-tauri/`, with hot reload for the UI. Dev builds keep all game data in `.dev-data/` inside the project, so testing never touches a real install.
- `npm run build` builds a release installer (NSIS) into `src-tauri/target/release/bundle/`.
- `npm run dev:web` serves the UI alone at http://localhost:1420 with a mock engine and sample data, which is handy for design work.
- `npm run typecheck` / `npm run format`. Translations: `npx tsx tools/check-i18n.ts` and `npx tsx tools/engine-strings.ts --check`.
- Headless engine checks (inside `src-tauri/`): `cargo run --example smoke -- 1.21.1 fabric` creates a throwaway instance, downloads everything and boots Minecraft with an offline profile; also `check_content`, `check_player` and `check_server`.
- ResinCraft Relay (`relay/`, friends join hosted servers as `<name>.rscraft.resincore.tech`): `cargo test` inside `relay/`, and `cargo run --example check_relay` inside `src-tauri/` runs the relay locally with a real vanilla server and a headless Minecraft client. Deployment: [relay/README.md](relay/README.md).

## Microsoft sign-in setup (required once)

Minecraft only accepts sign-ins from Azure apps that Mojang has approved. ResinCraft doesn't ship anyone else's client ID, so you need your own:

1. In the [Azure portal](https://portal.azure.com) go to **App registrations → New registration**.
   - Supported account types: **Personal Microsoft accounts only**.
   - No redirect URI is needed.
2. Under **Authentication**, set **Allow public client flows** to **Yes**.
3. Copy the **Application (client) ID**.
4. Submit the ID to Mojang for Minecraft API access: <https://aka.ms/mce-reviewappid>. Until it's approved, sign-in fails with a "rejected this app's Client ID" message.
5. Paste the ID in **Settings → Microsoft sign-in**, or set `RESINCRAFT_MS_CLIENT_ID` before launching.

## Project layout

```
src/
  shared/types.ts      Types + the engine API (ResinApi) the UI uses
  shared/bridge.ts     The channel map: every API call and the engine channel it goes to
  renderer/src/        React UI (pages/, components/, i18n/, styles.css)
    tauri-api.ts       Talks to the Rust engine
    mock.ts            Stands in for the engine in a plain browser (npm run dev:web)
src-tauri/             Tauri app and the Rust engine
  src/core             Context, events and tasks, downloads, paths, settings, instances, IPC router
  src/game             Versions, loaders, Java, launching
  src/content          Modrinth, CurseForge, FTB, ATLauncher, Technic, import/export
  src/player           Accounts, worlds, servers list, screenshots, notes
  src/server           Server hosting, tunnels, the relay client
  src/files            File manager and editor
  examples/            Headless checks
relay/                 ResinCraft Relay
site/                  Website (Next.js)
deploy/windows/        Windows Server deploy for the website and the relay
tools/                 Translation checks
```

## Updates

Installed copies update themselves with Tauri's updater (`src-tauri/src/updater.rs`, UI in `src/renderer/src/components/Updates.tsx`): a quiet check when ResinCraft starts and every 6 hours, a small "ResinCraft x.y is ready to install" banner (Update now / Release notes / Later), and **Settings → Updates** (check now, last checked, "Install updates automatically", release notes). The update file is the release's NSIS installer itself (`bundle.createUpdaterArtifacts`). It is checked against the public key in `tauri.conf.json` (and must carry the version it was signed for, `requireSignedVersion`, so a tampered latest.json can't pass off an older release as new), then runs in passive mode: it replaces the program files only, so `%APPDATA%\ResinCraft Launcher` (instances, accounts, settings) stays, and the new version starts by itself. If a game or a hosted server is running, ResinCraft offers to wait until they stop.

Where it looks (`plugins.updater.endpoints`): `https://raw.githubusercontent.com/ResinCoreAI/ResinCraft-Launcher/updates/latest.json`. The `updates` branch holds only that file. It's written by `.github/workflows/publish-update.yml` when a release is published (not a draft, not a pre-release, and only if it's the latest release), so a release reaches players only once its draft is published. The release itself then shows just the installer: the `.sig` is removed (its text is inside `latest.json`), and so is `latest.json`, except on releases up to 0.3.1, because launcher 0.3.0 still looks for it on the newest release.

**Signing key.** `updater.local.key` (the private key, password-protected) and `updater.local.env` (its password) sit next to `keys.local.env` and are git-ignored. **Back both up**: installed copies only accept updates signed with this key. CI reads them from the repository secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`; locally, export them before `npm run build:tauri`:

```bash
set -a; . ./updater.local.env; set +a; export TAURI_SIGNING_PRIVATE_KEY="$PWD/updater.local.key"
```

**Releasing an update.**

1. Bump the version in `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`.
2. Push a tag (`git tag v0.3.1 && git push origin v0.3.1`). CI makes a draft release with `ResinCraft.Launcher_<version>_x64-setup.exe` and `latest.json`.
3. Publish the draft on GitHub. That is the rollout: **Publish update** copies `latest.json` to the `updates` branch, and players get the update at their next check (start, or within 6 hours). If that workflow fails, re-run it from the Actions tab (Run workflow, with the tag).

**Testing an update locally.** Build an older and a newer copy under a separate name, so the test never touches a real install or its data (`RESINCRAFT_DATA_FOLDER` is read at build time):

```bash
RESINCRAFT_DATA_FOLDER="ResinCraft Update Test" npx tauri build --config update-test.json
```

with `update-test.json` setting `productName`, `mainBinaryName`, `identifier`, `version`, and `plugins.updater` = `{ "endpoints": ["http://127.0.0.1:18765/latest.json"], "dangerousInsecureTransportProtocol": true }`. Install the older one, put the newer setup.exe and a `latest.json` (version, url, and the `.sig` text as `signature`) in a folder served on that port, and start the older copy.

## Code signing (Windows "Publisher: unverified")

Windows shows **Publisher: unverified** and SmartScreen warnings for unsigned installers. Fixing it needs a code-signing certificate issued to you (Puirkle); no app setting can do it. Options: SignPath Foundation (free, open-source projects), Certum Open Source Code Signing (individuals), Microsoft Azure Artifact Signing (cheapest commercial, limited regions for individuals), or a standard OV/EV certificate.

Once you have one, add it to `src-tauri/tauri.conf.json` and every `npm run build:tauri` signs automatically:

- Certificate installed in Windows (token or .pfx):
  ```json
  "bundle": { "windows": { "certificateThumbprint": "<SHA1 thumbprint>", "digestAlgorithm": "sha256", "timestampUrl": "http://timestamp.digicert.com" } }
  ```
- Cloud signing (Azure Artifact Signing, SimplySign, …): use `"bundle": { "windows": { "signCommand": "<tool> sign … %1" } }`, where `%1` is the file to sign.

Signed builds show "Puirkle" as the publisher immediately; SmartScreen's warning fades as the signed installer gains download reputation.
