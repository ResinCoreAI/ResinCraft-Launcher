import { Icon } from '@/components/Icons'
import Screens from '@/components/Screens'
import { DownloadLink, ForYou, HeroBadge, History, ReleaseLine, WhatsNew, WindowsFiles } from '@/components/Release'
import { GITHUB, RELAY_EXAMPLE } from '@/lib/site'

const STATS = [
  ['5', 'Mod & modpack sources'],
  ['9', 'Server types'],
  ['4', 'Mod loaders'],
  ['16', 'Languages'],
]

const FEATURES = [
  ['layers', 'Instances & groups', 'Every version or modpack gets its own folder, Java and settings. Sort them into groups by dragging cards.'],
  ['search', 'Five sources, one search', 'Modrinth, CurseForge, FTB, ATLauncher and Technic: mods, modpacks, resource packs and shaders, filtered to your game version.'],
  ['history', 'Pick the exact version', 'Install the newest with one click or choose any release, beta or alpha, and switch later. Dependencies come along on their own.'],
  ['package', 'Modpacks in and out', 'Import Modrinth, CurseForge, FTB, ATLauncher and Technic packs. Share yours as a .mrpack or a CurseForge zip.'],
  ['server', 'Host a server', 'Vanilla, Paper, Purpur, Folia, Spigot, Fabric, Quilt, Forge or NeoForge, or turn a modpack into a server. Tuned Java flags included.'],
  [
    'globe',
    'Friends join from anywhere',
    <>
      Your server gets an address like <span className="mono">{RELAY_EXAMPLE}</span>. No port forwarding, nothing for friends to install.
    </>,
  ],
  ['puzzle', 'Plugins, managed like mods', 'Plugins from Modrinth, Hangar, SpigotMC and CurseForge, with a version picker, update checks and Update all.'],
  ['folder', 'Files & editor', 'Browse any instance or server folder and edit configs with syntax highlighting. Deleted files go to the Recycle Bin.'],
  ['shirt', 'Skins, capes & names', 'Several Microsoft accounts and offline profiles. Upload a skin, pick a cape and change your name without leaving the launcher.'],
  ['pointer', 'Right-click everything', 'Instances, mods, worlds, screenshots, files, servers and plugins all have a menu with the actions you need, by mouse or keyboard.'],
  ['refresh', 'Updates itself', 'ResinCraft checks for new versions and installs them in place with one click. Your instances, worlds and settings stay.'],
  [
    'languages',
    '16 languages',
    <span lang="mul">English, ไทย, 日本語, 한국어, 中文, Tiếng Việt, Bahasa Indonesia, Deutsch, Français, Español, Italiano, Polski, Português, Русский and Türkçe.</span>,
  ],
]

const ROUTES = [
  ['Your router', 'ResinCraft asks it to open the port. The fastest way, when the router allows it.'],
  [
    'ResinCraft Relay',
    <>
      Otherwise friends type <span className="mono">{RELAY_EXAMPLE}</span>. Just sign in with Microsoft.
    </>,
  ],
  ['playit.gg', "A free address that stays the same, if you've linked it."],
  ['bore.pub', 'The last resort, so the server can still be reached.'],
]

const STEPS = [
  ['Install and sign in', "Sign in with your Microsoft account on Microsoft's own page; your password never goes into ResinCraft. Or add an offline profile."],
  ['Make it yours', 'Create an instance for any version, or install a modpack. Add mods, shaders and resource packs as you like.'],
  ['Press Play', 'The right Java is found or downloaded for you, and the game files are checked before every start.'],
]

const FAQ = [
  ['Is ResinCraft free?', "Yes. It's free and open source under the MIT licence, and the code is on GitHub."],
  [
    'Do I need to own Minecraft?',
    'To play online, yes: you sign in with the Microsoft account that owns Minecraft: Java Edition. Offline profiles work for singleplayer, LAN and offline-mode servers.',
  ],
  ['Is signing in safe?', "You sign in on Microsoft's own website with a short code, so ResinCraft never sees your password. Sign-in tokens are stored encrypted on your PC."],
  [
    'How do my friends join my server?',
    <>
      ResinCraft first asks your router to open the port. If it can't, your server gets an address like <span className="mono">{RELAY_EXAMPLE}</span> through the
      ResinCraft Relay. Friends type that into Minecraft; they don't need to install anything.
    </>,
  ],
  [
    'How do updates work?',
    'ResinCraft checks for a new version when it starts and every few hours. One click downloads the same installer and updates in place, keeping your instances, worlds and accounts.',
  ],
  ['Which computers does it run on?', 'Windows 10 and 11 (64-bit) for now. macOS and Linux builds are planned.'],
  [
    'Windows says “Windows protected your PC”',
    <>
      The installer isn't code-signed yet, which costs money every year. Choose <b>More info</b>, then <b>Run anyway</b>. The source and the build are public on GitHub.
    </>,
  ],
  ['Does it run Forge modpacks?', 'Yes: Forge, NeoForge, Fabric and Quilt all work, for both playing and hosting servers.'],
  [
    'Where are my files kept?',
    <>
      In <span className="mono">%APPDATA%\ResinCraft Launcher</span>. Uninstalling leaves that folder, so your worlds aren't lost.
    </>,
  ],
  ['Is it made by Mojang or Microsoft?', 'No. ResinCraft is an independent project by Puirkle, written from scratch.'],
]

function SectionHead({ kicker, title, children }) {
  return (
    <header className="sec-head reveal">
      <span className="kicker">{kicker}</span>
      <h2>{title}</h2>
      {children}
    </header>
  )
}

export default function Home() {
  return (
    <>
      {/* ============ Hero ============ */}
      <section className="hero">
        <div className="hero-bg" aria-hidden="true" />
        <div className="wrap hero-grid">
          <div className="hero-copy">
            <HeroBadge />
            <h1>
              Modded Minecraft, <span className="grad">all in one place</span>
            </h1>
            <p className="lede">
              ResinCraft Launcher gives every version and modpack its own instance, finds mods on Modrinth, CurseForge, FTB, ATLauncher and Technic, and hosts
              servers your friends can join from anywhere. Free and open source.
            </p>
            <div className="cta-row">
              <DownloadLink className="btn primary big">Download for Windows</DownloadLink>
              <a className="btn big" href={GITHUB}>
                <Icon name="github" /> View on GitHub
              </a>
            </div>
            <p className="platforms">
              <Icon name="windows" small />
              <span>Windows 10 &amp; 11 · free &amp; open source (MIT) ·</span>
              <a href="#downloads">All downloads</a>
            </p>
          </div>

          <div className="hero-shot">
            <img
              className="shot-main"
              src="/assets/hero@1x.webp"
              srcSet="/assets/hero@1x.webp 1440w, /assets/hero.webp 2880w"
              sizes="(max-width: 960px) 92vw, 640px"
              alt="ResinCraft Launcher's Home screen: continue playing, play time and your instances"
              width="1440"
              height="900"
              fetchPriority="high"
            />
            <figure className="float-card">
              <img src="/assets/shots/version-picker@1x.webp" alt="" width="1440" height="900" loading="lazy" />
              <figcaption>
                <Icon name="check" small /> Any mod version, any time
              </figcaption>
            </figure>
          </div>
        </div>

        <div className="wrap">
          <dl className="stats-row reveal">
            {STATS.map(([n, label]) => (
              <div key={label}>
                <dt>{n}</dt>
                <dd>{label}</dd>
              </div>
            ))}
          </dl>
        </div>
      </section>

      {/* ============ Features ============ */}
      <section className="section" id="features">
        <div className="wrap">
          <SectionHead kicker="Features" title="Everything for modded Minecraft, without the folder mess" />
          <div className="feature-grid">
            {FEATURES.map(([icon, title, text]) => (
              <article className="card feature reveal" key={title}>
                <span className="f-ico">
                  <Icon name={icon} />
                </span>
                <h3>{title}</h3>
                <p>{text}</p>
              </article>
            ))}
          </div>
        </div>
      </section>

      {/* ============ Screenshots ============ */}
      <section className="section alt" id="screens">
        <div className="wrap">
          <SectionHead kicker="Screenshots" title="A closer look" />
          <Screens />
        </div>
      </section>

      {/* ============ Servers ============ */}
      <section className="section" id="servers">
        <div className="wrap split">
          <div className="reveal">
            <span className="kicker">Server hosting</span>
            <h2>Host a server for your friends</h2>
            <p className="lede">Make a server on your own PC in a minute, from server software, a modpack, one of your instances or a modpack file.</p>
            <ul className="ticks">
              {[
                'Plugins and server mods with a version picker and one-click updates',
                'Console, players, whitelist, server.properties and files, all in the app',
                'Starts with optimised Java flags, ready for big modpacks',
                'Your own domain if you have one, with the DNS records shown and checked',
              ].map((t) => (
                <li key={t}>
                  <Icon name="check" small /> {t}
                </li>
              ))}
            </ul>
            <DownloadLink>Download</DownloadLink>
          </div>
          <div className="card route reveal">
            <h3>How friends connect</h3>
            <ol>
              {ROUTES.map(([title, text]) => (
                <li key={title}>
                  <b>{title}</b>
                  <span>{text}</span>
                </li>
              ))}
            </ol>
            <p className="muted small">You can also pick a route yourself, or tell ResinCraft you forwarded the port by hand.</p>
          </div>
        </div>
      </section>

      {/* ============ How it works ============ */}
      <section className="section alt" id="how">
        <div className="wrap">
          <SectionHead kicker="How it works" title="From download to playing in three steps" />
          <div className="steps">
            {STEPS.map(([title, text], i) => (
              <article className="card step reveal" key={title}>
                <span className="num">0{i + 1}</span>
                <h3>{title}</h3>
                <p>{text}</p>
              </article>
            ))}
          </div>
        </div>
      </section>

      {/* ============ Downloads ============ */}
      <section className="section" id="downloads">
        <div className="wrap">
          <SectionHead kicker="Downloads" title="Get ResinCraft Launcher">
            <ReleaseLine />
          </SectionHead>

          <ForYou />

          <div className="platform-grid">
            <article className="card reveal">
              <h3>
                <span className="p-ico">
                  <Icon name="windows" />
                </span>
                Windows
              </h3>
              <WindowsFiles />
              <p className="muted small">Windows 10 or 11, 64-bit. Installs just for you, with no administrator rights, and updates itself after that.</p>
            </article>
            <article className="card reveal">
              <h3>
                <span className="p-ico">
                  <Icon name="laptop" />
                </span>
                Mac &amp; Linux
              </h3>
              <p className="muted">Not yet. Builds for macOS and Linux are planned; watch the GitHub releases.</p>
            </article>
          </div>

          <p className="help-line">
            Windows says <b>“Windows protected your PC”</b>? The installer isn't code-signed yet. Choose <b>More info</b>, then <b>Run anyway</b>.
          </p>

          <WhatsNew />
          <History />
        </div>
      </section>

      {/* ============ FAQ ============ */}
      <section className="section alt" id="faq">
        <div className="wrap narrow">
          <SectionHead kicker="FAQ" title="Questions people ask" />
          <div className="faq reveal">
            {FAQ.map(([q, a]) => (
              <details key={q}>
                <summary>{q}</summary>
                <p>{a}</p>
              </details>
            ))}
          </div>
        </div>
      </section>

      {/* ============ Final call to action ============ */}
      <section className="section final-cta">
        <div className="wrap reveal">
          <h2>Ready to play?</h2>
          <p className="lede">Download it free. Sign in with the Microsoft account you play Minecraft with, and you're set.</p>
          <div className="cta-row center">
            <DownloadLink className="btn primary big">Download</DownloadLink>
            <a className="btn big" href={GITHUB}>
              <Icon name="github" /> GitHub
            </a>
          </div>
        </div>
      </section>
    </>
  )
}
