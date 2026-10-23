# Website brief: ResinCraft Launcher

You are building the public website for **ResinCraft Launcher**, a desktop Minecraft: Java Edition launcher made by **Puirkle** (published by ResinCore). Everything you need is in this file.

- Where something is unknown it is marked `<<PLACEHOLDER>>`. Leave those visibly marked, list them at the end of your reply, and never invent them.
- **Design:** the site must look and feel like Puirkle's other product site, **https://qr.puirkle.net** (PuirkleQR Studio). Open it and study it before you start. Section 4 lists its design system as measured, and section 5 maps its page structure onto ResinCraft.
- **Honesty:** every number, feature and date on the page must be true. Section 6 has the full feature list; release data comes live from GitHub (§7).

---

## 1. Why the site exists

1. **Mojang app review.** Mojang must approve the launcher's Azure app before Microsoft sign-in works for everyone. Their form asks for an *"Associated website or domain"*, and reviewers will open this site. It must clearly show:
   - what ResinCraft is and who makes it;
   - that players sign in with their **own Microsoft account** on Microsoft's website (device-code flow);
   - that **playing online needs a Microsoft account that owns Minecraft** (offline profiles are for singleplayer, LAN and offline-mode servers);
   - a **privacy policy**;
   - that it is **not an official Minecraft product**.
   
   Review status: `<<MOJANG_REVIEW_STATUS>>`.
2. **Home for the launcher:** downloads, what's new, version history, FAQ and links to the source.

---

## 2. Release facts (real; the site reads the live values from GitHub)

| Fact | Value |
|---|---|
| Repository | `https://github.com/ResinCoreAI/ResinCraft-Launcher` (public, **GNU GPLv3**) |
| Latest version when this brief was written | `v0.2.0`, published 2026-10-18 (v0.1.0 came out the same day) |
| Platform | Windows 10/11, 64-bit. macOS and Linux are **not** available yet. |
| Installer | `ResinCraft.Launcher_<version>_amd64_windows.exe`, about 7 MB, installs per user (no admin rights) |
| "Download" link | Resolve the newest installer from the GitHub API (§7). Fallback, and the no-JS `href`: `https://github.com/ResinCoreAI/ResinCraft-Launcher/releases/latest` |
| Built with | Tauri 2 and Rust, with a React interface. It's a small native app. |
| Code signing | Not signed yet, so Windows SmartScreen shows "Windows protected your PC". Say so next to the downloads, with the fix: **More info → Run anyway**. |
| Contact | `puirkle@resincore.ai` (the address already published on qr.puirkle.net) |
| Copyright line | `© 2026 ResinCore · ResinCraft Launcher` (same pattern as the QR site) |

Never invent download counts, stars, ratings, testimonials or user numbers.

---

## 3. Deliverables

- **Static site**, separate from PuirkleQR (it only borrows its look), hosted on ResinCraft's own Windows server with IIS (see `deploy/windows/DEPLOY-WINDOWS.md`). Domain: `<<SITE_DOMAIN>>` (built for `resincraft.resincore.tech`). It's already built in `site/` with Next.js (JSX) as a static export.
- **Pages:** `/` (landing), `/privacy`, `/terms` and a `404`. Optional: `/changelog`, a full-page version history from the GitHub API.
- **A head start already exists in the repo:**
  - `site/index.html` has the full landing-page markup in this structure: the sections, the Lucide icon sprite and the copy.
  - `site/assets/` has the logo, favicons, the Plus Jakarta Sans variable font (latin woff2), IBM Plex Mono, the hero image and all screenshots.
  - `style.css`, `theme.js` and `app.js` are **not written yet**. Build on these files, or replace them if you use a framework.
- Self-host fonts and images. The only runtime request to another domain is the GitHub API call in §7, plus Cloudflare Web Analytics **only if** `<<ANALYTICS: none | Cloudflare Web Analytics>>` says so. If analytics is on, the privacy page must say it.
- Light and dark themes with a toggle in the top bar, like the QR site. Follow the system setting by default, remember the choice in `localStorage` (wrapped in try/catch), and set the theme before first paint so it never flashes.
- **Responsive from 320px up**, with no horizontal scroll at any width (`overflow-x: clip` on `html` and `body`, and `minmax(0, 1fr)` grid tracks). On mobile, the nav collapses into a menu button.
- **Accessibility:**
  - semantic landmarks and one `h1` per page;
  - real alt text;
  - a visible `:focus-visible` ring;
  - text contrast of at least 4.5:1 in both themes;
  - `prefers-reduced-motion` respected (no reveal or hover motion).
- **SEO:**
  - title, description, Open Graph and Twitter tags;
  - a canonical URL (`https://<<SITE_DOMAIN>>/`);
  - favicon and touch icon from the logo;
  - `robots.txt` and `sitemap.xml`.
- A `README.md` with local preview and Cloudflare Pages deployment steps.

---

## 4. Design system: match qr.puirkle.net

These values were measured from qr.puirkle.net. Reuse them as they are, with one deliberate change: ResinCraft swaps the QR site's **pink** third accent for its own **cyan**, and uses its **neon green** for "ok" states. That keeps the two sites recognisably one family while each product keeps its colours.

### 4.1 Tokens

```css
:root { /* light */
  --bg: #f7f6fc; --bg-2: #efedf8; --panel: #fff; --panel-2: #f4f2fb;
  --line: #e2def0; --line-strong: #cfc8e6; --text: #17141f; --muted: #625d76;
  --accent: #7c3aed; --accent-2: #a855f7; --accent-3: #0891b2;          /* QR site: #ec4899 */
  --accent-soft: #7c3aed17; --accent-glow: #7c3aed38; --on-accent: #fff;
  --ok: #16a34a; --warn: #c2790b; --bad: #dc2626; --info: #2563eb;
  --nav-bg: #ffffffb8;
  --shadow-sm: 0 1px 2px #1e10460f, 0 2px 8px #1e10460a;
  --shadow: 0 2px 4px #1e10460a, 0 12px 32px #1e104614;
  --shadow-lg: 0 30px 80px #3c14782e;
  --radius: 16px;
  --grad: linear-gradient(135deg, #7c3aed 0%, #a855f7 50%, #06b6d4 100%);
  --grad-text: linear-gradient(120deg, #6d28d9 0%, #a855f7 45%, #0891b2 100%);
}
/* dark: under @media (prefers-color-scheme: dark) for :root:not([data-theme="light"]), and again for :root[data-theme="dark"] */
  --bg: #0c0a12; --bg-2: #12101a; --panel: #17141f; --panel-2: #1f1b29;
  --line: #2a2536; --line-strong: #3a3349; --text: #f1eff7; --muted: #a39db5;
  --accent: #8b5cf6; --accent-2: #c084fc; --accent-3: #22d3ee;          /* QR site: #f472b6 */
  --accent-soft: #8b5cf624; --accent-glow: #8b5cf659;
  --ok: #39ff88; --warn: #fbbf24; --bad: #f87171; --info: #60a5fa;      /* QR site ok: #4ade80 */
  --nav-bg: #0c0a12b3;
  --shadow-sm: 0 1px 2px #0000004d; --shadow: 0 2px 4px #00000040, 0 16px 40px #00000059; --shadow-lg: 0 40px 100px #0000008c;
  --grad-text: linear-gradient(120deg, #c4b5fd 0%, #c084fc 45%, #67e8f9 100%);
```

### 4.2 Type

- **Font:** **Plus Jakarta Sans** (variable, self-hosted: `site/assets/fonts/plus-jakarta-sans-latin-wght-normal.woff2`). Fallbacks: `system-ui, "Segoe UI", "Noto Sans Thai", sans-serif`. Use **IBM Plex Mono** for file names, versions and commands.
- **Hero `h1`:** `clamp(34px, 5.4vw, 60px)`, weight 800, line-height 1.08. Part of it is set in the gradient text (`--grad-text` with `background-clip: text`), as on the QR site: "QR codes, barcodes and cards – **beautifully simple**".
- **Section `h2`:** about 40px, weight 800, centred, inside a `.sec-head` (max-width 760px, margin-bottom 44px).
- **Kicker** above each section title: 12.5px, weight 800, uppercase, letter-spacing 0.12em, colour `--accent-2` (dark) or `--accent` (light).
- **Body:** 16–17px, colour `--muted` for supporting text.

### 4.3 Components (as on the QR site)

- **Top bar:**
  - sticky, background `--nav-bg` with backdrop blur, 1px `--line` bottom border;
  - on the left, the logo plus "ResinCraft" (bold) and "Launcher";
  - centred text links;
  - on the right, a theme toggle in a round icon button. The QR site also has a language button; add one only if you translate the site.
- **Hero badge:** a pill (radius 999px, `--panel` with a `--line-strong` border) holding:
  - a green status dot with a soft ring;
  - a small gradient-filled version chip (`v0.2.0`);
  - "New update: **<headline>**" in accent colour;
  - an arrow.
  
  It links to What's new. On hover it lifts 1px.
- **Buttons:**
  - **`.btn`:** radius 12px (14px for `.big`), `--panel` background, `--line-strong` border, `--shadow-sm`.
  - **`.btn.primary`:** `--grad` background (size 160% and shifts on hover), white text, `box-shadow: 0 1px 0 #ffffff40 inset, 0 10px 28px var(--accent-glow)`.
  - Icon on the left (Lucide, 18px).
- **Hero layout:**
  - text on the left;
  - on the right, the real app screenshot in a rounded frame with `--shadow-lg`, plus one **floating card** overlapping its lower-left corner: a smaller real screenshot with a green "✓ …" caption chip, like the QR site's "Scan check passed" card;
  - behind it, a faint grid pattern and soft purple glows (`isolation: isolate`).
- **Stats row:**
  - a 4-column panel (radius 20px, `--panel`, 1px `--line`, dividers between cells);
  - each cell has a big gradient number (`clamp(28px, 4vw, 40px)`, weight 800) and a muted label;
  - 2×2 on mobile.
- **Feature cards:** `.card` (radius 16px, padding 24px, `--panel`, `--shadow-sm`) in a 3-column grid. Each card has:
  - a 40px rounded-square icon tile filled with `--grad` and a white Lucide icon;
  - a bold title;
  - 2–3 lines of muted text.
- **Alternating sections:** `.section.alt` gets a slightly different background (`--bg-2`) to separate bands, as the QR site does.
- **Reveal on scroll:** elements with `.reveal` fade and rise about 12px once, using IntersectionObserver. They show immediately under reduced motion or without JS.
- **Footer:** `--bg-2` background with four columns:
  - the brand, a tagline and the email;
  - **Product:** Features, Screenshots, Downloads, FAQ;
  - **Help:** Report a bug (GitHub issues), All releases, GitHub;
  - **More from Puirkle:** PuirkleQR Studio → qr.puirkle.net.
  
  Below them, the copyright line and the mandatory Mojang disclaimer.

### 4.4 Logo (use as-is)

A purple resin drop holding a green Minecraft-style block. The block's dark right face carries a pixel "R" whose leg ends in a small drop. It uses flat colours only; don't redraw it, recolour it or add effects.

- Files: `site/assets/logo.svg`, `logo-512.png`, `favicon.ico` and `apple-touch-icon.png`, also in `resources/website/assets/`.
- Mark colours: drop `#8b46ff`, block top `#4aff86`, block left `#1bc866`, block right `#3a178c`, pixels `#4aff86`.

```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="140 30 744 970">
  <path fill="#8b46ff" d="M512 48C490 150 400 252 268 402C206 472 168 548 168 632A344 344 0 0 0 856 632C856 548 818 472 756 402C624 252 534 150 512 48Z"/>
  <path fill="#4aff86" d="M299 500L520 372L741 500L520 628Z"/>
  <path fill="#1bc866" d="M284 525L506 653L506 907L284 780Z"/>
  <path fill="#3a178c" d="M534 653L755 525L755 780L534 907Z"/>
  <g fill="#4aff86">
    <path d="M550 650L585 630L585 668L550 688Z"/>
    <path d="M550 698L585 678L585 716L550 736Z"/>
    <path d="M550 746L585 726L585 764L550 784Z"/>
    <path d="M550 794L585 774L585 812L550 832Z"/>
    <path d="M550 842L585 822L585 860L550 880Z"/>
    <path d="M593 626L628 606L628 644L593 664Z"/>
    <path d="M636 601L671 581L671 619L636 639Z"/>
    <path d="M678 625L713 605L713 643L678 663Z"/>
    <path d="M593 722L628 702L628 740L593 760Z"/>
    <path d="M636 697L671 677L671 715L636 735Z"/>
    <path d="M636 745L671 725L671 763L636 783Z"/>
    <path d="M679 776C679 765 689 758 701 757C712 757 713 769 708 781C702 793 691 801 684 799C680 797 679 790 679 776Z"/>
  </g>
</svg>
```

---

## 5. Landing page structure (mirrors qr.puirkle.net)

| # | QR site section | ResinCraft section | Notes |
|---|---|---|---|
| 1 | Top bar | Top bar | Links: Features · Screenshots · Servers · Downloads · FAQ · GitHub |
| 2 | Hero + badge + stats | Hero + badge + stats | See §5.1 |
| 3 | Gallery | (skip, or later a gallery of real modpack instances) | Only with real content |
| 4 | Features (`#features`) | Features | 9 cards, §5.2 |
| 5 | Card Maker (`.alt`) | **Server hosting** (`#servers`) | Text and a checklist on the left; a "How friends connect" card on the right (§6.5) |
| 6 | Screenshots (`#screens`) | Screenshots | Tabs: Home · Library · Discover · Version picker · Mods |
| 7 | How it works (`#how`) | How it works | 3 numbered steps, §5.3 |
| 8 | Plans | (skip: ResinCraft is free) | Optionally a small "Free and open source" band |
| 9 | Downloads (`#downloads`) | Downloads | "For your system" box, platform cards, What's new, Previous versions, §7 |
| 10 | FAQ (`#faq`) | FAQ | §5.4 |
| 11 | Final CTA | Final CTA | "Ready to play?" with Download and GitHub buttons |
| 12 | Footer | Footer | §4.3 |

### 5.1 Hero

- **Badge:** `vX.Y.Z · New update: <headline>`, filled from the latest release (§7). Fallback text when there's no data: "See what's new".
- **h1:** "Modded Minecraft, **all in one place**" (the bold part uses the gradient). `<<PUIRKLE_TO_CONFIRM>>` if you want a different line.
- **Lede:** "ResinCraft Launcher gives every version and modpack its own instance, finds mods on Modrinth, CurseForge, FTB, ATLauncher and Technic, and hosts servers for your friends. Free and open source."
- **Buttons:** **Download for Windows** (primary) and **View on GitHub**.
- **Under the buttons:** a Windows icon · "Windows 10 & 11 · free & open source (GPLv3) ·" and an "All downloads" link.
- **Visual:** `assets/hero.webp` (Home screen, 2880×1800, with `hero@1x.webp` for srcset). The floating card is `shots/version-picker@1x.webp` with the chip "✓ Any mod version, any time".
- **Stats** (all true; don't change them unless the app changes): **5** mod & modpack sources · **9** server types · **4** mod loaders · **16** languages.

### 5.2 Feature cards (9, with Lucide icons)

| Icon | Title | Text |
|---|---|---|
| layers | Instances & groups | Every version or modpack gets its own folder, Java and settings. Sort them into groups by dragging cards. |
| search | Five sources, one search | Modrinth, CurseForge, FTB, ATLauncher and Technic: mods, modpacks, resource packs and shaders, filtered to your game version. |
| history | Pick the exact version | Install the newest with one click or choose any release, beta or alpha, and switch later. Dependencies come along on their own. |
| package | Modpacks in and out | Import Modrinth, CurseForge, FTB, ATLauncher and Technic packs. Share yours as a .mrpack or a CurseForge zip. |
| server | Host a server | Vanilla, Paper, Purpur, Folia, Spigot, Fabric, Quilt, Forge or NeoForge, or turn a modpack into a server. Tuned Java flags included. |
| globe | Friends join from anywhere | ResinCraft opens your router's port by itself, or shares the server through playit.gg. Got a domain? Use it. |
| folder-open | Files & editor | Browse any instance or server folder and edit configs with syntax highlighting. Deleted files go to the Recycle Bin. |
| shirt | Skins, capes & names | Several Microsoft accounts and offline profiles. Upload a skin, pick a cape and change your name without leaving the launcher. |
| refresh-cw | Updates itself | **Only once the updater ships (§6.9):** "Checks for new versions and installs them in place in one click. Your instances and settings stay." Until then, use the languages card: "16 languages: English, ไทย, 日本語, 한국어, 中文, Tiếng Việt, Bahasa Indonesia, Deutsch, Français, Español, Italiano, Polski, Português, Русский and Türkçe." |

### 5.3 How it works

1. **Install and sign in:** sign in with your Microsoft account on Microsoft's own page. Your password never goes into ResinCraft.
2. **Make it yours:** create an instance for any version, or install a modpack. Add mods, shaders and resource packs.
3. **Press Play:** the right Java is found or downloaded for you, and game files are checked before every start.

### 5.4 FAQ (use `<details>`, 1–3 sentences each)

- **Is ResinCraft free?** Yes, free and open source under the GNU GPLv3; the code is on GitHub.
- **Do I need to own Minecraft?** To play online, yes: you sign in with the Microsoft account that owns Minecraft: Java Edition. Offline profiles (singleplayer, LAN and offline-mode servers) work without signing in.
- **Is signing in safe?** You sign in on Microsoft's own website with a short code, so ResinCraft never sees your password. Sign-in tokens are stored encrypted on your PC.
- **Which computers does it run on?** Windows 10 and 11 (64-bit) for now. macOS and Linux builds are planned.
- **Windows says "Windows protected your PC".** The installer isn't code-signed yet, which costs money every year. Choose More info, then Run anyway. The source and the build are public on GitHub.
- **How do updates work?** *Once the updater ships:* ResinCraft checks for a new version when it starts and every few hours, then downloads the same installer and updates in place, keeping your instances, worlds and accounts. *Until then:* download the newest installer and run it over the old one; your data stays.
- **Does it run Forge modpacks?** Yes: Forge, NeoForge, Fabric and Quilt all work, for both playing and hosting servers.
- **Can I host a server for friends?** Yes, see §6.5.
- **Where are my files kept?** `%APPDATA%\ResinCraft Launcher`. Uninstalling leaves that folder, so your worlds aren't lost.
- **Is it made by Mojang or Microsoft?** No. ResinCraft is an independent project by Puirkle, written from scratch.

---

## 6. Every feature (source of truth for copy)

Status labels: **[0.2.0]** is in the published v0.2.0. **[next]** is built or being built for the next release; mention it only as "coming soon" until a release that contains it is published, and check the release notes before moving it up.

### 6.1 Instances and Library [0.2.0]
- An instance for every Minecraft version: all releases and snapshots, back to the alphas. Snapshots are optional in Settings.
- Each instance has its own folder, Java, memory, JVM arguments and window size. Instances can be coloured, duplicated, renamed and deleted (deleting goes to the Recycle Bin).
- **Groups:** a "New group" button, drag-and-drop between groups, rename and remove (removing a group never deletes instances), folding, and a group chosen when creating an instance.
- **Home:** "Continue playing", total play time, a "Jump back in" list.
- **Instance tabs:**
  - Content (mods, resource packs, shaders);
  - Worlds (game mode, hardcore and cheats, backups folder);
  - Servers (the in-game server list);
  - Screenshots (viewer, copy image, show in folder);
  - Notes;
  - Logs (live, colour-coded, filterable, copy);
  - Settings;
  - **Files** (§6.6).
- Play time per instance, and a crash notice.

### 6.2 Mods, modpacks and Discover [0.2.0]
- **Sources:** Modrinth, CurseForge, FTB, ATLauncher and Technic; **GitHub releases** for mods `[next]`.
- **All sites at once** `[next]`: an "All" tab searches every site; a project on several sites shows once ("Also on CurseForge"). Pasting a link (Modrinth, CurseForge, GitHub page, or a .jar) installs it directly.
- **Kinds:** mods, modpacks, resource packs and shaders, filtered to the instance's game version and loader.
- **Versions and installing:** install the newest or **pick any version**; tick several to install them together; switch versions later, newer or older; dependencies are added automatically.
- **Mod management:** update checks for Modrinth and CurseForge mods; toggle mods on and off; mods dropped in by hand are recognised; downloads are checked against the sites' published hashes.
- **Lists** `[next]`: export an instance's mods, resource packs or shaders as a list file, and install everything on a list into another instance (the same versions when the game and loader match).
- **Cancel** `[next]`: every download and install can be cancelled from its card; a half-installed modpack is removed.
- **Modpacks:** import Modrinth `.mrpack`, CurseForge, FTB, ATLauncher and Technic packs; **export** an instance as `.mrpack` or a CurseForge `.zip`.
- Files a CurseForge author doesn't allow other launchers to download are listed with a link and the folder they belong in.
- **Mod loaders:** Fabric, Quilt, Forge and NeoForge.

### 6.3 Java and game [0.2.0]
- Picks the right Java for each Minecraft version (for example Java 8 for 1.12.2, Java 21 for 1.21) and downloads Eclipse Temurin when none is found.
- Game files and libraries are verified before each start.
- While playing, the launcher can minimise, hide until the game closes, or stay open.

### 6.4 Accounts [0.2.0]
- Microsoft sign-in with a device code on Microsoft's site; several accounts.
- Tokens are encrypted on the PC with Windows' built-in data protection (DPAPI).
- **Offline profiles** for singleplayer, LAN and offline-mode servers. They don't need a Microsoft account; online servers that check accounts still require a Microsoft sign-in. `[next]` (in v0.2.0 they still needed a Microsoft account first)
- **Skins, capes and names:** upload a skin (classic or slim), choose an owned cape, change the Minecraft name (with an availability check).

### 6.5 Server hosting [0.2.0]
- **Create from:** server software (Vanilla; plugin servers Paper, Purpur, Folia and Spigot, the last built with Spigot's official BuildTools; mod servers Fabric, Quilt, Forge and NeoForge), a modpack (Modrinth, CurseForge, FTB), one of your instances, or a modpack file.
- **More server types** `[next]`: **Snapshot** (vanilla on a test version), **Leaf** (a fast Paper fork), **Arclight** (mods and Bukkit plugins on one server, on Forge, NeoForge or Fabric; separate Plugins and Mods tabs) and **Bedrock** (Mojang's Bedrock Dedicated Server for players on phones, consoles and Windows; any released version).
- **Bedrock players can join** `[next]`: one switch on a Java server installs Geyser and Floodgate, so Bedrock players join without a Java account. They use their own UDP port, opened on the router (or through playit.gg).
- **Server tabs:**
  - Console (live, with commands);
  - Players (online players with Op, Kick and Ban; whitelist, operators, banned players and banned IPs);
  - Plugins or Mods (browser for Modrinth, Hangar, SpigotMC and CurseForge; enable, disable, remove);
  - Properties (server.properties in a friendly form);
  - **Files** (§6.6);
  - Settings.
- **Optimised startup flags** (Aikar's G1GC set), on by default. The full command is visible in Settings.
- **Friends joining over the internet ("How friends connect"):**
  - **Automatic:** first the router (UPnP, which opens the port and closes it again when the server stops), then **playit.gg** if it's linked, and the **bore.pub** relay as a last resort. Without a playit link, the server card suggests linking it. (In v0.1.0 Automatic was router → bore.pub.)
  - **playit.gg:** a free address that never changes, once linked to a free (even guest) playit account. New servers start on playit.gg `[next]`; Automatic and the other ways are a click away in the server's Settings.
  - **Relay:** bore.pub, or the player's own bore relay.
  - **"Port forwarded myself"** mode, for people who set up their router by hand.
- **Your own domain:**
  - enter a domain such as `play.example.com`, and ResinCraft shows the exact DNS records (an A record, plus SRV when the port isn't 25565, or SRV to playit.gg);
  - "Check DNS" against public resolvers, a quiet re-check on every start, and a warning when the records stop matching;
  - optional **Cloudflare auto-update** with an API token, stored encrypted on the PC.
- **Plugin manager like the mod manager** `[next, in progress]`:
  - a version picker per plugin;
  - "Check for updates", per-plugin "Update" and "Update all";
  - source tracking (Modrinth, Hangar, Spiget, CurseForge);
  - multi-select for bulk actions.
- **More places to get plugins and mods** `[next]`: dev.bukkit.org and GitHub releases, an "All" tab that searches every site at once, and installing from a pasted link (Modrinth, CurseForge, dev.bukkit.org, Hangar, SpigotMC or GitHub page, or a .jar).

### 6.5b Bedrock Edition `[next]`
- A **Bedrock** page: the installed Minecraft for Windows and Minecraft Preview (from the Microsoft Store), their versions and worlds, Play, and "Join" for Bedrock servers hosted in ResinCraft.
- Windows only lets the Microsoft Store or the Xbox app install and update Bedrock, so ResinCraft doesn't switch Bedrock versions. Say so plainly if it comes up: Preview is for the next update; an older version is kept by hosting a Bedrock server of that version.

### 6.6 Files and editor [0.2.0]
- A **Files** tab on every instance and server: breadcrumbs, new file and folder, rename (F2), copy and paste, drag to move, drop files in from Explorer, "Show in Explorer".
- A built-in editor (CodeMirror) with syntax highlighting for JSON, YAML, TOML, `.properties`, JS and shell files. Ctrl+S saves and Ctrl+F finds. It asks before discarding unsaved edits and won't overwrite a file another program changed.
- Delete goes to the Recycle Bin. Paths can't leave the instance or server folder (links and junctions included). Files over 2 MB aren't opened for editing.

### 6.7 Interface [0.2.0, with [next] polish]
- 16 languages: English, Thai, Japanese, Korean, Chinese (Simplified and Traditional), Vietnamese, Indonesian, German, French, Spanish, Italian, Polish, Portuguese (Brazil), Russian and Turkish. Error messages from the engine are translated too.
- Dark, light or system theme; the window opens maximised; page transitions; calm loading spinner and progress.
- **[next, in progress]:** right-click menus with quick actions everywhere (instances, mods, worlds, files, servers, plugins, Discover results, accounts) and more playful micro-animations.

### 6.8 Safety and privacy [0.2.0]
- No telemetry, and no ResinCraft account. Puirkle runs no servers for the launcher.
- Everything is stored locally in `%APPDATA%\ResinCraft Launcher`.

### 6.9 Updates [next, in progress]
- **Update check** when the app starts and every 6 hours while it's open, plus a "Check for updates" button in Settings → About (shows the current version and when it last checked).
- **When an update is found:** a small banner "ResinCraft X.Y.Z is ready to install" with *Update now*, *Release notes* and *Later*.
- **The update file is the installer itself:** the same `…_amd64_windows.exe` people download from the website, with a signature file. ResinCraft downloads it and runs it in a quiet "update in place" mode, so there's no reinstall wizard. Instances, worlds, accounts and settings stay, and the launcher reopens on the new version.
- If a game or a hosted server is running, it asks first and can wait until they stop.
- An optional "Install updates automatically" setting, off by default.
- Updates are signed: the launcher refuses an update whose signature doesn't match.

---

## 7. Live release data from GitHub

Fetch once on page load, with no auth:
`GET https://api.github.com/repos/ResinCoreAI/ResinCraft-Launcher/releases?per_page=20` (the first non-draft, non-prerelease entry is the latest).

- **Installer:** the asset whose name ends in `-setup.exe`. Use its `browser_download_url` for every Download button and `size` for the "x.x MB" label. Ignore `.sig` and `latest.json` assets (they belong to the updater).
- **Hero badge and release line:** `tag_name`, `published_at` (format "18 October 2026") and the headline. For the headline, take the first `**bold**` phrase of the first bullet under `## What's new` in `body`; otherwise use "See what's new".
- **What's new card:** the bullets under `## What's new` in `body`. Render a tiny Markdown subset only (`**bold**`, `` `code` ``, links); escape everything else.
- **Previous versions card:** the other releases, showing `tag_name` and date, each linking to its `html_url`.
- **Downloads → Windows card:** every `.exe` and `.msi` asset with its size.
- **Failure** (offline, or rate-limited at 60 requests an hour per IP): keep the static fallbacks. Download goes to `/releases/latest`, and the line reads "The newest version is always on GitHub". Never show a broken or empty state.
- Optionally cache the response in `sessionStorage` for 10 minutes.

---

## 8. Screenshots

These are real, 2880×1800 (16:10) screenshots, each with an `@1x` 1440px version. They're in `site/assets/shots/` and `resources/website/assets/shots/`:

| File | Screen |
|---|---|
| `hero.webp` (+ `hero@1x.webp`, `hero.png`) | Home |
| `shots/home.webp` | Home |
| `shots/library.webp` | Library (taken before groups existed; retake it) |
| `shots/discover.webp` | Discover |
| `shots/version-picker.webp` | Mod version picker |
| `shots/content.webp` | Instance → Content (mods) |

**Still missing:** Servers (with Console), Players, Plugins, the Files tab with the editor, Accounts (skin/cape), the update banner, and a right-click menu. Until they exist, leave those slots out, or use a clearly labelled placeholder box. Never draw a fake UI.

Capture more from the browser preview: `npm run dev:web` opens every screen with sample data at http://localhost:1420. Take them at 2× (1440 × 900 window, device scale 2).

---

## 9. Privacy policy (`/privacy`): keep it accurate

Title "Privacy policy". Last updated `<<DATE>>`. Use plain language and short paragraphs.

- **Who:** ResinCraft Launcher is made by Puirkle and published by ResinCore. Contact: `puirkle@resincore.ai`.
- **This website:** no cookies or trackers. It asks GitHub's API which version is newest. `<<IF ANALYTICS: "We use Cloudflare Web Analytics, which counts visits without cookies or personal data.">>` The host (Cloudflare) may keep standard server logs. Downloads are served by GitHub.
- **The launcher runs on your computer.** Puirkle runs no servers for it and receives none of your data. There's no telemetry.
- **Microsoft sign-in:** you sign in on Microsoft's website with a device code; ResinCraft never sees your password.
  - It receives sign-in tokens from Microsoft, Xbox Live and Minecraft services and keeps them **only on your computer**, encrypted with Windows' data protection.
  - It sends them **only** to those services: to sign you in, start the game, and change your skin, cape or name when you ask.
- **Stored locally:** your Minecraft username, UUID and skin. Offline profiles store only the name you type.
- **Services the launcher contacts from your computer:**
  - **Sign-in, profile, skins and capes:** Microsoft and Xbox Live; Mojang and Minecraft services, which also serve the game files.
  - **Mods and modpacks:** Modrinth, CurseForge, Feed The Beast, ATLauncher and Technic.
  - **Mod loaders:** Fabric, Quilt, Forge and NeoForge.
  - **Java:** Eclipse Adoptium.
  - **Updates:** GitHub, to check for and download new versions. `[next]`
  - **Server hosting only:** PaperMC (Paper, Folia and Hangar), Purpur, SpigotMC (and its BuildTools sources) and Spiget.
  - Each service's own privacy policy applies.
- **Hosting a server (only if you use it):**
  - When you make a server public, ResinCraft asks your router to open its port (UPnP), shares it through **playit.gg** (once you've linked an account), or as a last resort uses the public **bore.pub** relay. You can also pick one by hand or use your own relay.
  - Players who join see the address you share; traffic through playit.gg or a relay passes through that service.
  - The port is closed again when the server stops.
  - **Your own domain (optional):** DNS checks ask public resolvers (Cloudflare 1.1.1.1, or Google 8.8.8.8), and only the domain name is sent.
  - **Cloudflare automation (optional, off by default):** your API token is sent only to Cloudflare's API to update your records, and it's stored encrypted on your PC.
- **CurseForge:** ResinCraft uses its own built-in CurseForge API key.
- **Deleting your data:** remove accounts in the launcher, or delete `%APPDATA%\ResinCraft Launcher`. Uninstalling keeps that folder so your worlds aren't lost.
- **Children:** sign-in follows Microsoft's account and family rules.
- **Changes** are posted on this page.

## 10. Terms (`/terms`)

- Provided as is, without warranty, under the GNU GPLv3.
- You need a legitimate copy of Minecraft: Java Edition. ResinCraft doesn't provide or unlock the game. The Minecraft EULA and Microsoft's terms apply, including to servers you host.
- Mods, modpacks and plugins belong to their authors and come under their own licences.
- Don't use ResinCraft to break Mojang's or Microsoft's rules.
- Contact: `puirkle@resincore.ai`.
- Include the mandatory disclaimer below.

### Minecraft brand rules (everywhere)

- No Minecraft, Mojang or Microsoft logos, fonts, artwork or official screenshots.
- "Minecraft" is never the biggest word on a page. Writing "for Minecraft: Java Edition" in plain text is fine.
- Never imply endorsement.
- Every page carries this line, verbatim: **"NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT."**

---

## 11. Before you finish: checklist

**Looks like qr.puirkle.net:**
- [ ] Same top bar, hero badge, gradient headline part, primary gradient button, stats row, feature cards with gradient icon tiles, kicker labels, alternating section bands, reveal on scroll and footer layout.
- [ ] Tokens from §4.1 in both themes; the theme toggle works and never flashes on load.
- [ ] Plus Jakarta Sans is self-hosted, and headings use weight 800.

**Facts:**
- [ ] Downloads resolve the newest installer from the GitHub API, with the `/releases/latest` fallback when JS is off or the API fails.
- [ ] What's new and Previous versions come from the API, not hard-coded text.
- [ ] Every feature on the page is in §6. **[next]** items appear only as "coming soon" until a published release includes them.
- [ ] No invented numbers, ratings, testimonials or quotes.
- [ ] The SmartScreen note sits next to the downloads.
- [ ] The Mojang disclaimer is on every page, and the privacy policy matches §9.

**Technical:**
- [ ] No horizontal scroll at 320, 375, 768, 1280 and 1920px, and no button or nav label wraps onto two lines.
- [ ] Reduced motion turns off reveal and hover motion.
- [ ] Lighthouse Accessibility and Best Practices both score 95 or above.
- [ ] Every `<<PLACEHOLDER>>` is still visible and listed in your final reply.
