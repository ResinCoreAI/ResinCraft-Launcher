// Mock details pages for the browser preview: each source shaped like the real one (Modrinth Markdown with a
// gallery and dependencies, CurseForge HTML with screenshots, GitHub README and release notes, nothing else).
// The bodies carry a script, an inline handler and a javascript: link so the sanitizer is exercised too.
import type { GalleryImage, ProjectDetails, ProjectHit, VersionInfo } from '@shared/types'
import { sourceOf } from '@shared/types'

/** A made-up screenshot: a gradient with a caption, as an SVG data URL. */
function shot(title: string, from: string, to: string): string {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="960" height="540"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="${from}"/><stop offset="1" stop-color="${to}"/></linearGradient></defs><rect width="960" height="540" fill="url(#g)"/><rect y="380" width="960" height="160" fill="#1d3a1b" opacity=".55"/><text x="48" y="470" font-family="sans-serif" font-size="44" fill="#fff">${title}</text></svg>`
  return `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`
}

const HOSTILE = `<script>alert('not run')</script><img src="x" onerror="alert('not run')" alt=""><a href="javascript:alert('no')">a harmless link</a>`

function gallery(title: string): GalleryImage[] {
  return [
    { url: shot(`${title}: a quiet village`, '#87ceeb', '#3f8f4f'), title: 'Village at dusk', description: 'Default settings, 16 chunks.' },
    { url: shot(`${title}: the nether`, '#5b1a1a', '#e2733c'), title: 'Nether fortress' },
    { url: shot(`${title}: settings`, '#2b2f3a', '#6b7aa8'), title: 'Video settings' }
  ]
}

function modrinth(h: ProjectHit): ProjectDetails {
  return {
    ...h,
    author: h.author,
    body: `<h2>About ${h.title}</h2>
<p>${h.description} Works on <strong>Fabric</strong> and <strong>Quilt</strong>; see the <a href="https://modrinth.com/mod/${h.slug}/versions">version list</a> for older releases.</p>
<p><img src="${shot(h.title, '#39e58c', '#1e6b46')}" alt="Banner" width="480"></p>
<h3>Features</h3>
<ul><li>Smoother frame rates, especially with high render distances</li><li>Fewer stutters when loading chunks</li><li>Fully compatible with vanilla resource packs</li></ul>
<table><thead><tr><th>Setting</th><th>Default</th></tr></thead><tbody><tr><td>Chunk builder threads</td><td>Auto</td></tr><tr><td>Use block face culling</td><td>On</td></tr></tbody></table>
<blockquote><p>Report problems on the issue tracker, not in reviews.</p></blockquote>
<pre><code>// config/${h.slug}.json
{ "advanced": { "cpu_render_ahead_limit": 3 } }</code></pre>
${HOSTILE}`,
    gallery: gallery(h.title),
    links: [
      { kind: 'source', url: `https://github.com/CaffeineMC/${h.slug}` },
      { kind: 'issues', url: `https://github.com/CaffeineMC/${h.slug}/issues` },
      { kind: 'discord', url: 'https://discord.gg/example' },
      { kind: 'donate', url: 'https://ko-fi.com/example' }
    ],
    license: 'Polyform Shield License 1.0.0',
    changelogs: true,
    dependencies: true
  }
}

function curseforge(h: ProjectHit): ProjectDetails {
  const pack = h.kind === 'modpack'
  return {
    ...h,
    body: `<p style="text-align:center"><img src="${shot(h.title, '#f16436', '#6b2412')}" width="600" alt=""></p>
<h2>${pack ? 'What is in the pack' : 'Overview'}</h2>
<p>${h.description}</p>
<p>${pack ? 'Over 200 mods, quests for every chapter and a custom main menu. Needs at least <b>6 GB</b> of memory.' : 'A library many mods need.'}</p>
<ul><li>Join our <a href="https://discord.gg/example">Discord</a></li><li><a href="#faq">FAQ</a></li></ul>
<h3 id="faq">FAQ</h3><p><em>Can I use this on a server?</em> Yes, a server pack is on every file.</p>
<iframe src="https://www.youtube.com/embed/dQw4w9WgXcQ" width="560" height="315"></iframe>
${HOSTILE}`,
    gallery: gallery(h.title).map((g) => ({ ...g, thumb: g.url })),
    links: [
      { kind: 'issues', url: `https://github.com/example/${h.slug}/issues` },
      { kind: 'wiki', url: `https://${h.slug}.wiki.example` }
    ],
    changelogs: true,
    dependencies: !pack
  }
}

function github(h: ProjectHit): ProjectDetails {
  const repo = h.id.replace(/^gh:/, '')
  return {
    ...h,
    body: `<h1>${h.title}</h1>
<p><img src="${shot('build passing', '#2ea043', '#1a7f37')}" alt="build" height="20"> <img src="${shot('license MIT', '#0969da', '#054da7')}" alt="license" height="20"></p>
<p>${h.description}</p>
<h2>Installing</h2>
<ol><li>Install <a href="https://fabricmc.net">Fabric</a></li><li>Download the jar from <a href="https://github.com/${repo}/releases">Releases</a></li><li>Drop it in <code>mods/</code></li></ol>
<h2>Building</h2>
<pre><code>./gradlew build</code></pre>
<p>- [x] Fabric<br>- [ ] Forge (planned)</p>
${HOSTILE}`,
    gallery: [],
    links: [{ kind: 'issues', url: `https://github.com/${repo}/issues` }],
    license: 'MIT',
    changelogs: true,
    dependencies: false
  }
}

/** Packs from FTB, ATLauncher and Technic: text, a picture or two, no dependencies. */
function modpackOnly(h: ProjectHit): ProjectDetails {
  const technic = h.source === 'technic'
  return {
    ...h,
    body: `<p>${h.description}</p><p>Explore, build and automate with friends.<br>Updated regularly.</p>`,
    gallery: technic
      ? [{ url: shot(h.title, '#facc15', '#7a5b05') }]
      : h.source === 'ftb'
        ? [{ url: shot(h.title, '#b26bff', '#3d1d66'), title: 'Background' }]
        : [],
    links: h.source === 'atlauncher' ? [{ kind: 'discord', url: 'https://discord.gg/example' }] : [],
    changelogs: !technic,
    dependencies: false
  }
}

export function mockDetails(h: ProjectHit): ProjectDetails {
  switch (h.source) {
    case 'curseforge':
      return curseforge(h)
    case 'github':
      return github(h)
    case 'modrinth':
      return modrinth(h)
    default:
      return modpackOnly(h)
  }
}

export function mockVersionInfo(projectId: string, versionId: string, hits: ProjectHit[]): VersionInfo {
  const source = sourceOf(projectId)
  const n = versionId.replace(/\D/g, '') || '0'
  const changelog =
    source === 'curseforge'
      ? `<p><b>Version ${n}</b></p><ul><li>Fixed a crash when opening the inventory</li><li>Updated translations</li></ul>`
      : `<h3>Changes</h3><ul><li>Faster chunk meshing (#${400 + Number(n)})</li><li>Fixed flickering clouds</li></ul><p>Thanks to everyone who reported bugs!</p>`
  const other = hits.filter((h) => h.id !== projectId && h.source === source)
  const dependencies: VersionInfo['dependencies'] =
    source === 'modrinth' || source === 'curseforge'
      ? [
          ...other.slice(0, 1).map((h) => ({ id: h.id, title: h.title, iconUrl: h.iconUrl, type: 'required' as const })),
          ...other.slice(1, 3).map((h) => ({ id: h.id, title: h.title, iconUrl: h.iconUrl, type: 'optional' as const })),
          { title: 'Indium', type: 'incompatible' as const }
        ]
      : []
  return { changelog: source === 'technic' ? undefined : changelog, dependencies }
}
