import { DISCLAIMER, EMAIL, GITHUB } from '@/lib/site'

export const metadata = {
  title: 'Terms',
  description: 'The terms for using ResinCraft Launcher and the ResinCraft Relay.',
  alternates: { canonical: '/terms/' },
}

export default function Terms() {
  return (
    <article className="doc">
      <h1>Terms</h1>
      <p className="updated">Last updated 23 October 2026</p>

      <h2>The software</h2>
      <p>
        ResinCraft Launcher is free and open source under the <a href={`${GITHUB}/blob/main/LICENSE`}>GNU General Public License v3</a>. It is provided as is, without warranty
        of any kind.
      </p>

      <h2>Minecraft</h2>
      <p>
        You need a legitimate copy of Minecraft: Java Edition to play online. ResinCraft doesn't provide or unlock the game. Minecraft is governed by the Minecraft
        EULA and Microsoft's terms, and those apply to servers you host with ResinCraft too.
      </p>

      <h2>Mods, modpacks and plugins</h2>
      <p>They belong to their authors and come under their own licences. ResinCraft downloads them from the sites their authors publish them on.</p>

      <h2>The ResinCraft Relay</h2>
      <ul>
        <li>The relay is offered free, without any promise that it is always available.</li>
        <li>Use it only for Minecraft servers. Don't use it for anything illegal, for attacks, or to break Mojang's or Microsoft's rules.</li>
        <li>Names are first come, first served. We may remove names that impersonate others or are offensive, and block accounts that abuse the relay.</li>
      </ul>

      <h2>Contact</h2>
      <p>
        <a href={`mailto:${EMAIL}`}>{EMAIL}</a>
      </p>

      <p className="updated" style={{ marginTop: 40 }}>
        {DISCLAIMER}
      </p>
    </article>
  )
}
