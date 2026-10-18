import { EMAIL } from '@/lib/site'

export const metadata = {
  title: 'Privacy policy',
  description: 'What ResinCraft Launcher and this website do with your data: nothing is sent to Puirkle, and there is no telemetry.',
  alternates: { canonical: '/privacy/' },
}

export default function Privacy() {
  return (
    <article className="doc">
      <h1>Privacy policy</h1>
      <p className="updated">Last updated 18 October 2026</p>

      <h2>Who we are</h2>
      <p>
        ResinCraft Launcher is made by Puirkle and published by ResinCore. Questions about privacy: <a href={`mailto:${EMAIL}`}>{EMAIL}</a>.
      </p>

      <h2>This website</h2>
      <p>
        This site uses no cookies, analytics or trackers. To show the newest version, your browser asks GitHub's public API which release is the latest; GitHub's
        privacy policy applies to that request and to downloads, which are served by GitHub. The server hosting this site may keep standard access logs.
      </p>

      <h2>The launcher runs on your computer</h2>
      <p>
        Puirkle runs no servers for ResinCraft and receives none of your data. There is no telemetry and no ResinCraft account. Everything the launcher stores
        stays in <span className="mono">%APPDATA%\ResinCraft Launcher</span> on your PC.
      </p>

      <h2>Signing in with Microsoft</h2>
      <p>
        You sign in on Microsoft's own website with a short device code, so ResinCraft never sees your password. ResinCraft receives sign-in tokens from Microsoft,
        Xbox Live and Minecraft services, keeps them only on your computer, encrypted with Windows' data protection, and sends them only to those services: to sign
        you in, start the game, and change your skin, cape or name when you ask it to.
      </p>
      <p>The launcher stores your Minecraft username, UUID and skin so it can show who is signed in. Offline profiles store only the name you type.</p>

      <h2>Services the launcher contacts</h2>
      <p>Depending on what you do, ResinCraft connects directly from your computer to:</p>
      <ul>
        <li>
          <b>Microsoft, Xbox Live, Mojang and Minecraft services</b>: sign-in, your profile, skins and capes, and the game files.
        </li>
        <li>
          <b>Modrinth, CurseForge, Feed The Beast, ATLauncher and Technic</b>: searching and downloading mods and modpacks. ResinCraft uses its own built-in
          CurseForge API key.
        </li>
        <li>
          <b>Fabric, Quilt, Forge and NeoForge</b>: mod loaders.
        </li>
        <li>
          <b>Eclipse Adoptium</b>: Java downloads.
        </li>
        <li>
          <b>GitHub</b>: checking for and downloading ResinCraft updates.
        </li>
        <li>
          <b>PaperMC (Paper, Folia, Hangar), Purpur, SpigotMC and Spiget</b>: only when you host a server.
        </li>
      </ul>
      <p>Each of these services has its own privacy policy.</p>

      <h2>Hosting a server</h2>
      <p>Only if you host a server and make it public:</p>
      <ul>
        <li>
          ResinCraft first asks your router to open the server's port (UPnP) and closes it again when the server stops.
        </li>
        <li>
          If that isn't possible, it shares the server through the <b>ResinCraft Relay</b>. To claim an address, the launcher proves to the relay that your
          Microsoft account owns Minecraft; the relay keeps your Minecraft UUID and the name you chose, never your Microsoft tokens. Players' connections pass through
          the relay to reach your PC.
        </li>
        <li>
          Other routes you can use instead: <b>playit.gg</b> (once you link an account) and the public <b>bore.pub</b> relay. Traffic through them passes through
          those services.
        </li>
        <li>
          <b>Your own domain (optional):</b> DNS checks ask public resolvers (Cloudflare 1.1.1.1, or Google 8.8.8.8), sending only the domain name.
        </li>
        <li>
          <b>Cloudflare automation (optional, off by default):</b> if you turn it on, your Cloudflare API token is stored encrypted on your PC and sent only to
          Cloudflare's API to update your domain's records.
        </li>
      </ul>
      <p>Players who join your server see the address you share with them.</p>

      <h2>Deleting your data</h2>
      <p>
        Remove accounts in the launcher, or delete <span className="mono">%APPDATA%\ResinCraft Launcher</span>. Uninstalling keeps that folder so your worlds
        aren't lost; delete it yourself if you want everything gone. To free a ResinCraft Relay name, email us.
      </p>

      <h2>Children</h2>
      <p>Signing in follows Microsoft's account and family rules.</p>

      <h2>Changes</h2>
      <p>If this policy changes, the new version is posted on this page with a new date.</p>
    </article>
  )
}
