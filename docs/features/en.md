# ResinCraft Launcher: features

A modern, friendly Minecraft launcher by **Puirkle**, for Windows, macOS and Linux. No telemetry and no ResinCraft account.

> NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

The version in brackets is the one each feature arrived in. The newest are 1.4.0 and 1.4.1.

## Playing

- **Instances (0.1.0):** separate installs per version or modpack, each with its own mods, worlds, Java, memory and JVM arguments.
- **Any Minecraft version (0.1.0):** every release and snapshot, back to the classic alphas.
- **Mod loaders (0.1.0):** Fabric, Quilt, Forge and NeoForge, with a loader version picker. Classic Forge for 1.7.10 to 1.12.2 (1.1.2), Forge back to Minecraft 1.2.5 and LiteLoader (1.2.0). OptiFine (1.0.1) and jar mods (1.2.0) too.
- **Change version and loader (1.2.7):** switch an instance's Minecraft version or mod loader from its Settings tab, as in Prism. The version chip on the instance page opens it.
- **Play (0.1.0):** one click. **Play as** another account, or run the same instance twice with two accounts (1.0.5).
- **Quick join (1.1.0):** your recent servers, and singleplayer worlds (1.2.3), one click to jump straight in.
- **Worlds (0.1.0):** rename a world, back it up, import a world from a zip. Play a world directly (1.2.0).
- **Automatic world backups (1.4.0):** while a game or server runs, its worlds are backed up every 30 minutes, keeping the newest 3 per world (Settings → Auto backup).
- **Restore a backup (1.4.0):** put a world back from any backup, your own or automatic, or bring it back as a copy. The world it replaces goes to the Recycle Bin.
- **Crash fixer (1.2.7):** when Minecraft crashes, ResinCraft reads the crash report and logs. It tells you what went wrong in plain words and offers a one-click fix. Fixes: install a missing mod, turn off a mod that crashed, choose the right Java, give the game more memory.
- **Reinstall (1.3.0):** download an instance's game files again, or put a modpack's own files back as the pack has them. From the instance's menu or **Settings → Repair**. Worlds stay.
- **Tuned Java flags (1.2.7):** games start with ResinCraft's G1GC flags for smoother frames (Settings → Java, or per instance).
- **Java (0.1.0):** finds installed Javas, picks the right one per Minecraft version, and downloads Eclipse Temurin when none fits.
- **RAM display (1.2.1):** how much memory the launcher, each game and each server use (optionally on Home).
- **Discord Rich Presence (1.2.3):** pack name, version, play time, singleplayer or server. Buttons for friends (1.2.6): **View modpack** and **Get ResinCraft**.

## Instances

- **Organise:** groups (0.2.0), your own order (0.3.1, drag to arrange), icons and banners (0.3.0), and colours (0.1.0).
- **Tabs (0.1.0):** Content, Worlds, Servers, Screenshots, Notes, Files (built-in file manager and editor, 0.2.0), Logs and Settings.
- **Copy from another instance (1.2.5):** worlds, shader packs, resource packs and game settings; mods and their settings (config folder) since 1.2.6.
- **Duplicate (0.1.0), export** (Modrinth `.mrpack` or CurseForge `.zip`, 1.0.5) and **make it your own modpack (1.1.0)**.
- **Select several (1.1.0):** move, delete, duplicate or update many instances at once.
- **Commands and environment (1.2.0):** commands to run before launch and after the game closes, and environment variables.
- **Instant switches (1.2.7):** turning a mod on or off is instant; the rename happens in the background.
- **Instant delete and create (1.3.0):** a deleted instance disappears at once and goes to the Recycle Bin in the background. A modpack being installed shows in the Library straight away, with its progress on the card.

## Content and Discover

- **Sites (0.1.0):** mods, modpacks, resource packs and shaders from Modrinth, CurseForge, FTB, ATLauncher, Technic and GitHub releases (0.3.1), filtered to the instance's version and loader.
- **Modpack pages (1.2.0)** with description, gallery, versions, changelogs, and **the mods the pack contains (1.2.5)**.
- **Smart installs (0.3.0):** pick any version, install several at once, required dependencies added automatically, the same mod from two sites caught.
- **Updates (0.1.0):** check for and apply updates (one or all); modpack updates and version switching (1.1.0). **Update all** downloads up to four at a time (1.4.1).
- **Manual downloads (1.0.1):** CurseForge files whose authors block launchers are picked up from your Downloads folder or other **watch folders** (1.0.5, Settings → Downloads).
- **Lists (0.4.0):** export the mods, resource packs or shaders as a list, and install a list into another instance.
- **Find servers (1.4.0):** Discover → **Servers** lists servers hosted with ResinCraft and servers players added. Search and filter by category, region, language, modpack, Bedrock or offline-profile support; like and report.
- **A page for each server (1.4.1):** the full description and MOTD, every detail, its client pack, links and a whitelist request box.
- **Join in one click (1.4.0):** **Join** picks the instance that fits, or asks. **Install & join** makes an instance with exactly the server's mods and joins.
- **Ask to be whitelisted (1.4.0):** **Ask to join** sends a request signed with your Minecraft account. It shows **Waiting for the host**, **On the whitelist** or **Ask again**, and ResinCraft tells you (and flashes the taskbar) when the host answers (1.4.1).

## Hosting servers

- **Server types (0.1.0):** Vanilla, Paper, Purpur, Folia, Spigot, Fabric, Quilt, Forge and NeoForge; Leaf, Arclight (mods and plugins together) and Bedrock Dedicated Server since 0.3.1.
- **From a modpack or an instance (0.1.0):** create a server from Modrinth, CurseForge (official server pack when there is one) or FTB, or from one of your instances.
- **No official server pack? (1.2.11):** ResinCraft builds the server from the pack's own mod list, says so when the server is made, and shows a **Built by ResinCraft** badge.
- **Files to download yourself (1.2.9):** files whose authors only allow downloads from their website are listed on the server's **Console** tab. Before each start, ResinCraft looks for them in your Downloads folder and in your instances of the same pack, and puts them in place.
- **Memory by pack size (1.2.9):** a new modded server gets 6 GB from 80 mods and 8 GB from 150 (at most half your PC's RAM).
- **Client-only mods turned off only when they crash (1.2.8):** client mods stay on the server as long as it starts fine. If a start crashes because a mod reached for the game client, that mod is turned off (`.jar.disabled`) and the server starts again by itself. A mod another mod needs stays on.
- **Manage (0.1.0):** console, properties editor, plugin and mod manager with updates (0.3.0), whitelist, ops and bans, server icon (1.2.5), backups.
- **Faster updates (1.4.1):** **Update all** downloads up to four plugins or mods at a time and puts them in together, or queues them for the next restart while the server runs.
- **List your server (1.4.0):** Settings → **Server list** puts it in Discover → Servers with a description, tags, region and language. Servers you run elsewhere can be added with **Submit a server**.
- **Client packs (1.4.0):** publish what players need to join. Since 1.4.1: the server's modpack (installed from its own site), each other mod from Modrinth or CurseForge, and your own files straight from your PC; resource packs and shaders along; plugin and vanilla servers too.
- **Restart and more menu (1.2.7):** Restart next to Stop; a menu with copy address, open folder, back up, settings and delete.
- **Online now (1.2.7):** the players on your server, checked live every few seconds.
- **Java flags (0.2.0):** ResinCraft's tuned flags, or the modpack's own (`user_jvm_args.txt`, start scripts, 1.2.7).
- **Anti-xray (1.3.0):** Paper, Purpur, Folia and Leaf servers can hide ores from x-ray mods and texture packs (Settings → Anti-xray).
- **Anti-cheat (1.4.0):** one switch installs the Grim anti-cheat on plugin servers.
- **Regenerate world (1.4.0):** start a server over with a new world; the old one goes to the Recycle Bin.
- **Reinstall (1.3.0):** install a server's software again, or put its modpack files back. The world and settings stay.
- **Instant delete and create (1.3.0):** a deleted server disappears at once; a server being made shows on the list straight away with its progress.
- **Remote control (1.3.0):** run your servers from any browser or your phone with the web console at `console.rscraft.resincore.tech`, signed in with the same Minecraft account, while ResinCraft is open or in the system tray (Settings → Remote control). It can do everything the server manager does. The file manager, Java settings and plugins from any link need a second switch, **Allow full control**. A **Web console** button on the Servers page (1.3.1).
- **Your other PCs (1.4.1):** the Servers tab's PC menu opens the servers on your other PCs with remote control on: start, stop, manage players, plugins and settings, and make or import servers there.
- **Let friends help (1.4.1):** Settings → Remote control → **People who can use your servers**: give a Minecraft account **Viewer**, **Operator** or **Manager** access, to all your servers or the ones you pick. Files, Java settings, and making or deleting servers stay yours.
- **Sharing with friends:**
  - ResinCraft Relay (`<name>.rscraft.resincore.tech`, 0.3.0)
  - playit.gg (0.2.0)
  - bore (0.2.0)
  - your router (UPnP, 0.2.0)
  - your own domain, with Cloudflare DNS automation (0.2.0), verified with a TXT record (**Pending verification**, **Verified**, 1.3.1)
  - **Automatic** for new servers (1.3.2): your router first, then ResinCraft Relay, then playit.gg
- **Bedrock players can join (0.3.1)** Java servers (Geyser + Floodgate).
- **Whitelist requests (1.2.7):** players ask to join, and you accept or reject them in the Players tab.
  - **Web page:** at your server's own address or your verified domain (1.3.2, with ResinCraft Relay).
  - **Your own Discord bot:** players type `/whitelist`.
  - **The server list (1.4.0):** players with ResinCraft ask from Discover → Servers.
  - **Your answer:** Accept puts them on the whitelist; the bot tells them in a direct message.
  - **Protection:** banned players are refused; **Reject and block** stops someone from asking again, with a **Blocked** list and **Unblock** per player (1.4.1).

## Bedrock Edition (Windows)

- **Bedrock page (0.3.1):** install (1.0.0), update and repair (1.2.0), launch and uninstall (1.2.3) Minecraft for Windows and Minecraft Preview (worlds are backed up first).

## Accounts

- **Microsoft sign-in (0.1.0)**, with tokens kept encrypted. **Offline profiles (0.1.0)** for singleplayer and LAN. **Bedrock profiles (1.0.0)** for Microsoft accounts without Java Edition.
- **Skins, capes and name change (0.1.0).** A skin library, skins from a player name and online search (1.0.3); a 3D skin viewer (1.1.0).

## Moving from another launcher

- **Import (1.0.5):** instances from Prism Launcher, MultiMC, the Modrinth App, the CurseForge app, ATLauncher, GDLauncher and the official Minecraft Launcher, with their mods, worlds, packs and settings. Servers can be imported too.
- **Move to another PC (1.1.1):** Settings → Move to another PC saves everything to one `.resincraft` file; open it on the new PC.

## The app

- **Windows (0.1.0), macOS** (Apple Silicon and Intel) **and Linux** (x64 and ARM64: AppImage, `.deb`, 1.2.0; `.tar.gz`, 1.2.4).
- **Windows installers:** the `.exe` (0.1.0), an `.msi` for managed installs (1.2.10), and **ResinCraft Setup (1.3.0)**: a small installer that always installs the newest version, at a link that never changes.
- **Updates (0.3.0):** checked automatically.
  - An **Update** button in the title bar (1.2.7).
  - **Close them and update**, when games or servers are running (1.2.7).
  - **Update silently** in the background (1.2.7).
- **Keep running in the system tray (1.2.7):** closing the window keeps servers, the Discord bot, remote control and downloads going.
- **Reinstall ResinCraft (1.3.0):** Settings → Updates installs the app again over itself, to repair it. Instances, accounts and settings stay.
- **Themes:** Dark, Light and System (0.1.0); Obsidian, Deepslate, Cherry, Nether and The End (1.3.0).
  - **Create theme (1.3.0):** your own theme, with **Fine-tune** for any colour (colour pickers) and CSS for parts of the app (1.3.2).
  - **Share themes (1.3.2):** **Export** a `.rctheme` file, with its picture and CSS; **Import theme**, or double-click the file (1.4.0).
  - **Custom CSS (1.3.0):** your own CSS over any theme, starting from the current colours, with a built-in guide to the colours, the parts of the app and ready-made examples.
  - And a hidden surprise (1.3.0). Try the Konami code.
- **16 languages (0.1.0):** English, Deutsch, Español, Français, Bahasa Indonesia, Italiano, 日本語, 한국어, Polski, Português (Brasil), Русский, ไทย, Türkçe, Tiếng Việt, 简体中文, 繁體中文.
- **Storage manager (1.1.0):** keep instances on another drive. **Move instances** there, or **Use without moving** (1.4.1).
- **Reset settings** and **Uninstall ResinCraft (1.4.0)** in Settings.
