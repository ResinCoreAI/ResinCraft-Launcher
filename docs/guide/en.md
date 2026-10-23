# ResinCraft Launcher: user guide

How to do the most common things in ResinCraft. Names in **bold** are what you'll see on screen.

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
14. [Troubleshooting](#14-troubleshooting)

## 1. Install and sign in

1. Download ResinCraft for your system from the [Releases page](https://github.com/ResinCoreAI/ResinCraft-Launcher/releases/latest):
   - **Windows:** `…_amd64_windows.exe`
   - **macOS:** `…_universal_macos.dmg`
   - **Linux:** `.AppImage`, `.deb` or `.tar.gz`
2. Install and open it. Windows may say the publisher is unverified; on macOS, right-click the app and choose **Open** the first time.
3. Open **Accounts** → **Sign in with Microsoft**, and sign in on Microsoft's own page. Your password never goes through ResinCraft.
   - No Microsoft account? Add an **Offline profile** for singleplayer and LAN.

## 2. Make your first instance

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

1. Open your instance → **Content** tab → **Add mods**. Or open **Discover** from the left bar.
2. Search, or browse Modrinth, CurseForge, FTB and the other sites. Only versions that fit your instance are shown.
3. Press **Install**. Required mods (dependencies) are added for you.
4. Back in **Content**:
   - use the switch next to a mod to turn it on or off;
   - press **Check for updates** to update mods (one or all).

Some CurseForge authors don't allow launchers to download their files. ResinCraft then lists them under **Download all**: download them in your browser, and ResinCraft finds them in your Downloads folder by itself. Does your browser save somewhere else? Add that folder in **Settings → Downloads → Watch folders**.

## 4. Install a modpack

1. Open **Discover** → **Modpacks** (or **Library** → **New instance** → **Download a modpack**).
2. Open a pack to see its description, its versions and the **Mods** it contains.
3. Press **Install pack**. ResinCraft makes a new instance with everything in it.
4. When the pack gets an update, ResinCraft tells you; you can also pick another pack version on the instance page.

## 5. Change the Minecraft version or mod loader

1. On the instance page, click the version chip at the top (for example **1.21.4 · Fabric 0.16.10**). It opens **Settings → Version**.
2. Choose the **Minecraft** version, the **Mod loader** and the loader version.
3. Press **Change version** and confirm. The next start downloads what's needed.
4. Mods made for the old version may not work: go to **Content** and press **Check for updates**.

## 6. When Minecraft crashes

When the game crashes, a **Minecraft crashed** window opens. ResinCraft reads the crash report and logs and tells you what went wrong, for example:

- *Sodium Extra needs Sodium, which isn't installed* → **Fix** installs Sodium.
- *citresewn crashed the game* → **Fix** turns that mod off (turn it on again in **Content** later).
- *This needs Java 21 or newer* → **Fix** lets ResinCraft pick the right Java.
- *Minecraft ran out of memory* → **Fix** gives it more memory.

Press **Fix** on each one, or **Fix all and play again**. Nothing changes until you press a button. If ResinCraft can't tell why, **Open logs** shows what the game said last.

## 7. Copy things from another instance

1. Open the instance you want to copy *into* → press the **Copy from another instance** button at the top.
2. Choose the instance to copy *from*.
3. Tick what you want:
   - **worlds**;
   - **mods** and **mod settings** (the config folder);
   - **shader packs** and **resource packs**;
   - **game settings** (video, controls, key binds).
4. Press **Copy**. Worlds with a name that's already taken get a new name. Mods and packs you already have are kept as they are.

## 8. Host a server

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
- **Mods** or **Plugins:** add, update or turn off.
- **Properties:** the server settings (difficulty, whitelist on/off, and so on).
- **Settings:**
  - memory, Java, and **Java flags** (ResinCraft's or the modpack's own);
  - how friends connect;
  - the server icon.
- **Restart** and the **⋯** menu (copy address, open folder, back up now, delete).

Mods that only run on the game client (shaders, minimaps…) are turned off on servers automatically. You can see them in the **Mods** tab and turn them back on if you need.

## 9. Let friends join your server

In the server's **Settings** → **How friends connect**:

- **Automatic** (recommended): tries **ResinCraft Relay** first, then playit.gg, then bore.
- **ResinCraft Relay:** friends join `yourname.rscraft.resincore.tech`. It needs a Microsoft account signed in.
- **playit.gg:** a free address that stays the same. The first start asks you to link a playit.gg account (a guest account is fine).
- **Port forwarded myself:** if you set up your router yourself.

The **Console** tab shows the address to give your friends. Bedrock friends can join Java servers too: turn on **Bedrock players can join**.

## 10. Whitelist requests (web page and Discord bot)

Let players ask to join, and accept or reject them yourself.

1. In **Properties**, set **white-list** to **true** (otherwise anyone can join anyway).
2. Open the server's **Players** tab and turn on **Whitelist requests**.
3. Choose how players ask:
   - **Web page:**
     - Works when the server shares through ResinCraft Relay.
     - Press **Copy link** and send players the link: `https://rscraft.resincore.tech/join/<name>`.
     - The page works while the server is online.
   - **Your Discord bot:**
     1. Open the [Discord Developer Portal](https://discord.com/developers/applications) → **New Application** → **Bot** → **Reset Token**, and copy the token.
     2. Paste it in ResinCraft and press **Save**. It's kept sealed on your PC and never shown again.
     3. When it says **Online as …**, press **Add the bot to your Discord** and pick your Discord server.
     4. Players type `/whitelist TheirName` in your Discord.
4. Requests appear in the **Players** tab:
   - **Accept:** puts the player on the whitelist (the bot tells them by direct message).
   - **Reject:** declines the request.
   - **Reject and block:** declines it and stops that person from asking again. **Unblock all** undoes Discord blocks.

Banned players' requests are refused automatically. The Discord bot works while ResinCraft is open; turn on the system tray (section 13) to keep it working when the window is closed.

## 11. Bedrock Edition

On Windows, **Bedrock** on the left installs, opens and uninstalls Minecraft for Windows and Minecraft Preview. Uninstalling backs up your worlds first.

## 12. Move from another launcher

**Library** → **Move from another launcher** brings in instances from:

- Prism Launcher and MultiMC
- the Modrinth App and the CurseForge app
- ATLauncher and GDLauncher
- the official Minecraft Launcher

Their mods, worlds and settings come with them. The originals stay where they are.

## 13. Updates and the system tray

- **Updates:**
  - When a new ResinCraft version is ready, an **Update** button appears in the title bar.
  - If a game or server is running, you choose **Close them and update** (servers save their worlds first) or **Update when they stop**.
- **Update silently** (Settings → Updates): installs new versions in the background, without asking, once nothing is running.
- **Keep running in the system tray** (Settings → Appearance):
  - Closing the window keeps ResinCraft running: servers, the Discord bot and downloads go on.
  - Click the tray icon to open it again; right-click → **Quit ResinCraft** to quit for real.

## 14. Troubleshooting

- **The game won't start:** look at the **Minecraft crashed** window (section 6), or the instance's **Logs** tab.
- **A mod doesn't show in Discover:** it may not exist for your Minecraft version or loader. Try another version, or change the instance's version (section 5).
- **Friends can't join:** check the address on the server's **Console** tab and that the server says **Running**. With ResinCraft Relay, make sure you're signed in with a Microsoft account.
- **The server crashes on start:** if a client-only mod caused it, ResinCraft turns that mod off and starts the server again. Look at the **Console** for the line that says which mod it turned off.
- **Still stuck?** Report a problem on [GitHub](https://github.com/ResinCoreAI/ResinCraft-Launcher/issues) with the log from the **Logs** tab.
