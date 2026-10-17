# Website brief: ResinCraft Launcher

You are building the public website for **ResinCraft Launcher**, a desktop Minecraft: Java Edition launcher made by **Puirkle**. Everything you need is in this file. Where something is unknown it is marked `<<PLACEHOLDER>>`. Leave those visibly marked and list them at the end of your reply. Never invent them.

> **The most important requirement: the site must not look AI-generated.** The owner has said so explicitly. Section 4 is the design direction and section 5 is a list of banned patterns; treat both as hard rules, not suggestions. When a choice isn't covered here, pick the less generic option. Before you finish, run the checklist in section 11.

---

## 1. Why the site exists

1. **Mojang app review (urgent).** Mojang must approve the launcher's Azure app before players can sign in. Their form asks for an *"Associated website or domain"*, and reviewers will open this site. It must clearly show:
   - what ResinCraft is and who makes it
   - that players sign in with their **own Microsoft account** on Microsoft's website (device-code flow)
   - that **only people who own Minecraft can play**
   - a **privacy policy**
   - that it is **not an official Minecraft product**
2. **Home for the launcher later**: downloads, changelog, docs.

The launcher **is not publicly downloadable yet**. Say "Not released yet" plainly. Never show a fake download button or claim a release exists.

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

A hexagon (a block seen at an angle) with a purple-to-cyan frame and a green-to-cyan drop of resin inside. **The logo is the only place on the site where a colour gradient is allowed.**

```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">
  <defs>
    <linearGradient id="resin-frame" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#b26bff"/>
      <stop offset="1" stop-color="#22d3ee"/>
    </linearGradient>
    <linearGradient id="resin-drop" x1="0" y1="0" x2="0.6" y2="1">
      <stop offset="0" stop-color="#39ff88"/>
      <stop offset="1" stop-color="#22d3ee"/>
    </linearGradient>
  </defs>
  <path d="M32 4 56 18v28L32 60 8 46V18z" fill="#120c1f" stroke="url(#resin-frame)" stroke-width="3.5" stroke-linejoin="round"/>
  <path d="M32 15c7 9 11 15 11 21a11 11 0 0 1-22 0c0-6 4-12 11-21z" fill="url(#resin-drop)"/>
  <path d="M26.5 37.5a5.5 5.5 0 0 0 5.5 5.5" stroke="#fff" stroke-opacity=".6" stroke-width="3" stroke-linecap="round" fill="none"/>
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
- **No glows.** No `box-shadow` halos, no `text-shadow` glows, no blurred colour blobs. A neon sign's colour comes from the tube, not the fog. The single exception is a faint glow on the logo mark in the header (`filter: drop-shadow(0 0 6px` with purple at about 35% alpha`)`).
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
Logo plus the "ResinCraft" wordmark on the left. On the right, **three plain text links**: Tour · Sign-in & safety · FAQ, plus a small text "Not released yet" status with a `--ink-faint` dot. No pill CTA button. The header is not sticky. On mobile, the links collapse to a plain "Menu" text button that opens a simple full-width list.

### 7.2 Hero (left-aligned, content-height)
- **Headline, ≤ 7 words, solid ink, one word underlined in `--purple`.** Options, or write a better plain one:
  - "Every modpack in its own place."
  - "Your Minecraft installs, kept tidy."
  - "Modded Minecraft without the folder mess."
- **One paragraph (2–3 sentences):** "ResinCraft is a desktop launcher for Minecraft: Java Edition. Each setup gets its own instance with its own mods, worlds and Java. Find mods on Modrinth, CurseForge and FTB, pick the version you want, and press Play."
- **Status line** in mono: `v0.1 · in development · Windows first`, with versions in `--cyan`.
- **Actions:** a text link "Take the tour ↓" and a text link "Read how sign-in works". No primary button, because there's nothing to download yet. When downloads exist, a single solid purple "Download for Windows" button replaces the first link.
- **Visual:** the real **Home** screenshot, large, starting beside or below the text and running off the right edge. This is the one element allowed to fade in on load.

### 7.3 The tour (the core of the page)
Four or five real screenshots, each paired with a short caption block. **Vary the layout:** one full-width, then two side by side at unequal widths (about 60/40), then one with the text on the right. Each caption is a short heading plus 1–3 sentences, plus optional **callouts**: thin 1px `--rule` lines from the caption to a spot on the screenshot, ending in a small mono label. That makes it read like annotated documentation, not marketing.

| Screen | Heading idea | What to say |
|---|---|---|
| Library | "One instance per setup" | Separate folders for each version or modpack; colour each one, duplicate it, or send it to the Recycle Bin (it's recoverable). |
| Discover | "Modrinth, CurseForge and FTB in one search" | Switch source, search mods, modpacks, resource packs and shaders, filtered to the instance's game version and loader. |
| Version picker | "Pick the exact version" | Install the latest with one click, or open the list and choose a specific release, beta or alpha. Tick several mods to install them together. |
| Instance → Content | "Change your mind later" | Switch any mod to another version, newer or older; toggle it off; check for updates. Dependencies come along automatically. |
| Logs | "See what the game is doing" | Live log, colour-coded, filterable, one-click copy for bug reports. |

### 7.4 Sign-in and safety (important for Mojang reviewers)
This content is genuinely ordinal, so numbered steps are appropriate here (and only here). Lay them out as a vertical list with large mono numerals in `--ink-faint`:
1. Click **Sign in with Microsoft** in ResinCraft.
2. ResinCraft shows a short code, shown here as a mono readout in `--cyan` on `--paper-2`, e.g. `RC7Q-4KD2` (label it "example code"), and opens Microsoft's sign-in page in your browser.
3. Enter the code and sign in **on Microsoft's site**. Your password never goes into ResinCraft.
4. ResinCraft checks with Microsoft, Xbox and Minecraft services that the account **owns Minecraft: Java Edition**. Then you can play.

Then two short plain paragraphs: tokens are encrypted with the operating system's secure storage and only sent to Microsoft and Mojang; and offline profiles unlock only after you've added an account that owns the game.

### 7.5 What works today / not done yet
Two plain typographic lists side by side (stacked on mobile), no cards. A `--green` dot marks each "works" item; an `--ink-faint` dash marks each "not yet" item.

**Works today:** instances for every Minecraft version (all releases and snapshots, back to the alphas) · Fabric and Quilt · Discover across Modrinth, CurseForge and FTB · choose any mod version, install several at once, switch versions later · automatic dependencies · update checks for Modrinth and CurseForge mods · recognises mods you dropped in by hand · Modrinth `.mrpack`, CurseForge and FTB modpacks · files a CurseForge author won't let other launchers download are listed for you to fetch by hand · Java found or downloaded automatically (Eclipse Temurin) · Microsoft sign-in with encrypted tokens and multiple accounts · live game log, crash notice, play-time per instance · memory, Java and JVM settings per instance.

**Not done yet:** Forge and NeoForge (in progress; most FTB and many CurseForge packs need these) · public downloads · macOS and Linux builds · world, screenshot and server-list management · modpack export · skin changer · launcher auto-update.

### 7.6 Changelog teaser (optional, only with real data)
If the owner gives you real dated entries, show the latest 2–3 as `date · version · one line` in mono, plus a link to `/changelog`. **Do not invent entries or dates.** If there's no data, leave this section out.

### 7.7 FAQ (plain, conversational)
Use `<details>`/`<summary>` and answer like a person, in 1–3 sentences each.
- *Is it free?* `<<CONFIRM_PRICING>>`
- *Do I need to own Minecraft?* Yes. You sign in with the Microsoft account that owns Minecraft: Java Edition.
- *Is signing in safe?* You sign in on Microsoft's own website with a code; ResinCraft never sees your password, and it stores sign-in tokens encrypted on your computer.
- *Where do mods come from?* Modrinth, CurseForge and FTB. Downloads are checked against the checksums those sites publish.
- *Why do some CurseForge mods need a manual download?* Some authors only allow downloads from the CurseForge website. ResinCraft lists those files with a link and the folder they go in.
- *Does it run Forge modpacks?* Not yet. Forge and NeoForge support is being built; Fabric and Quilt work now.
- *Which systems?* Windows first. macOS and Linux later.
- *Is it made by Mojang or Microsoft?* No. It's an independent project by Puirkle.
- *Is it based on Prism Launcher?* Its feature list is inspired by Prism Launcher, which is open source, but ResinCraft is written from scratch and contains none of Prism's code.

### 7.8 Footer (one statement, then one line)
- A single closing sentence in the display face, left-aligned. For example, "Made by Puirkle for people who keep too many modpacks." `<<PUIRKLE_TO_CONFIRM>>`
- One line of inline links: Privacy · Terms · Contact (`<<CONTACT_EMAIL_OR_URL>>`) · Source (`<<GITHUB_URL_OR_REMOVE>>`) · © `<<YEAR>>` Puirkle.
- The mandatory line, verbatim, in `--ink-faint`: **"NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT."**
- No social icon row and no link columns.

---

## 8. Screenshots (the main visual content)

The site depends on **real** screenshots of the launcher. Expected files, as 2× PNG or WebP at 16:10, dark theme:
`/assets/shots/home.webp`, `library.webp`, `discover.webp`, `version-picker.webp`, `content.webp`, `logs.webp`.

- Wrap each in `<figure>` with a 1px `--rule` border, 4px radius (plus the optional notched corner), and a `<figcaption>` in `--ink-faint`.
- Use `loading="lazy"` on all except the hero shot, which gets `fetchpriority="high"`.
- **Until the real files exist,** render each slot as a `--paper-2` box with the faint block-grid and the label `screenshot: <name>` in mono `--ink-faint`, at the correct aspect ratio. It should look clearly like a placeholder. Never generate or draw a fake UI.
- How the owner captures them: run `npm run dev` in the ResinCraft repo. The app window (or http://localhost:5173 in a browser, which uses sample data) shows every screen.

---

## 9. Privacy policy (`/privacy`): base text, keep it accurate

Title "Privacy policy". Last updated `<<DATE>>`. Plain language, short paragraphs.

- **Who:** ResinCraft Launcher is made by Puirkle. Contact: `<<CONTACT_EMAIL_OR_URL>>`.
- **This website:** no cookies, analytics or trackers. `<<IF HOSTED ON GITHUB PAGES / CLOUDFLARE: "Our host may keep standard server logs.">>`
- **The launcher runs on your computer.** Puirkle runs no servers for it and receives none of your data. There's no telemetry.
- **Microsoft sign-in:** you sign in on Microsoft's website with a device code; ResinCraft never sees your password. It receives sign-in tokens from Microsoft, Xbox Live and Minecraft services, keeps them **only on your computer**, encrypted with your operating system's secure storage, and sends them **only** to those services to sign you in and start the game.
- **Stored locally:** your Minecraft username, UUID and skin address, so the launcher can show who's signed in.
- **Services the launcher contacts directly from your computer:** Microsoft and Xbox Live (sign-in); Mojang and Minecraft services (game files and profile); Modrinth, CurseForge and Feed The Beast (searching and downloading mods and modpacks); the Fabric and Quilt metadata servers (mod loaders); Eclipse Adoptium (Java downloads). Their own privacy policies apply.
- **CurseForge key:** if you add a CurseForge API key in Settings, it's stored on your computer and sent only to CurseForge.
- **Deleting your data:** remove accounts in the launcher, or delete its data folder (Windows: `%APPDATA%\ResinCraft Launcher`).
- **Children:** sign-in follows Microsoft's account and family rules.
- **Changes** are posted on this page.

## 10. Terms (`/terms`): short base text
Provided as is, without warranty. You need a legitimate copy of Minecraft: Java Edition; ResinCraft doesn't provide or unlock the game. Minecraft is governed by the Minecraft EULA and Microsoft's terms. Mods and modpacks belong to their authors and come under their own licences. Don't use ResinCraft to break Mojang's or Microsoft's rules. Contact: `<<CONTACT_EMAIL_OR_URL>>`. Mandatory disclaimer line from §7.8.

### Minecraft brand rules (apply everywhere)
No Minecraft, Mojang or Microsoft logos, fonts, artwork or official screenshots. Don't make "Minecraft" the biggest word on any page. Writing "for Minecraft: Java Edition" in plain text is fine. Never imply endorsement or partnership.

---

## 11. Before you finish: checklist

**Not-AI checks (all must pass):**
- [ ] No gradient anywhere except inside the logo SVG.
- [ ] No glow, blur, glass, orb or blob effects (except the faint glow on the header logo).
- [ ] No icon-card grid; features are told through screenshots and plain lists.
- [ ] No eyebrow labels, except the numbered sign-in steps.
- [ ] Header is the minimal three-link version; footer is a statement plus one line.
- [ ] Copy contains none of the banned words in §6; nothing is invented (no stats, testimonials or dates).
- [ ] Headings are roman (never italic), solid colour, sentence case.
- [ ] Every colour and font comes from a token; there's no raw hex outside `:root`.
- [ ] Purple only on clickable things, cyan only on technical data, green only on status.
- [ ] At most one load animation; nothing animates on scroll.

**Facts and rules:**
- [ ] The mandatory Mojang disclaimer is on every page, verbatim.
- [ ] No fake download; the status says "Not released yet".
- [ ] Feature claims match §7.5; Forge/NeoForge are listed as not done yet.
- [ ] Sign-in steps (§7.4) and the privacy policy (§9) are present.

**Technical:**
- [ ] No horizontal scroll at 320, 375, 768, 1280 and 1920px; no button or nav label wraps onto two lines.
- [ ] No requests to other domains (fonts are self-hosted).
- [ ] Lighthouse Accessibility and Best Practices are both 95 or above.
- [ ] Every `<<PLACEHOLDER>>` is still visible and listed in your final reply.
