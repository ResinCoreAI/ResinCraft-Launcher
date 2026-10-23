# ResinCraft Launcher: features

A modern, friendly Minecraft launcher by **Puirkle**, for Windows, macOS and Linux. No telemetry and no ResinCraft account.

> NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

Items marked **(1.2.7)** are new in version 1.2.7.

## Playing

- **Instances:** separate installs per version or modpack, each with its own mods, worlds, Java, memory and JVM arguments.
- **Any Minecraft version:** every release and snapshot, back to the classic alphas.
- **Mod loaders:** Fabric, Quilt, Forge (back to Minecraft 1.2.5), NeoForge and LiteLoader, with a loader version picker. OptiFine and jar mods too.
- **Change version and loader (1.2.7):** switch an instance's Minecraft version or mod loader from its Settings tab, as in Prism. The version chip on the instance page opens it.
- **Play:** one click. **Play as** another account, or run the same instance twice with two accounts.
- **Quick join:** your recent servers and singleplayer worlds, one click to jump straight in.
- **Worlds:** play a world directly, rename it, back it up, import a world from a zip.
- **Crash fixer (1.2.7):** when Minecraft crashes, ResinCraft reads the crash report and logs. It tells you what went wrong in plain words and offers a one-click fix. Fixes: install a missing mod, turn off a mod that crashed, choose the right Java, give the game more memory.
- **Java:** finds installed Javas, picks the right one per Minecraft version, and downloads Eclipse Temurin when none fits.
- **RAM display:** how much memory the launcher, each game and each server use (optionally on Home).
- **Discord Rich Presence:** pack name, version, play time, singleplayer or server. Buttons for friends: **View modpack** and **Get ResinCraft**.

## Instances

- **Organise:** groups, your own order (drag to arrange), icons, banners and colours.
- **Tabs:** Content, Worlds, Servers, Screenshots, Notes, Files (built-in file manager and editor), Logs and Settings.
- **Copy from another instance:** worlds, mods and their settings (config folder), shader packs, resource packs and game settings.
- **Duplicate, export** (Modrinth `.mrpack` or CurseForge `.zip`) and **make it your own modpack**.
- **Commands and environment:** commands to run before launch and after the game closes, and environment variables.
- **Instant switches (1.2.7):** turning a mod on or off is instant; the rename happens in the background.

## Content and Discover

- **Sites:** mods, modpacks, resource packs and shaders from Modrinth, CurseForge, FTB, ATLauncher, Technic and GitHub releases, filtered to the instance's version and loader.
- **Modpack pages** with description, gallery, versions, changelogs, and **the mods the pack contains**.
- **Smart installs:** pick any version, install several at once, required dependencies added automatically, the same mod from two sites caught.
- **Updates:** check for and apply updates (one or all); modpack updates and version switching.
- **Manual downloads:** CurseForge files whose authors block launchers are picked up from your Downloads folder or other **watch folders** (Settings → Downloads).
- **Lists:** export the mods, resource packs or shaders as a list, and install a list into another instance.

## Hosting servers

- **Server types:** Vanilla, Paper, Purpur, Folia, Spigot, Leaf, Fabric, Quilt, Forge, NeoForge, Arclight (mods and plugins together) and Bedrock Dedicated Server.
- **From a modpack or an instance:** create a server from Modrinth, CurseForge (official server pack when there is one) or FTB, or from one of your instances.
- **Client-only mods turned off only when they crash:** client mods stay on the server as long as it starts fine. If a start crashes because a mod reached for the game client, that mod is turned off (`.jar.disabled`) and the server starts again by itself. A mod another mod needs stays on.
- **Manage:** console, properties editor, plugin and mod manager with updates, whitelist, ops and bans, server icon, backups.
- **Restart and more menu (1.2.7):** Restart next to Stop; a menu with copy address, open folder, back up, settings and delete.
- **Online now:** the players on your server, checked live every few seconds **(1.2.7)**.
- **Java flags (1.2.7):** ResinCraft's tuned flags, or the modpack's own (`user_jvm_args.txt`, start scripts).
- **Sharing with friends:**
  - ResinCraft Relay (`<name>.rscraft.resincore.tech`)
  - playit.gg
  - bore
  - your router (UPnP)
  - your own domain, with Cloudflare DNS automation
- **Bedrock players can join** Java servers (Geyser + Floodgate).
- **Whitelist requests (1.2.7):** players ask to join, and you accept or reject them in the Players tab.
  - **Web page:** `https://rscraft.resincore.tech/join/<name>` (with ResinCraft Relay).
  - **Your own Discord bot:** players type `/whitelist`.
  - **Your answer:** Accept puts them on the whitelist; the bot tells them in a direct message.
  - **Protection:** banned players are refused; **Reject and block** stops someone from asking again.

## Bedrock Edition (Windows)

- Install, launch and uninstall Minecraft for Windows and Minecraft Preview (worlds are backed up first).

## Accounts

- **Microsoft sign-in**, with tokens kept encrypted. **Offline profiles** for singleplayer and LAN.
- **Skins, capes and name change.**

## Moving from another launcher

- Import instances from Prism Launcher, MultiMC, the Modrinth App, the CurseForge app, ATLauncher, GDLauncher and the official Minecraft Launcher, with their mods, worlds, packs and settings. Servers can be imported too.

## The app

- **Windows, macOS** (Apple Silicon and Intel) **and Linux** (x64 and ARM64: AppImage, `.deb`, `.tar.gz`).
- **Updates:** checked automatically.
  - **(1.2.7)** An **Update** button in the title bar.
  - **(1.2.7)** **Close them and update**, when games or servers are running.
  - **(1.2.7)** **Update silently** in the background.
- **Keep running in the system tray (1.2.7):** closing the window keeps servers, the Discord bot and downloads going.
- **16 languages:** English, Deutsch, Español, Français, Bahasa Indonesia, Italiano, 日本語, 한국어, Polski, Português (Brasil), Русский, ไทย, Türkçe, Tiếng Việt, 简体中文, 繁體中文.
- **Storage manager** (move data to another drive), **backup and restore** of settings, **light and dark** themes.
