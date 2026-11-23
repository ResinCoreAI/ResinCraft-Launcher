// Cards under a chat message: a ResinCraft Relay server address in the text (copy it, or join it from Quick join), a
// Store item someone shared, a mod or modpack share link (its card, as Discord shows it), and image links. Images
// load only when clicked, so the site they're on doesn't see who's looking until then.
import { useEffect, useState } from 'react'
import { Copy, ExternalLink, Image as ImageIcon, Package, Play, Server, ShoppingBag } from 'lucide-react'
import type { ChatEmbed, ChatMessage, ProjectHit } from '@shared/types'
import { api } from '../../api'
import { money } from '../../storeStatus'
import { useStore } from '../../store'
import { t } from '../../i18n'

const SERVER = /\b([a-z0-9][a-z0-9-]{0,62})\.rscraft\.resincore\.tech(?::\d{2,5})?\b/gi
const IMAGE = /https?:\/\/[^\s<>"]+?\.(?:png|jpe?g|gif|webp)(?:\?[^\s<>"]*)?(?=[\s)>\]]|$)/gi
/** A share link for a mod or modpack: the relay's page (https://rscraft.resincore.tech/mod/<id>) or resincraft://mod/<id>. */
const MOD = /(?:https?:\/\/(?:[a-z0-9-]+\.)*rscraft\.resincore\.tech\/mod\/|resincraft:\/\/mod\/)([^\s<>"?#]+)/gi

/** Projects looked up for cards, kept while the launcher runs (each link asks once). */
const projects = new Map<string, Promise<ProjectHit | null>>()
function projectOf(id: string): Promise<ProjectHit | null> {
  let p = projects.get(id)
  if (!p) {
    p = api.content.details(id).then(
      (d) => d,
      () => null
    )
    projects.set(id, p)
  }
  return p
}

/** A mod or modpack link's card, like Discord's: the launcher, its name, what it is and its logo; Open goes to its page. */
function ModCard({ id }: { id: string }) {
  const { go } = useStore()
  const [p, setP] = useState<ProjectHit | null | undefined>()
  useEffect(() => {
    let live = true
    void projectOf(id).then((x) => live && setP(x))
    return () => {
      live = false
    }
  }, [id])
  const open = () => go({ page: 'discover', project: id })
  return (
    <div className="chat-embed chat-modcard">
      <div className="chat-modcard-text">
        <span className="chat-modcard-site">ResinCraft Launcher</span>
        <button className="chat-modcard-title" onClick={open}>
          {p ? p.title : p === null ? t('chat.embed.modLink') : '…'}
        </button>
        {p?.description && <span className="chat-modcard-desc">{p.description}</span>}
        <span className="chat-modcard-foot">
          <button className="btn sm primary" onClick={open}>
            {t('chat.embed.openMod')}
          </button>
          {!!p?.downloads && <span className="hint">{t('chat.embed.downloads', { n: new Intl.NumberFormat().format(p.downloads) })}</span>}
        </span>
      </div>
      {p?.iconUrl ? (
        <img className="chat-modcard-icon" src={p.iconUrl} alt="" loading="lazy" referrerPolicy="no-referrer" />
      ) : (
        <span className="chat-modcard-icon blank" aria-hidden>
          <Package />
        </span>
      )}
    </div>
  )
}

function ServerCard({ address }: { address: string }) {
  const { go, toast } = useStore()
  const name = address.split('.')[0]
  return (
    <div className="chat-embed">
      <span className="chat-embed-icon" aria-hidden>
        <Server />
      </span>
      <span className="chat-embed-text">
        <b>{name}</b>
        <span className="hint mono ellipsis">{address}</span>
      </span>
      <button className="btn ghost icon sm" title={t('chat.embed.copyAddress')} aria-label={t('chat.embed.copyAddress')} onClick={() => void navigator.clipboard.writeText(address).then(() => toast(t('chat.embed.copied'), 'success'))}>
        <Copy />
      </button>
      <button className="btn sm primary" onClick={() => go({ page: 'quickjoin', add: { name, address, edition: 'java' } })}>
        <Play /> {t('chat.embed.join')}
      </button>
    </div>
  )
}

function ImageCard({ url }: { url: string }) {
  const [shown, setShown] = useState(false)
  const [broken, setBroken] = useState(false)
  let host = ''
  try {
    host = new URL(url).host
  } catch {}
  if (shown && !broken) {
    return (
      <button className="chat-embed-image" title={url} onClick={() => void api.app.openExternal(url)}>
        <img src={url} alt="" loading="lazy" referrerPolicy="no-referrer" onError={() => setBroken(true)} />
      </button>
    )
  }
  return (
    <div className="chat-embed">
      <span className="chat-embed-icon" aria-hidden>
        <ImageIcon />
      </span>
      <span className="chat-embed-text">
        <b>{broken ? t('chat.embed.imageFailed') : t('chat.embed.image')}</b>
        <span className="hint ellipsis">{host}</span>
      </span>
      {!broken && (
        <button className="btn sm" onClick={() => setShown(true)}>
          {t('chat.embed.showImage')}
        </button>
      )}
      <button className="btn ghost icon sm" title={t('chat.embed.open')} aria-label={t('chat.embed.open')} onClick={() => void api.app.openExternal(url)}>
        <ExternalLink />
      </button>
    </div>
  )
}

function StoreCard({ e }: { e: ChatEmbed }) {
  const { go } = useStore()
  return (
    <div className="chat-embed store">
      <span className="chat-embed-icon" aria-hidden>
        <ShoppingBag />
      </span>
      <span className="chat-embed-text">
        <b>{e.name}</b>
        {e.description && <span className="hint">{e.description}</span>}
        <span className="data">
          {e.monthly > 0 && t('chat.gift.perMonth', { amount: money(e.monthly, e.currency) })}
          {e.monthly > 0 && e.yearly > 0 && ' · '}
          {e.yearly > 0 && t('chat.gift.perYear', { amount: money(e.yearly, e.currency) })}
        </span>
      </span>
      <button className="btn sm" onClick={() => go({ page: 'store' })}>
        {t('chat.embed.openStore')}
      </button>
    </div>
  )
}

export default function Embeds({ m }: { m: ChatMessage }) {
  if (m.deleted) return null
  const shared = m.data && 'embed' in m.data ? m.data.embed : null
  const text = m.body.replace(/```[\s\S]*?```/g, ' ')
  const servers = [...new Set([...text.matchAll(SERVER)].map((x) => x[0].toLowerCase()))].slice(0, 3)
  const images = [...new Set([...text.matchAll(IMAGE)].map((x) => x[0]))].slice(0, 3)
  const mods = [...new Set([...text.matchAll(MOD)].map((x) => decodeURIComponent(x[1]).replace(/[).,!]+$/, '')))].filter(Boolean).slice(0, 3)
  if (!shared && !servers.length && !images.length && !mods.length) return null
  return (
    <div className="chat-embeds">
      {shared?.type === 'store' && <StoreCard e={shared} />}
      {mods.map((id) => (
        <ModCard key={id} id={id} />
      ))}
      {servers.map((a) => (
        <ServerCard key={a} address={a} />
      ))}
      {images.map((u) => (
        <ImageCard key={u} url={u} />
      ))}
    </div>
  )
}
