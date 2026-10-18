# Website brief: ResinCraft Launcher

You are building the public website for **ResinCraft Launcher**, a desktop Minecraft: Java Edition launcher made by **Puirkle**. Everything you need is in this file. Where something is unknown it is marked `<<PLACEHOLDER>>`. Leave those visibly marked and list them at the end of your reply. Never invent them.

> **The most important requirement: the site must not look AI-generated.** The owner has said so explicitly. Section 4 is the design direction and section 5 is a list of banned patterns; treat both as hard rules, not suggestions. When a choice isn't covered here, pick the less generic option. Before you finish, run the checklist in section 11.

---

## 1. Why the site exists

1. **Mojang app review.** Mojang must approve the launcher's Azure app before Microsoft sign-in works for everyone. Their form asks for an *"Associated website or domain"*, and reviewers will open this site. It must clearly show:
   - what ResinCraft is and who makes it
   - that players sign in with their **own Microsoft account** on Microsoft's website (device-code flow)
   - that **only people who own Minecraft can play**
   - a **privacy policy**
   - that it is **not an official Minecraft product**
   Review status: `<<MOJANG_REVIEW_STATUS>>`.
2. **Home for the launcher**: download, source code, changelog, FAQ.

### Release facts (real, use exactly)

- **Current version:** `v0.1.0`, released 2026-10-18.
- **Platform:** Windows 10/11, 64-bit. Installer: `ResinCraft.Launcher_0.1.0_x64-setup.exe`, about 5 MB.
- **Download link:** always point to `https://github.com/ResinCoreAI/ResinCraft-Launcher/releases/latest`, never to a versioned file, so the site doesn't need editing on each release.
- **Source code:** `https://github.com/ResinCoreAI/ResinCraft-Launcher`, **MIT licence**, free.
- **Built with:** Tauri 2 and Rust, with a React interface. It's a small native app, not an Electron bundle; you may mention the ~5 MB installer as a plain fact.
- **Windows SmartScreen:** releases aren't code-signed yet, so Windows shows "Windows protected your PC / unknown publisher" on first run. Say so honestly next to the download, with the fix: "More info → Run anyway".
- Never invent download counts, stars or user numbers.

---

## 2. Deliverables

- A **static site**: no backend, no analytics, no tracking, no cookie banner (there are no cookies).
- Pages: `/` (landing), `/privacy`, `/terms`, a `404`. A `/changelog` page is optional (see §7.6).
- Plain HTML + CSS + a few lines of vanilla JS. If you need a build step, use **Astro** with static output; nothing heavier. No Tailwind and no UI kits: their defaults are part of the "AI look".
- Self-host every asset, fonts included (woff2 in `/assets/fonts`). There must be **zero requests to other domains** at runtime.
- Deployable as plain files to GitHub Pages or Cloudflare Pages. Include a `README.md` covering local preview and deployment.
- Dark theme only is fine; the brand is a dark one. If you add a light theme, it has to be designed, not inverted.
- Responsive from 320px up. No horizontal scrolling at any width: put `overflow-x: clip` on `html` and `body`, and use `minmax(0, 1fr)` grid tracks.
- Accessibility: semantic landmarks, one `h1` per page, real alt text, a visible `:focus-visible` ring that appears instantly, contrast of at least 4.5:1 for text, and `prefers-reduced-motion` respected.
- SEO basics: title, description, Open Graph and Twitter tags, canonical (`<<SITE_URL>>`), favicon and touch icon made from the logo, `robots.txt`, `sitemap.xml`.

---

## 3. Brand assets (fixed, use as given)

**Name:** ResinCraft Launcher (short: ResinCraft). **By:** Puirkle.

### Logo (SVG, use as-is)

A purple resin drop holding a green Minecraft-style block. The block's dark right face carries a pixel "R" whose leg ends in a small drop. All flat colours, no gradients. Use it as-is; don't redraw it, recolour it, or add effects. The file is also in the launcher repo at `resources/logo.svg`, and ready-made web files are in `resources/website/assets/`: `logo.svg`, `logo-512.png`, `favicon.ico`, `apple-touch-icon.png`.

Colours used in the mark: drop `#8b46ff`, block top `#4aff86`, block left `#1bc866`, block right `#3a178c`, pixels `#4aff86`. These sit alongside the UI tokens below; they're for the logo only.

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

### Colours

Brand colours are **neon purple, cyan and neon green** on a purple-tinted near-black. Declare them once as CSS custom properties on `:root` and reference only the tokens; no raw hex anywhere else in the CSS.

| Token | Value | Job (one job each) |
|---|---|---|
| `--paper` | `#0a0912` | page background (never pure black) |
| `--paper-2` | `#13111d` | raised surfaces: screenshot frames, code readouts |
| `--paper-3` | `#1a1727` | hover state of raised surfaces |
| `--rule` | `#262238` | 1px lines and borders |
| `--ink` | `#eeecf6` | body text and headings (never pure white) |
| `--ink-dim` | `#a8a3bd` | secondary text |
| `--ink-faint` | `#716b88` | captions, metadata |
| `--purple` | `#b26bff` | **the** accent: links, primary button, focus ring, current nav item |
| `--cyan` | `#22d3ee` | technical data only: version numbers, file names, the sign-in code, monospace labels |
| `--green` | `#39ff88` | status only: "works today" marks and small live/ok dots. Matches the app's Play button. |
| `--on-accent` | `#0b0616` | text on a purple fill |

**How to make it feel neon without looking generated:**
- **Colour lives on small things.** Thin 1–2px lines, text links, a 6px status dot, the cyan monospace code, a single underline under one word in the hero. Large areas stay `--paper`.
- **No glows.** No `box-shadow` halos, no `text-shadow` glows, no blurred colour blobs. A neon sign's colour comes from the tube, not the fog. No exceptions, not even on the logo.
- **Solid fills only.** The primary button is solid `--purple` with `--on-accent` text. It is not a gradient, and it doesn't glow on hover. On hover it gets slightly lighter, or moves down 1px on press.
- **One colour per surface.** A single component never mixes purple and cyan. Cyan means "data", green means "status", purple means "you can click this".

### Typography

Use self-hosted, OFL-licensed faces. **Not Inter, Roboto, Poppins, Montserrat or Space Grotesk**; they read as a template.

- **Display:** **Chakra Petch** (600/700). Its squared-off corners echo blocks without being a pixel font. Alternative: Tomorrow. Headings use sentence case, `letter-spacing: -0.01em`, and are **never italic**.
- **Body:** **IBM Plex Sans** (400/500/600), 17–18px, line-height 1.6, measure about 65 characters.
- **Mono:** **IBM Plex Mono** (400/500) for version numbers, file names, the sign-in code, and small technical labels. Use `font-variant-numeric: tabular-nums` wherever numbers line up.
- No all-caps letter-spaced eyebrows above headings (see §5).

### Shape

- **Corners: 4px.** Blocky and deliberate, not the soft 16–24px "SaaS bubble". Buttons, frames and inputs all use 4px. Pills are not used.
- **Optional signature detail:** screenshot frames get one **notched corner**, the top-right corner cut at 45° (about 10px) with `clip-path`, like a block with a chip taken out. Use it on frames only, consistently, nowhere else.
- **Texture:** an optional faint block-grid of 16px squares with lines at about 4% opacity, used **once**, behind the hero only, fading out downward. Not repeated on other sections.

---

## 4. Design direction

**Page shape: a product tour ("workbench").** The launcher's real screens are the proof. The page is a guided walk through the app with honest captions, rather than a marketing funnel. That fits the product (a tool people use) and avoids the generic shape every generated landing page shares: hero, then three icon cards, then testimonials, then a call-to-action band, then a four-column footer.

**Tone:** practical, a bit nerdy, and personal. It should read like one person who plays modded Minecraft made a launcher they wanted to use. Puirkle is a real maker, so let the voice be first-person in a couple of places.

**Layout principles:**
- **Left-aligned, not centred.** Text columns hug a left margin. Centred text is used at most once (a short closing line).
- **The hero is only as tall as its content.** Not `100vh`.
- **Section spacing varies on purpose:** tight where two things belong together, a long gap before a change of topic. Don't use the same padding everywhere.
- **Asymmetry:** the main screenshot is wider than the text column and runs off the right edge of the content area (clipped by the viewport), so it reads as "here's the real thing" rather than a framed card in a grid.
- **Separate sections with whitespace or a full-width 1px `--rule` line,** not with boxes inside boxes.

**Motion:** close to none.
- At most **one** load-in moment: the hero screenshot fades in over 400ms. Nothing animates on scroll.
- Hover: links thicken their underline; buttons lighten. No scaling, lifting or tilting cards.
- `prefers-reduced-motion`: no motion at all.

---

## 5. Banned patterns (each one reads as "made by AI")

Don't ship any of these:

1. Gradient backgrounds on the hero or any section, including purple-to-blue, purple-to-cyan and "aurora" mesh blobs.
2. **Gradient-filled text** (`background-clip: text`). Headlines are solid `--ink`.
3. Glow effects: coloured `box-shadow` halos on cards or buttons, `text-shadow` neon, blurred orbs or spheres floating behind content.
4. **The icon-card feature grid:** three or six equal cards, each an icon in a coloured square above a heading above two lines of text. Use the screenshot tour and plain typographic lists instead.
5. A full-screen centred hero with one slogan and one big button.
6. An uppercase, letter-spaced, numbered **eyebrow** above every section (`01 · FEATURES`, `WHY RESINCRAFT`). Use no eyebrows at all, except the real numbered sign-in steps.
7. The **generic nav:** logo left, five links in the middle, a pill button on the right, sticky with a blurred background.
8. The **generic footer:** four columns (Product · Company · Resources · Legal), then a row of social icons, then a tiny copyright line.
9. **Glassmorphism** (frosted, blurred panels).
10. **Re-drawn UI chrome:** fake browser bars with traffic-light dots, fake phone frames, fake window title bars around screenshots or code. Use the real screenshot in a `<figure>` with a 1px border.
11. A hand-built HTML/CSS "mock" of the launcher. Use real screenshots (see §8).
12. Emoji used as icons (✨🚀⚡🔥🎯✅), and mixing icon sets. If you need icons, use **Lucide** only (the launcher uses Lucide), inline SVG, 1.5px stroke, at most about 6 on the whole page.
13. Scroll-triggered fade-up on every section, parallax, a cursor-follower dot, auto-rotating carousels.
14. Bouncy or elastic easing. Use only `cubic-bezier(0.2, 0, 0, 1)` for ease-out.
15. Italic words inside headings (`Built to *play*`). Carry emphasis with colour or an underline.
16. **Invented social proof:** testimonials, star ratings, "trusted by 10,000+ players", download counts, "10× faster". There are no users yet; don't pretend.
17. Stock or AI-generated illustrations, 3D renders, characters, and Minecraft-style renders or textures (also a Mojang brand-rule problem).
18. `#000` or `#fff` anywhere.

---

## 6. Copy rules (so the words don't sound generated)

- **Specific beats impressive.** Say what actually happens: "Picks Java 8 for 1.12.2 and Java 21 for 1.21, and downloads it if you don't have it." Not "Seamless Java management."
- **Banned words and phrases:** seamless, effortless, unleash, elevate, supercharge, empower, next-level, game-changer, cutting-edge, robust, revolutionary, all-in-one, "look no further", "whether you're a … or a …", "say goodbye to", "take your … to the next level", "it's not just a launcher, it's …", "in today's world".
- **No slogan triplets** ("Fast. Simple. Powerful.") and no rhetorical question-then-answer headings ("Why ResinCraft? Because…").
- **Few dashes.** Use commas, full stops and colons. At most one em-dash per section.
- Sentence-case headings, ending without a full stop.
- Real typography in rendered text: curly quotes ("…", '…'), an apostrophe ('), a proper ellipsis (…), and a multiplication sign (×) in "1280×720".
- **Admit what's missing.** A short "Not done yet" list makes the page more believable, not less.
- One or two sentences in Puirkle's own voice are encouraged (§7.1, §7.7), for example: "I kept a separate .minecraft folder for every modpack and lost track of which Java went with which. ResinCraft is the launcher I wanted for that." Mark it `<<PUIRKLE_TO_CONFIRM>>` so the owner can rewrite it in their own words.

---

## 7. Landing page (`/`), section by section

### 7.1 Header (edge-aligned, minimal)
Logo plus the "ResinCraft" wordmark on the left. On the right, **three plain text links**: Tour · Sign-in & safety · FAQ, plus a small mono status `v0.1.0` (version in `--cyan`) with a `--green` dot, linking to the latest release. No pill CTA button. The header is not sticky. On mobile, the links collapse to a plain "Menu" text button that opens a simple full-width list.

### 7.2 Hero (left-aligned, content-height)
- **Headline, ≤ 7 words, solid ink, one word underlined in `--purple`.** Options, or write a better plain one:
  - "Every modpack in its own place."
  - "Your Minecraft installs, kept tidy."
  - "Modded Minecraft without the folder mess."
- **One paragraph (2–3 sentences):** "ResinCraft is a free, open-source launcher for Minecraft: Java Edition. Each setup gets its own instance with its own mods, worlds and Java. Find mods and modpacks on Modrinth, CurseForge, FTB, ATLauncher and Technic, pick the version you want, and press Play."
- **Status line** in mono: `v0.1.0 · Windows 10/11 · MIT licence`, with the version in `--cyan`.
- **Actions:** one solid purple **"Download for Windows"** button (links to the latest release, see §1), then a text link "Source on GitHub" and a text link "Take the tour ↓". Under the button, one `--ink-faint` line: "About 5 MB. Windows may say "unknown publisher" the first time: choose More info → Run anyway."
- **Visual:** the real **Home** screenshot (`hero.webp`), large, starting beside or below the text and running off the right edge. This is the one element allowed to fade in on load.

### 7.3 The tour (the core of the page)
Five to seven real screenshots, each paired with a short caption block. **Vary the layout:** one full-width, then two side by side at unequal widths (about 60/40), then one with the text on the right. Each caption is a short heading plus 1–3 sentences, plus optional **callouts**: thin 1px `--rule` lines from the caption to a spot on the screenshot, ending in a small mono label. That makes it read like annotated documentation, not marketing.

| Screen | Heading idea | What to say |
|---|---|---|
| Library | "One instance per setup" | Separate folders for each version or modpack. Sort them into groups by dragging cards between them; colour, duplicate, or send an instance to the Recycle Bin (it's recoverable). |
| Discover | "Five sources, one search" | Modrinth, CurseForge, FTB, ATLauncher and Technic. Search mods, modpacks, resource packs and shaders, filtered to the instance's game version and loader. |
| Version picker | "Pick the exact version" | Install the latest with one click, or open the list and choose a specific release, beta or alpha. Tick several mods to install them together. |
| Instance → Content | "Change your mind later" | Switch any mod to another version, newer or older; toggle it off; check for updates. Dependencies come along automatically. |
| Servers | "Host a server for friends" | Make a Vanilla, Paper, Purpur, Folia, Spigot, Fabric, Quilt, Forge or NeoForge server, turn a modpack or one of your instances into a server, and install plugins from Modrinth, Hangar, SpigotMC and CurseForge. It starts with tuned Java flags, and friends can join over the internet through your router or playit.gg. |
| Accounts | "Your skin, cape and name" | Several Microsoft accounts plus offline profiles. Upload a skin, pick a cape you own, change your Minecraft name, all inside the launcher. |

Instance pages also have Worlds, Screenshots, Servers (the in-game server list) and Notes tabs; mention them in one line under the Content shot rather than giving each a screenshot.

### 7.4 Sign-in and safety (important for Mojang reviewers)
This content is genuinely ordinal, so numbered steps are appropriate here (and only here). Lay them out as a vertical list with large mono numerals in `--ink-faint`:
1. Click **Sign in with Microsoft** in ResinCraft.
2. ResinCraft shows a short code, shown here as a mono readout in `--cyan` on `--paper-2`, e.g. `RC7Q-4KD2` (label it "example code"), and opens Microsoft's sign-in page in your browser.
3. Enter the code and sign in **on Microsoft's site**. Your password never goes into ResinCraft.
4. ResinCraft checks with Microsoft, Xbox and Minecraft services that the account **owns Minecraft: Java Edition**. Then you can play.

Then two short plain paragraphs: sign-in tokens are encrypted on your PC with Windows' built-in data protection (tied to your Windows user) and only sent to Microsoft, Xbox and Minecraft services; and offline profiles (singleplayer, LAN and offline-mode servers only) unlock only after you've added a Microsoft account that owns the game.

### 7.5 What works today / not done yet
Two plain typographic lists side by side (stacked on mobile), no cards. A `--green` dot marks each "works" item; an `--ink-faint` dash marks each "not yet" item.

**Works today (v0.1.0):**
- **Instances:** every Minecraft version (all releases and snapshots, back to the alphas); groups with drag-and-drop; memory, Java and JVM settings per instance; Java found or downloaded automatically (Eclipse Temurin).
- **Loaders:** Fabric, Quilt, Forge and NeoForge.
- **Mods and modpacks:**
  - Discover across Modrinth, CurseForge, FTB, ATLauncher and Technic;
  - choose any mod version, install several at once, switch versions later, with automatic dependencies;
  - update checks for Modrinth and CurseForge mods; mods you dropped in by hand are recognised;
  - files a CurseForge author won't let other launchers download are listed for you to fetch by hand.
- **Import and export:** import Modrinth `.mrpack`, CurseForge, FTB, ATLauncher and Technic modpacks; export an instance as `.mrpack` or a CurseForge `.zip`.
- **Per instance:** Worlds, Screenshots, the in-game server list, and Notes.
- **Accounts:** Microsoft sign-in with encrypted tokens and multiple accounts; offline profiles; skin upload, cape choice and name change.
- **Server hosting:** vanilla, plugin and modded servers; servers made from modpacks or instances; a plugin and mod browser; optimised startup flags; internet access through your router (UPnP), playit.gg, or the bore.pub relay.
- **Game and app:** live game log, crash notice, play time per instance; the interface in 16 languages.

**Not done yet:**
- macOS and Linux builds;
- code-signed releases (hence the SmartScreen warning);
- launcher auto-update;
- a built-in file manager and editor for instances and servers (in progress).

Keep these lists in sync with the release notes; don't add items that aren't in a release.

### 7.6 Changelog teaser (optional, only with real data)
The only real entry so far is `2026-10-18 · v0.1.0 · First public release`. Show it in mono with a link to the GitHub releases page, or to `/changelog` if you build one. **Do not invent other entries or dates.**

### 7.7 FAQ (plain, conversational)
Use `<details>`/`<summary>` and answer like a person, in 1–3 sentences each.
- *Is it free?* Yes. It's free and open source under the MIT licence; the code is on GitHub.
- *Do I need to own Minecraft?* Yes. You sign in with the Microsoft account that owns Minecraft: Java Edition.
- *Is signing in safe?* You sign in on Microsoft's own website with a code. ResinCraft never sees your password, and it stores sign-in tokens encrypted on your computer.
- *Why does Windows warn me when I install it?* The installer isn't code-signed yet, which costs money every year. Choose "More info", then "Run anyway". The source and the build are public on GitHub, so you can check what it does.
- *Where do mods come from?* Modrinth, CurseForge, FTB, ATLauncher and Technic. Downloads are checked against the checksums those sites publish.
- *Why do some CurseForge mods need a manual download?* Some authors only allow downloads from the CurseForge website. ResinCraft lists those files with a link and the folder they go in.
- *Does it run Forge modpacks?* Yes: Forge, NeoForge, Fabric and Quilt all work.
- *Can I host a server for my friends?* Yes. ResinCraft can make and run a server on your PC, including from a modpack. Friends join over your network, or over the internet if your router allows it or through a free playit.gg link.
- *Which systems?* Windows 10 and 11 for now. macOS and Linux later.
- *Which languages?* English plus 15 others, including German, Spanish, French, Japanese, Korean, Portuguese (Brazil), Russian, Thai, Vietnamese and Chinese.
- *Is it made by Mojang or Microsoft?* No. It's an independent project by Puirkle.
- *Is it based on Prism Launcher?* Its feature list is inspired by Prism Launcher, which is open source, but ResinCraft is written from scratch and contains none of Prism's code.

### 7.8 Footer (one statement, then one line)
- A single closing sentence in the display face, left-aligned. For example, "Made by Puirkle for people who keep too many modpacks." `<<PUIRKLE_TO_CONFIRM>>`
- One line of inline links: Privacy · Terms · Contact (`<<CONTACT_EMAIL_OR_URL>>`) · Source (`https://github.com/ResinCoreAI/ResinCraft-Launcher`) · © 2026 Puirkle.
- The mandatory line, verbatim, in `--ink-faint`: **"NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT."**
- No social icon row and no link columns.

---

## 8. Screenshots (the main visual content)

Real screenshots already exist in the launcher repo, in **`resources/website/assets/`**. Copy them into the site's `/assets/`:

| File | Size | Use |
|---|---|---|
| `hero.webp` (+ `hero.png` fallback, `hero@1x.webp`) | 2880×1800 | hero (Home screen) |
| `shots/library.webp` (+ `@1x`) | 2880×1800 | tour: Library |
| `shots/discover.webp` (+ `@1x`) | 2880×1800 | tour: Discover |
| `shots/version-picker.webp` (+ `@1x`) | 2880×1800 | tour: Version picker |
| `shots/content.webp` (+ `@1x`) | 2880×1800 | tour: Instance → Content |
| `shots/home.webp` (+ `@1x`) | 2880×1800 | spare Home shot |

Serve the 2× file through `srcset` with the `@1x` version for normal screens, and set `width`/`height` attributes (16:10) so nothing shifts while loading.

**Still missing:** `shots/servers.webp` and `shots/accounts.webp`. They also predate the Library groups, so `library.webp` should be retaken. Until the real files exist, render each missing slot as a `--paper-2` box with the faint block-grid and the label `screenshot: <name>` in mono `--ink-faint`, at the correct aspect ratio. It should look clearly like a placeholder. Never generate or draw a fake UI.

- Wrap each in `<figure>` with a 1px `--rule` border, 4px radius (plus the optional notched corner), and a `<figcaption>` in `--ink-faint`.
- Use `loading="lazy"` on all except the hero shot, which gets `fetchpriority="high"`.
- How the owner captures more: in the ResinCraft repo, `npm run build:web && npx electron scripts/screenshots.cjs` renders every screen with sample data at 2× into `resources/screenshots/`. Alternatively, `npm run dev:web` serves the same sample-data UI at http://localhost:1420.

---

## 9. Privacy policy (`/privacy`): base text, keep it accurate

Title "Privacy policy". Last updated `<<DATE>>`. Plain language, short paragraphs.

- **Who:** ResinCraft Launcher is made by Puirkle. Contact: `<<CONTACT_EMAIL_OR_URL>>`.
- **This website:** no cookies, analytics or trackers. `<<IF HOSTED ON GITHUB PAGES / CLOUDFLARE: "Our host may keep standard server logs.">>` Downloads are served by GitHub, whose privacy policy applies.
- **The launcher runs on your computer.** Puirkle runs no servers for it and receives none of your data. There's no telemetry.
- **Microsoft sign-in:** you sign in on Microsoft's website with a device code; ResinCraft never sees your password. It receives sign-in tokens from Microsoft, Xbox Live and Minecraft services, keeps them **only on your computer**, encrypted with Windows' data protection, and sends them **only** to those services to sign you in, start the game, and change your skin, cape or name when you ask it to.
- **Stored locally:** your Minecraft username, UUID and skin, so the launcher can show who's signed in. Offline profiles store only the name you type.
- **Services the launcher contacts directly from your computer:**
  - **Sign-in, profile, skins and capes:** Microsoft and Xbox Live; Mojang and Minecraft services, which also serve the game files.
  - **Mods and modpacks:** Modrinth, CurseForge, Feed The Beast, ATLauncher and Technic.
  - **Mod loaders:** Fabric, Quilt, Forge and NeoForge.
  - **Java downloads:** Eclipse Adoptium.
  - **Server software and plugins (server hosting only):** PaperMC (Paper, Folia and Hangar), Purpur, SpigotMC (and its BuildTools sources) and Spiget.
  - Their own privacy policies apply.
- **Hosting a server (only if you use it):**
  - If you make a server public, ResinCraft first asks your router to open its port (UPnP). If that doesn't work, it shares the server through **playit.gg** once you've linked a playit.gg account, and as a last resort through the public **bore.pub** relay. You can also pick one of these by hand, or use your own relay.
  - Players who join see the address you share. Traffic through playit.gg or a relay passes through that service.
  - ResinCraft closes the port again when the server stops.
  - **Your own domain (optional):** if you enter a domain for a server, ResinCraft checks its DNS records by asking public DNS resolvers (Cloudflare 1.1.1.1, or Google 8.8.8.8 as a fallback). These lookups contain only the domain name.
  - **Cloudflare automation (optional, off by default):** if you turn it on and paste a Cloudflare API token, ResinCraft sends that token only to Cloudflare's API, to update your domain's records when your address changes. The token is stored encrypted on your PC.
- **CurseForge:** ResinCraft uses its own CurseForge API key, built into the app; you don't need one.
- **Deleting your data:** remove accounts in the launcher, or delete its data folder (`%APPDATA%\ResinCraft Launcher`). Uninstalling the launcher leaves that folder so your worlds aren't lost; delete it yourself if you want everything gone.
- **Children:** sign-in follows Microsoft's account and family rules.
- **Changes** are posted on this page.

## 10. Terms (`/terms`): short base text
- ResinCraft is provided as is, without warranty, under the MIT licence.
- You need a legitimate copy of Minecraft: Java Edition; ResinCraft doesn't provide or unlock the game. Minecraft is governed by the Minecraft EULA and Microsoft's terms. Servers you host must follow the Minecraft EULA too.
- Mods, modpacks and plugins belong to their authors and come under their own licences.
- Don't use ResinCraft to break Mojang's or Microsoft's rules.
- Contact: `<<CONTACT_EMAIL_OR_URL>>`.
- Include the mandatory disclaimer line from §7.8.

### Minecraft brand rules (apply everywhere)
No Minecraft, Mojang or Microsoft logos, fonts, artwork or official screenshots. Don't make "Minecraft" the biggest word on any page. Writing "for Minecraft: Java Edition" in plain text is fine. Never imply endorsement or partnership.

---

## 11. Before you finish: checklist

**Not-AI checks (all must pass):**
- [ ] No gradients anywhere (the logo is flat too).
- [ ] No glow, blur, glass, orb or blob effects.
- [ ] No icon-card grid; features are told through screenshots and plain lists.
- [ ] No eyebrow labels, except the numbered sign-in steps.
- [ ] Header is the minimal three-link version; footer is a statement plus one line.
- [ ] Copy contains none of the banned words in §6; nothing is invented (no stats, testimonials, download counts or dates).
- [ ] Headings are roman (never italic), solid colour, sentence case.
- [ ] Every colour and font comes from a token; there's no raw hex outside `:root`.
- [ ] Purple only on clickable things, cyan only on technical data, green only on status.
- [ ] At most one load animation; nothing animates on scroll.

**Facts and rules:**
- [ ] The mandatory Mojang disclaimer is on every page, verbatim.
- [ ] The download button points to `/releases/latest` on GitHub, and the SmartScreen note sits next to it.
- [ ] Feature claims match §7.5; the file manager and macOS/Linux are listed as not done yet.
- [ ] Sign-in steps (§7.4) and the privacy policy (§9), including the server-hosting part, are present.

**Technical:**
- [ ] No horizontal scroll at 320, 375, 768, 1280 and 1920px; no button or nav label wraps onto two lines.
- [ ] No runtime requests to other domains (fonts and images are self-hosted; links to GitHub are fine).
- [ ] Lighthouse Accessibility and Best Practices are both 95 or above.
- [ ] Every `<<PLACEHOLDER>>` is still visible and listed in your final reply.
