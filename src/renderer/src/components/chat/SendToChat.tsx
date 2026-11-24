// "Send to chat" from anywhere in the launcher: pick a friend's chat or a group, add a note, send. What can go:
// - a mod, modpack, shader or resource pack: its share link (rscraft.resincore.tech/mod/…), which shows as a card with
//   Open (Embeds.tsx ModCard); a file with no project behind it goes as the file;
// - an instance: exported as a .mrpack (mods that are on Modrinth or CurseForge go as download links, so it stays small;
//   the instance's configs and other suggested folders go inside), which the receiver installs from the chat;
// - files: screenshots (as pictures) and config or other files from an instance's Files tab;
// - a Store item: its card with the price.
// One host (SendToChatHost, in App) shows the window; anything opens it with sendToChat(item).
import { useEffect, useState } from 'react'
import { Loader2, Send } from 'lucide-react'
import type { Instance } from '@shared/types'
import { api } from '../../api'
import { chat, dmId, showChat, useChat } from '../../chatState'
import { useStore } from '../../store'
import { t } from '../../i18n'
import { dataPath } from '../../util'
import { Modal } from '../ui'
import { GroupTile } from './ChatGroups'
import PlayerHead from '../PlayerHead'

export type Sendable =
  | { kind: 'link'; title: string; url: string }
  | { kind: 'files'; title: string; files: SendFile[] }
  | { kind: 'instance'; title: string; inst: Instance }
  | { kind: 'store'; title: string; product: string }

/** A file on this PC: a picture goes as one (`measure` gives its size, so the chat lays it out before it loads). */
export interface SendFile {
  path: string
  size?: number
  image?: boolean
  measure?: () => Promise<{ w: number; h: number } | undefined>
}

let open: ((item: Sendable) => void) | null = null
/** Opens "Send to chat" for `item` (SendToChatHost shows it). */
export function sendToChat(item: Sendable): void {
  open?.(item)
}

/** A mod, pack or shader by its project (Modrinth or CurseForge id): its share link, which shows as a card. */
export async function sendProject(projectId: string, title: string): Promise<void> {
  sendToChat({ kind: 'link', title, url: await api.chat.shareLink(projectId) })
}

/** The window, mounted once in App. */
export function SendToChatHost() {
  const [item, setItem] = useState<Sendable | null>(null)
  useEffect(() => {
    open = setItem
    return () => {
      open = null
    }
  }, [])
  return item ? <SendToChatModal item={item} onClose={() => setItem(null)} /> : null
}

/** A picture's size, read from it. */
export async function pictureSize(dataUrl: string): Promise<{ w: number; h: number } | undefined> {
  return new Promise((resolve) => {
    const img = new Image()
    img.onload = () => resolve({ w: img.naturalWidth, h: img.naturalHeight })
    img.onerror = () => resolve(undefined)
    img.src = dataUrl
  })
}

export function SendToChatModal({ item, onClose }: { item: Sendable; onClose: () => void }) {
  const v = useChat()
  const { toast, info, activeAccount, go } = useStore()
  const [to, setTo] = useState('')
  const [note, setNote] = useState('')
  const [busy, setBusy] = useState<string | null>(null)
  const chats = [
    ...v.groups.map((g) => ({ conv: g.conv, name: g.name, group: true, uuid: '' })),
    ...v.friends.friends.map((f) => ({ conv: f.conv ?? dmId(v.me, f.uuid), name: f.name, group: false, uuid: f.uuid }))
  ]
  const needsFiles = item.kind === 'files' || item.kind === 'instance'
  const limit = v.maxUploadMb * 1_000_000
  const tooBig = item.kind === 'files' && item.files.some((f) => (f.size ?? 0) > limit)
  // Why it can't go: no chat for this account, or this relay doesn't take files.
  const blocked = v.status !== 'ready' ? t('chat.send.noChat') : needsFiles && !v.drive ? t('chat.send.noFiles') : tooBig ? t('chat.files.tooBig', { size: v.maxUploadMb }) : ''

  const upload = async (conv: string, path: string, image?: boolean, size?: { w: number; h: number }) =>
    chat.uploadPath(conv, path, image ? 'image' : 'file', image && size ? { w: size.w, h: size.h } : undefined)

  const send = async () => {
    const target = chats.find((c) => c.conv === to)
    if (!target) return
    const text = note.trim()
    setBusy(t('chat.send.sending'))
    try {
      if (item.kind === 'link') {
        await chat.send(to, text ? `${text}\n${item.url}` : item.url)
      } else if (item.kind === 'store') {
        await chat.share(to, { type: 'store', product: item.product }, text)
      } else if (item.kind === 'files') {
        const sent = []
        for (const f of item.files) {
          let size: { w: number; h: number } | undefined
          if (f.image && f.measure) size = await f.measure().catch(() => undefined)
          sent.push(await upload(to, f.path, f.image, size))
        }
        await chat.sendFiles(to, text, sent)
      } else {
        // An instance: as a .mrpack in the launcher's cache, then like any file.
        if (!info) throw new Error(t('chat.send.noChat'))
        setBusy(t('chat.send.exporting'))
        const own = item.inst.ownPack
        const include = (await api.content.exportPreview(item.inst.id)).filter((e) => e.suggested).map((e) => e.name)
        const r = await api.content.exportModpackTo(
          item.inst.id,
          {
            format: 'mrpack',
            name: own?.name || item.inst.modpack?.name || item.inst.name,
            version: own?.version || '1.0.0',
            author: own?.author || activeAccount?.username || '',
            summary: own?.summary ?? '',
            include
          },
          dataPath(info.dataDir, 'cache', 'chat-send')
        )
        setBusy(t('chat.send.sending'))
        await chat.sendFiles(to, text, [await upload(to, r.path)])
      }
      toast(t('chat.share.sent', { name: target.name }), 'success', {
        label: t('chat.openChat'),
        run: () => {
          showChat(to)
          go({ page: 'inbox' })
        }
      })
      onClose()
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(null)
    }
  }

  return (
    <Modal
      title={t('chat.send.title', { item: item.title })}
      subtitle={item.kind === 'instance' ? t('chat.send.instanceHint') : undefined}
      onClose={() => !busy && onClose()}
      footer={
        <>
          {busy && (
            <span className="hint row chat-send-busy" role="status">
              <Loader2 className="spin" size={14} /> {busy}
            </span>
          )}
          <div className="spacer" />
          <button className="btn ghost" onClick={onClose} disabled={!!busy}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={!to || !!busy || !!blocked} onClick={() => void send()}>
            <Send /> {t('chat.send.send')}
          </button>
        </>
      }
    >
      <div className="chat-gift">
        {blocked ? (
          <p className="hint bad">{blocked}</p>
        ) : chats.length === 0 ? (
          <p className="hint">{t('chat.share.none')}</p>
        ) : (
          <div className="chat-pick-list" role="radiogroup" aria-label={t('chat.send.to')}>
            {chats.map((c) => (
              <label key={c.conv} className="chat-pick-row">
                <input type="radio" name="send-to" checked={to === c.conv} onChange={() => setTo(c.conv)} disabled={!!busy} />
                {c.group ? <GroupTile name={c.name} size={24} /> : <PlayerHead who={c.uuid} name={c.name} size={24} className="chat-head" />}
                <span className="ellipsis">{c.name}</span>
              </label>
            ))}
          </div>
        )}
        {!blocked && (
          <label className="field">
            <span>{t('chat.gift.note')}</span>
            <input className="input" maxLength={500} value={note} onChange={(e) => setNote(e.target.value)} placeholder={t('chat.share.placeholder')} disabled={!!busy} />
          </label>
        )}
      </div>
    </Modal>
  )
}
