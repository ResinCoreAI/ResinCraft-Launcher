# ResinCraft Launcher website

Next.js (App Router, JSX) built as a **static export**: `npm run build` writes plain HTML, CSS and JS to `out/`, ready for any web server. The design follows qr.puirkle.net (PuirkleQR Studio), with ResinCraft's cyan and green.

The newest version, download links, "What's new" and the version history come live from the GitHub releases of [ResinCoreAI/ResinCraft-Launcher](https://github.com/ResinCoreAI/ResinCraft-Launcher). Publishing a release updates the site on its own. If GitHub can't be reached, every Download button still opens `/releases/latest`.

> The copy describes **v0.3.0** (updater, right-click menus, plugin manager, ResinCraft Relay, offline profiles without Microsoft). Deploy it once v0.3.0 is published and the relay is running at `*.rscraft.resincore.tech`.

## Develop

```bash
npm install
npm run dev        # http://localhost:7811
```

## Build

Set your domain (used for the canonical URL, Open Graph image, robots.txt and sitemap.xml):

```bash
SITE_URL=https://your.domain npm run build      # PowerShell: $env:SITE_URL="https://your.domain"; npm run build
npm start                                       # preview out/ at http://localhost:7811
```

Without `SITE_URL`, the build uses `https://resincraft.resincore.tech`.

## Deploy on your own server

Copy the contents of `out/` to the web root, for example `/var/www/resincraft`. Example nginx site:

```nginx
server {
    listen 443 ssl http2;
    server_name your.domain;
    root /var/www/resincraft;

    # ssl_certificate / ssl_certificate_key: e.g. from certbot

    location / {
        try_files $uri $uri/ $uri.html =404;
    }
    error_page 404 /404.html;

    location /_next/static/ {
        add_header Cache-Control "public, max-age=31536000, immutable";
    }
    location /assets/ {
        add_header Cache-Control "public, max-age=604800";
    }

    add_header X-Content-Type-Options nosniff always;
    add_header Referrer-Policy strict-origin-when-cross-origin always;
    # Next.js inlines small scripts, hence 'unsafe-inline'; the only outside request is GitHub's API.
    add_header Content-Security-Policy "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src https://api.github.com; base-uri 'none'; form-action 'none'; frame-ancestors 'none'" always;
}
```

Any static host works too (Cloudflare Pages: build command `npm run build`, output directory `out`, environment variable `SITE_URL`).

## Where things are

| Path | What |
|---|---|
| `app/page.jsx` | Landing page: hero, features, screenshots, servers, how it works, downloads, FAQ |
| `app/privacy/`, `app/terms/`, `app/not-found.jsx` | Text pages and the 404 |
| `app/layout.jsx` | Fonts (self-hosted via `next/font/local`), metadata, theme boot script, header and footer |
| `app/globals.css` | All styles; colours are tokens at the top (light and dark) |
| `components/Release.jsx`, `lib/release.js` | Live GitHub release data with static fallbacks |
| `components/Header.jsx` | Top bar, theme toggle, mobile menu |
| `lib/site.js` | Repo, email, relay example address, disclaimer |
| `public/assets/` | Logo, favicons, screenshots, `og.png` |

New screenshots: run `npm run build:web && npx electron scripts/screenshots.cjs` in the launcher repo, then convert them to WebP (2880 px and a 1440 px `@1x`) into `public/assets/shots/`.
