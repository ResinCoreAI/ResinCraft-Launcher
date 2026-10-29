# ResinCraft Launcher: user guide

How to do the most common things in ResinCraft. Names in **bold** are what you'll see on screen. Each section says which version it arrived in.

> NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

## Contents

1. [Install and sign in](#1-install-and-sign-in)
2. [Make your first instance](#2-make-your-first-instance)
3. [Add mods, resource packs and shaders](#3-add-mods-resource-packs-and-shaders)
4. [Install a modpack](#4-install-a-modpack)
5. [Change the Minecraft version or mod loader](#5-change-the-minecraft-version-or-mod-loader)
6. [When Minecraft crashes](#6-when-minecraft-crashes)
7. [Copy things from another instance](#7-copy-things-from-another-instance)
8. [Host a server](#8-host-a-server)
9. [Let friends join your server](#9-let-friends-join-your-server)
10. [Whitelist requests (web page and Discord bot)](#10-whitelist-requests-web-page-and-discord-bot)
11. [Bedrock Edition](#11-bedrock-edition)
12. [Move from another launcher](#12-move-from-another-launcher)
13. [Updates and the system tray](#13-updates-and-the-system-tray)
14. [Themes and custom CSS](#14-themes-and-custom-css)
15. [Remote control (web console)](#15-remote-control-web-console)
16. [Reinstall and repair](#16-reinstall-and-repair)
17. [Find and join servers](#17-find-and-join-servers)
18. [List your server and share its mods](#18-list-your-server-and-share-its-mods)
19. [Your other PCs and friends who help](#19-your-other-pcs-and-friends-who-help)
20. [World backups](#20-world-backups)
21. [Storage and moving to another PC](#21-storage-and-moving-to-another-pc)
22. [Troubleshooting](#22-troubleshooting)

## 1. Install and sign in

*Since version 0.1.0.*

1. Download ResinCraft for your system from the [Releases page](https://github.com/ResinCoreAI/ResinCraft-Launcher/releases/latest):
   - **Windows:** [`ResinCraft-Setup.exe`](https://github.com/ResinCoreAI/ResinCraft-Launcher/releases/download/setup/ResinCraft-Setup.exe) (always installs the newest version), or `…_amd64_windows.exe` / `…_amd64_windows.msi`
   - **macOS:** `…_universal_macos.dmg`
   - **Linux:** `.AppImage`, `.deb` or `.tar.gz`
2. Install and open it. Windows may say the publisher is unverified; on macOS, right-click the app and choose **Open** the first time.
3. Open **Accounts** → **Sign in with Microsoft**, and sign in on Microsoft's own page. Your password never goes through ResinCraft.
   - No Microsoft account? Add an **Offline profile** for singleplayer and LAN.

## 2. Make your first instance

*Since version 0.1.0.*

An *instance* is a separate Minecraft install with its own mods, worlds and settings.

1. Open **Library** → **New instance**.
2. Pick a **Minecraft version** and a **loader**:
   - **Vanilla:** no mods.
   - **Fabric** or **Quilt:** light mods, performance mods.
   - **Forge** or **NeoForge:** big content mods.
3. Give it a name and press **Create**.
4. Press **Play**. The first start downloads the game and the right Java; later starts are quick.

Tip: **Quick join** on the left lists your recent servers and worlds; one click jumps straight in.

## 3. Add mods, resource packs and shaders

*Since version 0.1.0.*

1. Open your instance → **Content** tab → **Add mods**. Or open **Discover** from the left bar.
2. Search, or browse Modrinth, CurseForge, FTB and the other sites. Only versions that fit your instance are shown.
3. Press **Install**. Required mods (dependencies) are added for you.
4. Back in **Content**:
   - use the switch next to a mod to turn it on or off;
   - press **Check for updates** to update mods (one or all). **Update all** downloads up to four at a time.

Some CurseForge authors don't allow launchers to download their files. ResinCraft then lists them under **Download all**: download them in your browser, and ResinCraft finds them in your Downloads folder by itself. Does your browser save somewhere else? Add that folder in **Settings → Downloads → Watch folders**.

## 4. Install a modpack

*Since version 0.1.0.*

1. Open **Discover** → **Modpacks** (or **Library** → **New instance** → **Download a modpack**).
2. Open a pack to see its description, its versions and the **Mods** it contains.
3. Press **Install pack**. The new instance shows in the **Library** straight away, with the install's progress on its card; **Play** works once it's done.
4. When the pack gets an update, ResinCraft tells you; you can also pick another pack version on the instance page.

## 5. Change the Minecraft version or mod loader

*Since version 1.2.7.*

1. On the instance page, click the version chip at the top (for example **1.21.4 · Fabric 0.16.10**). It opens **Settings → Version**.
2. Choose the **Minecraft** version, the **Mod loader** and the loader version.
3. Press **Change version** and confirm. The next start downloads what's needed.
4. Mods made for the old version may not work: go to **Content** and press **Check for updates**.

## 6. When Minecraft crashes

*Since version 1.2.7.*

When the game crashes, a **Minecraft crashed** window opens. ResinCraft reads the crash report and logs and tells you what went wrong, for example:

- *Sodium Extra needs Sodium, which isn't installed* → **Fix** installs Sodium.
- *citresewn crashed the game* → **Fix** turns that mod off (turn it on again in **Content** later).
- *This needs Java 21 or newer* → **Fix** lets ResinCraft pick the right Java.
- *Minecraft ran out of memory* → **Fix** gives it more memory.

Press **Fix** on each one, or **Fix all and play again**. Nothing changes until you press a button. If ResinCraft can't tell why, **Open logs** shows what the game said last.

## 7. Copy things from another instance

*Since version 1.2.5.*

1. Open the instance you want to copy *into* → press the **Copy from another instance** button at the top.
2. Choose the instance to copy *from*.
3. Tick what you want:
   - **worlds**;
   - **mods** and **mod settings** (the config folder);
   - **shader packs** and **resource packs**;
   - **game settings** (video, controls, key binds).
4. Press **Copy**. Worlds with a name that's already taken get a new name. Mods and packs you already have are kept as they are.

## 8. Host a server

*Since version 0.1.0.*

1. Open **Servers** → **Create server**.
2. Choose how:
   - **Server software:** Paper, Fabric, Forge, NeoForge, Vanilla, Bedrock…
   - **Modpack:** the same pack your friends play.
   - **From an instance:** the mods you already use.
   - **From a file.**
3. Press **Create**. The window closes and the download continues at the bottom right.
4. Agree to the Minecraft EULA when asked, then press **Start**.

On a server's page:

- **Console:** what the server says; type commands at the bottom.
- **Players:** who's online right now, plus the whitelist, operators and bans.
- **Mods** or **Plugins:** add, update or turn off. **Update all** downloads up to four at a time; while the server runs, the updates go in on the next restart.
- **Properties:** the server settings (difficulty, whitelist on/off, and so on).
- **Settings:**
  - memory, Java, and **Java flags** (ResinCraft's or the modpack's own);
  - how friends connect;
  - the server icon.
- **Restart** and the **⋯** menu (copy address, open folder, back up now, delete).

- **Settings → Anti-xray** (Paper, Purpur, Folia, Leaf): hide ores from x-ray mods and texture packs. **Hide ores** is light on the server; **Fake ores** is stronger. It takes effect on the next start.
- **Settings → Anti-cheat** (plugin servers): one switch installs Grim.

Mods that only run on the game client (shaders, minimaps…) stay on the server as long as it starts fine. If one crashes it, ResinCraft turns that mod off and starts the server again; the **Mods** tab can turn it back on.

When a modpack has no official server pack, ResinCraft builds the server from the pack's own mod list: the window after **Create** says so, and the server shows a **Built by ResinCraft** badge. Some pack files can only be downloaded from their author's website. They're listed at the top of the **Console** tab, with **Get file**. Download them in your browser and ResinCraft puts them in place before the next start, or press **Look again**.

## 9. Let friends join your server

*Since version 0.2.0.*

In the server's **Settings** → **How friends connect**:

- **Automatic** (recommended, and the default for new servers): your router first, then **ResinCraft Relay**, then playit.gg.
- **ResinCraft Relay:** friends join `yourname.rscraft.resincore.tech`. It needs a Microsoft account signed in.
- **playit.gg:** a free address that stays the same. The first start asks you to link a playit.gg account (a guest account is fine).
- **Port forwarded myself:** if you set up your router yourself.

The **Console** tab shows the address to give your friends. Bedrock friends can join Java servers too: turn on **Bedrock players can join**.

**Your own domain:** with ResinCraft Relay, switch on **Map your own domain to ResinCraft Relay** and type the domain. Add the DNS records it shows, including the TXT record that proves the domain is yours, then press **Verify**. The badge goes from **Pending verification** to **Verified**. With Cloudflare auto-DNS on, ResinCraft adds the records for you.

## 10. Whitelist requests (web page and Discord bot)

*Since version 1.2.7.*

Let players ask to join, and accept or reject them yourself.

1. In **Properties**, set **white-list** to **true** (otherwise anyone can join anyway).
2. Open the server's **Players** tab and turn on **Whitelist requests**.
3. Choose how players ask:
   - **Web page:**
     - Works when the server shares through ResinCraft Relay.
     - Press **Copy link** and send players the link. It's your server's own address, or your verified domain.
     - The page works while the server is online.
   - **Your Discord bot:**
     1. Open the [Discord Developer Portal](https://discord.com/developers/applications) → **New Application** → **Bot** → **Reset Token**, and copy the token.
     2. Paste it in ResinCraft and press **Save**. It's kept sealed on your PC and never shown again.
     3. When it says **Online as …**, press **Add the bot to your Discord** and pick your Discord server.
     4. Players type `/whitelist TheirName` in your Discord.
4. Requests appear in the **Players** tab:
   - **Accept:** puts the player on the whitelist (the bot tells them by direct message).
   - **Reject:** declines the request.
   - **Reject and block:** declines it and stops that person from asking again. Blocked players show in the **Blocked** list, with **Unblock** for each.
   - **From the server list:** players with ResinCraft can ask from your server's page in Discover → Servers (section 17).

Banned players' requests are refused automatically. The Discord bot works while ResinCraft is open; turn on the system tray (section 13) to keep it working when the window is closed.

## 11. Bedrock Edition

*Since version 0.3.1.*

On Windows, **Bedrock** on the left installs, opens and uninstalls Minecraft for Windows and Minecraft Preview. Uninstalling backs up your worlds first.

## 12. Move from another launcher

*Since version 1.0.5.*

**Library** → **Move from another launcher** brings in instances from:

- Prism Launcher and MultiMC
- the Modrinth App and the CurseForge app
- ATLauncher and GDLauncher
- the official Minecraft Launcher

Their mods, worlds and settings come with them. The originals stay where they are.

## 13. Updates and the system tray

*Since version 0.3.0 (the system tray since 1.2.7).*

- **Updates:**
  - When a new ResinCraft version is ready, an **Update** button appears in the title bar.
  - If a game or server is running, you choose **Close them and update** (servers save their worlds first) or **Update when they stop**.
- **Update silently** (Settings → Updates): installs new versions in the background, without asking, once nothing is running.
- **Keep running in the system tray** (Settings → Appearance):
  - Closing the window keeps ResinCraft running: servers, the Discord bot and downloads go on.
  - Click the tray icon to open it again; right-click → **Quit ResinCraft** to quit for real.

## 14. Themes and custom CSS

*Since version 1.3.0.*

1. Open **Settings → Appearance → Theme** and pick a card: **System**, Dark, Light, Obsidian, Deepslate, Cherry, Nether or The End.
2. **Custom CSS:** press **Insert current colours**, change any colour, then **Apply**. It works over any theme; **Clear** removes it.
3. Open **How to customize** under the box: it lists the colours and shapes you can change, the parts of the app you can style, and examples (a background picture, rounder corners, bigger text…). Click one to add it to your CSS.

Psst: try the Konami code (↑ ↑ ↓ ↓ ← → ← → B A), or click the ResinCraft logo seven times.

## 15. Remote control (web console)

*Since version 1.3.0.*

Run your servers from any browser, or from your phone.

1. Sign in with a Microsoft account in **Accounts**.
2. Turn on **Settings → Remote control → Allow remote control**. When it says **Connected as …**, you're ready.
3. Turn on **Keep running in the system tray** (section 13), so it keeps working after you close the window.
4. Open `https://console.rscraft.resincore.tech` (or press **Open the web console**) and sign in with the **same Minecraft account**.

From the console you can do everything the server manager does: start, stop and restart servers, watch the console and type commands, manage players and whitelist requests, add and update plugins and mods, change properties and settings, switch the modpack version, reinstall, back up, and make new servers. Each change shows as a note in ResinCraft.

The file manager, Java settings and plugins from any download link can run programs on your PC, so they need a second switch: **Allow full control**. Turn it on only if your Minecraft (Microsoft) account has two-step verification. Without it, the console can never reach anything else on your PC.

## 16. Reinstall and repair

*Since version 1.3.0.*

- **An instance won't start because files are missing or broken:**
  - Right-click it in the **Library** → **Reinstall** → **Game files**, or open its **Settings → Repair**.
  - Minecraft, the mod loader, libraries and assets download again. Worlds, mods and settings stay.
- **A modpack's files were changed or deleted:** **Reinstall** → **Modpack files** puts the pack's mods, configs and scripts back. Files you changed are backed up first; worlds and your own mods stay.
- **A server:** its **⋯** menu → **Reinstall** → **Server software** (or **Modpack files** for a modpack server). The world, settings and player lists stay.
- **ResinCraft itself:** **Settings → Updates → Reinstall ResinCraft** installs the app again over itself.

## 17. Find and join servers

*Since version 1.4.0.*

1. Open **Discover** → **Servers**. It lists servers hosted with ResinCraft and servers players added.
2. Search, or filter by category (SMP, PvP, Minigames, Skyblock…), region, language, modpack, and whether Bedrock players or offline profiles can join.
3. Click a server's icon or name to open its page: the full description, every detail, its client pack and links.
4. Press **Join**. ResinCraft picks the instance that fits the server, or asks which one to use. A server with its own mods has **Install & join**, which makes an instance with exactly those mods and joins.
5. Whitelisted server? Press **Ask to join**. The button shows **Waiting for the host**, then **On the whitelist** (press **Join**) or **Ask again**. ResinCraft tells you, and flashes the taskbar, when the host answers.

Like the servers you enjoy, and report the ones that break the rules.

## 18. List your server and share its mods

*Since version 1.4.0.*

1. In a hosted server's **Settings**, switch on **Server list** (it's off by default).
2. Add a description, tags, region and language. Bedrock and offline-profile support and the modpack fill themselves in.
3. **Client pack:** publish what players need, so they can **Install & join**:
   - the server's modpack, installed from its own site;
   - each other mod, from Modrinth or CurseForge;
   - your own files, sent straight from your PC while ResinCraft runs;
   - resource packs and shaders you choose to send along.
   Plugin and vanilla servers can have one too: pick an instance with the server's Minecraft version.

A server you run somewhere else? Add it with **Submit a server** in Discover → Servers.

## 19. Your other PCs and friends who help

*Since version 1.4.1.*

**Your other PCs:** turn on remote control on each PC (section 15) and sign in with the same Minecraft account. Then, in **Servers**, open the PC menu at the top and pick a PC. You can start, stop and manage its servers from here, and **Create server** or **Import a server** there too. To import from a folder on that PC, type the folder; that needs **Allow full control** on that PC.

**Friends who help run your servers:**

1. Open **Settings → Remote control → People who can use your servers**.
2. Type their Minecraft name and press **Add**.
3. Pick what they can do:
   - **Viewer:** see the console, players and settings.
   - **Operator:** also start, stop and restart, send commands, and handle players and whitelist requests.
   - **Manager:** also change settings, plugins and mods, and the modpack.
4. Keep **All servers**, or pick the servers under **Which servers?**.

They open your servers from their ResinCraft (the PC menu in **Servers**) or the web console, while remote control is on and ResinCraft runs on your PC. Files, Java settings, and making or deleting servers stay yours.

## 20. World backups

*Since version 1.4.0.*

- **Automatic:** while a game or server runs, its worlds are backed up every 30 minutes, keeping the newest 3 per world. Change it or turn it off in **Settings → Auto backup**.
- **Restore:** in an instance's **Worlds** tab, or **Settings → Backups** for a hosted server, press **Restore** on a backup, your own or automatic. The world it replaces goes to the Recycle Bin, so a restore can be undone. A singleplayer world can also come back as a copy.

## 21. Storage and moving to another PC

*Since version 1.1.0 (Move to another PC since 1.1.1).*

- **Another drive:** **Settings → Storage → Change…**, pick a drive or folder, then:
  - **Move instances:** moves your instances, servers and Minecraft's shared files there.
  - **Use without moving:** new things go there; your current instances stay and come back when you switch back.
- **A new PC:** **Settings → Move to another PC** saves everything to one `.resincraft` file. Open it with ResinCraft on the new PC.

## 22. Troubleshooting

- **The game won't start:** look at the **Minecraft crashed** window (section 6), or the instance's **Logs** tab. Files missing or broken? Reinstall them (section 16).
- **A mod doesn't show in Discover:** it may not exist for your Minecraft version or loader. Try another version, or change the instance's version (section 5).
- **Friends can't join:** check the address on the server's **Console** tab and that the server says **Running**. With ResinCraft Relay, make sure you're signed in with a Microsoft account.
- **The server crashes on start:** if a client-only mod caused it, ResinCraft turns that mod off and starts the server again. Look at the **Console** for the line that says which mod it turned off. If it says files are missing, get them from the list at the top of the **Console** tab.
- **Still stuck?** Report a problem on [GitHub](https://github.com/ResinCoreAI/ResinCraft-Launcher/issues) with the log from the **Logs** tab.
