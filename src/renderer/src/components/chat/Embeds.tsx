// Cards under a chat message: a ResinCraft Relay server address in the text (copy it, or join it from Quick join), a
// Store item someone shared, and image links. Images load only when clicked, so the site they're on doesn't see who's
// looking until then.
import { useState } from 'react'
import { Copy, ExternalLink, Image as ImageIcon, Play, Server, ShoppingBag } from 'lucide-react'
import type { ChatEmbed, ChatMessage } from '@shared/types'
import { api } from '../../api'
import { money } from '../../storeStatus'
import { useStore } from '../../store'
import { t } from '../../i18n'

const SERVER = /\b([a-z0-9][a-z0-9-]{0,62})\.rscraft\.resincore\.tech(?::\d{2,5})?\b/gi
const IMAGE = /https?:\/\/[^\s<>"]+?\.(?:png|jpe?g|gif|webp)(?:\?[^\s<>"]*)?(?=[\s)>\]]|$)/gi

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
  if (!shared && !servers.length && !images.length) return null
  return (
    <div className="chat-embeds">
      {shared?.type === 'store' && <StoreCard e={shared} />}
      {servers.map((a) => (
        <ServerCard key={a} address={a} />
      ))}
      {images.map((u) => (
        <ImageCard key={u} url={u} />
      ))}
    </div>
  )
}
