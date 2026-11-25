// "Send to chat" from anywhere in the launcher: pick friends' chats and groups (one or several), add a note, send.
// What can go:
// - a mod, modpack, shader or resource pack: its share link (rscraft.resincore.tech/mod/…), which shows as a card with
//   Open (Embeds.tsx ModCard); a file with no project behind it goes as the file;
// - an instance: packed with PRAR into `<name>.resinpack` (backup.rs share_instance: its own mods, resource packs,
//   shaders, configs and the other suggested folders), which the receiver adds from the chat. The file is made once
//   for every chat it goes to, and removed from this PC once it's sent (or failed);
// - files: screenshots (as pictures) and config or other files from an instance's Files tab;
// - a Store item: its card with the price.
// One host (SendToChatHost, in App) shows the window; anything opens it with sendToChat(item).
import { useEffect, useState } from 'react'
import { Send } from 'lucide-react'
import type { ChatFile, Instance } from '@shared/types'
import { api } from '../../api'
import { chat, dmId, showChat, useChat } from '../../chatState'
import { useStore } from '../../store'
import { t } from '../../i18n'
import { Modal } from '../ui'
import { GroupTile } from './ChatGroups'
import PlayerHead from '../PlayerHead'
// Its list and tiles are chat's: loaded here too, since this opens from anywhere before chat ever has.
import '../../chat.css'

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
  const { toast, go } = useStore()
  const [to, setTo] = useState<string[]>([])
  const [note, setNote] = useState('')
  const chats = [
    ...v.groups.map((g) => ({ conv: g.conv, name: g.name, group: true, uuid: '' })),
    ...v.friends.friends.map((f) => ({ conv: f.conv ?? dmId(v.me, f.uuid), name: f.name, group: false, uuid: f.uuid }))
  ]
  const toggle = (conv: string) => setTo((list) => (list.includes(conv) ? list.filter((c) => c !== conv) : [...list, conv]))
  const needsFiles = item.kind === 'files' || item.kind === 'instance'
  const limit = v.maxUploadMb * 1_000_000
  const tooBig = item.kind === 'files' && item.files.some((f) => (f.size ?? 0) > limit)
  // Why it can't go: no chat for this account, or this relay doesn't take files.
  const blocked = v.status !== 'ready' ? t('chat.send.noChat') : needsFiles && !v.drive ? t('chat.send.noFiles') : tooBig ? t('chat.files.tooBig', { size: v.maxUploadMb }) : ''

  // Send closes the window at once: packing an instance takes a while, and it shows its progress where tasks do (the
  // instance page and the tasks corner). A toast says which chats have it, or why some don't.
  const send = () => {
    const targets = chats.filter((c) => to.includes(c.conv))
    if (targets.length === 0) return
    const text = note.trim()
    const names = targets.map((c) => c.name).join(', ')
    onClose()
    if (item.kind === 'instance' || item.kind === 'files') toast(t('chat.send.inBackground', { item: item.title, name: names }), 'info')
    void (async () => {
      // An instance is packed once, for every chat, and the file goes once it's sent everywhere it can.
      let packed: string | null = null
      try {
        let files: SendFile[] = item.kind === 'files' ? item.files : []
        if (item.kind === 'instance') {
          const r = await api.backup.shareInstance(item.inst.id)
          packed = r.path
          if (r.bytes > limit) throw new Error(t('chat.send.packTooBig', { size: Math.ceil(r.bytes / 1_000_000), max: v.maxUploadMb }))
          files = [{ path: r.path }]
        }
        // A picture's size is read once, for every chat.
        const sizes = new Map<string, { w: number; h: number } | undefined>()
        for (const f of files) if (f.image && f.measure) sizes.set(f.path, await f.measure().catch(() => undefined))
        const sendTo = async (conv: string) => {
          if (item.kind === 'link') return chat.send(conv, text ? `${text}\n${item.url}` : item.url)
          if (item.kind === 'store') return chat.share(conv, { type: 'store', product: item.product }, text)
          const sent: ChatFile[] = []
          for (const f of files) {
            const size = sizes.get(f.path)
            sent.push(await chat.uploadPath(conv, f.path, f.image ? 'image' : 'file', f.image && size ? { w: size.w, h: size.h } : undefined))
          }
          return chat.sendFiles(conv, text, sent)
        }
        const done: typeof targets = []
        const failed: { name: string; why: string }[] = []
        for (const c of targets) {
          try {
            await sendTo(c.conv)
            done.push(c)
          } catch (e) {
            failed.push({ name: c.name, why: (e as Error).message })
          }
        }
        return { done, failed }
      } finally {
        if (packed) await api.backup.dropShare(packed).catch(() => {})
      }
    })().then(
      ({ done, failed }) => {
        if (done.length > 0)
          toast(t('chat.share.sent', { name: done.map((c) => c.name).join(', ') }), 'success', {
            label: t('chat.openChat'),
            run: () => {
              showChat(done[0].conv)
              go({ page: 'inbox' })
            }
          })
        for (const f of failed) toast(t('chat.send.failedTo', { item: item.title, name: f.name, why: f.why }), 'error')
      },
      (e) => toast(t('chat.send.failed', { item: item.title, why: (e as Error).message }), 'error')
    )
  }

  return (
    <Modal
      title={t('chat.send.title', { item: item.title })}
      subtitle={item.kind === 'instance' ? t('chat.send.instanceHintPack') : undefined}
      onClose={onClose}
      footer={
        <>
          {to.length > 1 && <span className="hint">{t('chat.send.chosen', { count: to.length })}</span>}
          <div className="spacer" />
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={to.length === 0 || !!blocked} onClick={send}>
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
          <div className="chat-pick-list" role="group" aria-label={t('chat.send.to')}>
            {chats.map((c) => (
              <label key={c.conv} className="chat-pick-row">
                <input type="checkbox" aria-label={c.name} checked={to.includes(c.conv)} onChange={() => toggle(c.conv)} />
                {c.group ? <GroupTile name={c.name} size={24} /> : <PlayerHead who={c.uuid} name={c.name} size={24} className="chat-head" />}
                <span className="ellipsis">{c.name}</span>
              </label>
            ))}
          </div>
        )}
        {!blocked && (
          <label className="field">
            <span>{t('chat.gift.note')}</span>
            <input className="input" maxLength={500} value={note} onChange={(e) => setNote(e.target.value)} placeholder={t('chat.share.placeholder')} />
          </label>
        )}
      </div>
    </Modal>
  )
}
