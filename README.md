# ResinCraft Launcher

A modern, friendly Minecraft: Java Edition launcher by **Puirkle**.

ResinCraft's feature set is modelled on [Prism Launcher](https://github.com/PrismLauncher/PrismLauncher), but it's a from-scratch implementation: no Prism code is used. It's built with Electron, React and TypeScript, and it talks directly to the public Mojang, Microsoft, Fabric, Quilt, Modrinth and Adoptium APIs.

## Features

| | |
|---|---|
| **Instances** | Separate installs per version/modpack, each with its own mods, worlds and settings. Colour themes, duplicate, open folder, delete to Recycle Bin. |
| **Any version** | Every release and snapshot from Mojang's manifest, back to the classic alphas (legacy asset layouts handled). |
| **Mod loaders** | Fabric and Quilt, with loader version picker. Forge/NeoForge are on the roadmap. |
| **Discover** | Search Modrinth for mods, modpacks, resource packs and shaders, filtered to the instance's version and loader. |
| **Smart installs** | Required dependencies are installed automatically; existing copies are replaced on update. |
| **Mod management** | Enable/disable, remove, recognise hand-added jars by hash, check for and apply updates (one or all). |
| **Modpacks** | One-click Modrinth modpack install, plus `.mrpack` import (with zip-slip and download-host checks). |
| **Java** | Auto-detects installed Javas (including the official launcher's runtimes), picks the right major version per game, and downloads Eclipse Temurin when none fits. |
| **Accounts** | Microsoft sign-in via device code (no password ever enters the app), tokens encrypted with OS secure storage, automatic refresh, skin avatars, multiple accounts. Offline profiles are available once a Microsoft account is added. |
| **Play** | Download progress, live colour-coded log console with filter/copy, crash notice, play-time tracking, minimize/hide launcher while playing. |
| **Settings** | Memory slider, JVM args, window size, Java selection, parallel downloads, dark/light/system theme. Instances can override memory, Java and JVM args. |

## Getting started

ResinCraft is moving from Electron to **Tauri 2 + Rust** (branch `tauri`). The React UI is shared: it talks to the engine through `window.resin`, built from one channel map in `src/shared/bridge.ts`, so it runs unchanged on either shell.

### Tauri (new)

Needs Rust (stable, MSVC on Windows) and, on Windows, the Visual Studio C++ Build Tools. WebView2 ships with Windows 10/11.

```bash
npm install
npm run dev:tauri
```

- `npm run dev:tauri` starts the UI on Vite (port 1420) and the Rust engine in `src-tauri/`, with hot reload for the UI.
- `npm run build:tauri` builds a release installer (NSIS) into `src-tauri/target/release/bundle/`.
- Engine layout: `src-tauri/src/core` (context, events/tasks, downloads, paths, settings, instances, IPC router), `game` (versions, loaders, Java, launching), `content` (Modrinth, CurseForge, FTB, ATLauncher, Technic, import/export), `player` (accounts, worlds, servers, screenshots, notes).
- Headless checks: `cargo run --example smoke -- 1.21.1 fabric`, `cargo run --example check_content`, `cargo run --example check_player` (run inside `src-tauri/`).
- ResinCraft Relay (`relay/`, friends join hosted servers as `<name>.rscraft.resincore.tech`): `cargo test` inside `relay/`, and `cargo run --example check_relay` inside `src-tauri/` runs the relay locally with a real vanilla server and a headless Minecraft client. Deployment: [relay/README.md](relay/README.md).

### Electron (current release build)

```bash
npm install
npm run dev
```

- `npm run dev` starts the app with hot reload. In dev mode all game data lives in `.dev-data/` inside the project, so testing never touches a real install. The Tauri build uses the same folder.
- `npm run build` creates a production build in `out/`. `npm start` runs it.
- `npm run dist` packages an installer with electron-builder (output in `dist/`).
- `npm run typecheck` / `npm run format`.
- `npm run smoke -- [version] [vanilla|fabric|quilt]` runs a headless end-to-end check: it creates a throwaway instance, downloads everything, boots Minecraft with an offline profile and stops it once the game reports the user is set.

Opening the Vite URL (http://localhost:5173) in a normal browser shows the UI with a mock backend, which is handy for design work.

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
  shared/types.ts      Types + the IPC contract (ResinApi) shared by every layer
  main/                Electron main process: the launcher engine
    index.ts           Window, security, IPC wiring
    mojang.ts          Version manifest, rules, libraries, natives, assets
    loaders.ts         Fabric / Quilt meta + profile merging
    launch.ts          Prepare → build arguments → spawn Java → stream logs
    java.ts            Java discovery and Temurin downloads
    accounts.ts        Microsoft → Xbox → XSTS → Minecraft auth, offline profiles
    modrinth.ts        Search, installs, dependencies, updates, .mrpack
    net.ts             Verified, retrying, parallel downloads
  preload/index.ts     contextBridge → window.resin
  renderer/src/        React UI (pages/, components/, store.tsx, styles.css)
scripts/smoke.ts       Headless launch test
```

## Roadmap

- Forge and NeoForge (installer processors)
- CurseForge content (needs an API key)
- World, screenshot and server-list management
- Modpack export (`.mrpack`)
- Skin changer

## Updates

Installed copies update themselves with Tauri's updater (`src-tauri/src/updater.rs`, UI in `src/renderer/src/components/Updates.tsx`): a quiet check when ResinCraft starts and every 6 hours, a small "ResinCraft x.y is ready to install" banner (Update now / Release notes / Later), and **Settings → Updates** (check now, last checked, "Install updates automatically", release notes). The update file is the release's NSIS installer itself (`bundle.createUpdaterArtifacts`). It is checked against the public key in `tauri.conf.json` (and must carry the version it was signed for, `requireSignedVersion`, so a tampered latest.json can't pass off an older release as new), then runs in passive mode: it replaces the program files only, so `%APPDATA%\ResinCraft Launcher` (instances, accounts, settings) stays, and the new version starts by itself. If a game or a hosted server is running, ResinCraft offers to wait until they stop.

Where it looks (`plugins.updater.endpoints`): `https://github.com/ResinCoreAI/ResinCraft-Launcher/releases/latest/download/latest.json`, the `latest.json` of the newest published GitHub release. `/releases/latest` skips drafts and pre-releases, so a release reaches players only once its draft is published.

**Signing key.** `updater.local.key` (the private key, password-protected) and `updater.local.env` (its password) sit next to `keys.local.env` and are git-ignored. **Back both up**: installed copies only accept updates signed with this key. CI reads them from the repository secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`; locally, export them before `npm run build:tauri`:

```bash
set -a; . ./updater.local.env; set +a; export TAURI_SIGNING_PRIVATE_KEY="$PWD/updater.local.key"
```

**Releasing an update.**

1. Bump the version in `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`.
2. Push a tag (`git tag v0.3.0 && git push origin v0.3.0`). CI makes a draft release with `ResinCraft.Launcher_<version>_x64-setup.exe`, its `.sig` and `latest.json`.
3. Publish the draft on GitHub. That is the rollout: players get it at their next check (start, or within 6 hours).

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
