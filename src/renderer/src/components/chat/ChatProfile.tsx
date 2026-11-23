// Profiles in chat: your status (Online, Idle, Do Not Disturb, Invisible, and a line of your own), your profile (picture,
// banner colour, bio), other players' profile cards (with mutual groups, Message, Send a gift, Set nickname), and
// nicknames, which everyone in that chat sees.
import { useEffect, useRef, useState } from 'react'
import { Ban, Gift, ImagePlus, MessageCircle, Pencil, Trash2, UserMinus, X } from 'lucide-react'
import type { ChatProfile, ChatStatus } from '@shared/types'
import { chat, displayName, dmId, nameOf, useChat } from '../../chatState'
import { useStore } from '../../store'
import { currentLanguage, t, type Key } from '../../i18n'
import { Modal, type ContextEntry } from '../ui'
import { ChatAvatar, FileImage } from './ChatMedia'
import { GroupTile } from './ChatGroups'

export const STATUS_LABEL: Record<ChatStatus, Key> = {
  online: 'chat.status.online',
  idle: 'chat.status.idle',
  dnd: 'chat.status.dnd',
  invisible: 'chat.status.invisible',
  offline: 'chat.status.offline'
}

/** The status menu under your name. */
export function statusMenu(onEdit: () => void, onText: () => void): ContextEntry[] {
  const pick = (s: Exclude<ChatStatus, 'offline'>, hint?: Key): ContextEntry => ({
    label: t(STATUS_LABEL[s]),
    hint: hint ? t(hint) : undefined,
    icon: <span className={`chat-presence inline ${s === 'invisible' ? 'offline' : s}`} />,
    onSelect: () => void chat.setStatus(s).catch(() => {})
  })
  return [
    pick('online'),
    pick('idle'),
    pick('dnd', 'chat.status.dndHint'),
    pick('invisible', 'chat.status.invisibleHint'),
    '-',
    { label: t('chat.status.setText'), icon: <Pencil size={14} />, onSelect: onText },
    { label: t('chat.profile.edit'), icon: <ImagePlus size={14} />, onSelect: onEdit }
  ]
}

export function StatusTextModal({ onClose }: { onClose: () => void }) {
  const v = useChat()
  const [text, setText] = useState(v.profile?.statusText ?? '')
  const save = (value: string) =>
    void chat
      .setStatus((v.profile?.status === 'offline' ? 'online' : v.profile?.status) ?? 'online', value)
      .then(onClose, () => onClose())
  return (
    <Modal
      title={t('chat.status.setText')}
      onClose={onClose}
      footer={
        <>
          {v.profile?.statusText && (
            <button className="btn ghost" onClick={() => save('')}>
              {t('chat.status.clear')}
            </button>
          )}
          <div className="spacer" />
          <button className="btn primary" onClick={() => save(text.trim())}>
            {t('common.save')}
          </button>
        </>
      }
    >
      <input className="input" autoFocus maxLength={80} value={text} onChange={(e) => setText(e.target.value)} placeholder={t('chat.status.textPlaceholder')} />
    </Modal>
  )
}

const BANNERS = ['', '#8b5cf6', '#ec4899', '#ef4444', '#f59e0b', '#22c55e', '#06b6d4', '#3b82f6', '#64748b']

/** Your own profile: picture, banner colour, bio. */
export function ProfileModal({ onClose }: { onClose: () => void }) {
  const v = useChat()
  const { toast } = useStore()
  const p = v.profile
  const [bio, setBio] = useState(p?.bio ?? '')
  const [banner, setBanner] = useState(p?.banner ?? '')
  const [avatar, setAvatar] = useState<string | null>(p?.avatar ?? null)
  const [busy, setBusy] = useState(false)
  const input = useRef<HTMLInputElement>(null)
  const pickPicture = async (file: File | undefined) => {
    if (!file) return
    if (!/^image\/(png|jpeg|gif|webp)$/.test(file.type) || file.size > 8_000_000) {
      toast(t('chat.profile.pictureRule'), 'error')
      return
    }
    setBusy(true)
    try {
      const f = await chat.uploadBlob('', file, file.name, 'avatar')
      setAvatar(f.id)
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(false)
    }
  }
  const save = async () => {
    setBusy(true)
    try {
      await chat.saveProfile({ bio: bio.trim(), banner, avatar })
      toast(t('chat.profile.saved'), 'success')
      onClose()
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(false)
    }
  }
  return (
    <Modal
      title={t('chat.profile.edit')}
      onClose={onClose}
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={busy} onClick={() => void save()}>
            {t('common.save')}
          </button>
        </>
      }
    >
      <div className="chat-profile-edit">
        <div className="chat-card-banner" style={{ background: banner || undefined }} />
        <div className="chat-profile-pic">
          {avatar ? <FileImage id={avatar} className="chat-avatar-img big" /> : <ChatAvatar uuid={v.me} size={72} />}
          <div className="row wrap">
            <button className="btn sm" disabled={busy || !v.drive} onClick={() => input.current?.click()} title={v.drive ? undefined : t('chat.files.off')}>
              <ImagePlus /> {t('chat.profile.picture')}
            </button>
            {avatar && (
              <button className="btn ghost sm" onClick={() => setAvatar(null)}>
                <Trash2 /> {t('chat.profile.useHead')}
              </button>
            )}
          </div>
          <input ref={input} type="file" accept="image/png,image/jpeg,image/gif,image/webp" hidden onChange={(e) => void pickPicture(e.target.files?.[0])} />
        </div>
        <div className="field">
          <span>{t('chat.profile.banner')}</span>
          <div className="row wrap chat-swatches">
            {BANNERS.map((c) => (
              <button key={c || 'none'} type="button" className={`chat-swatch ${banner === c ? 'on' : ''}`} style={{ background: c || 'var(--surface-2)' }} aria-label={c || t('chat.profile.noBanner')} onClick={() => setBanner(c)} />
            ))}
          </div>
        </div>
        <label className="field">
          <span>{t('chat.profile.bio')}</span>
          <textarea className="input" rows={3} maxLength={190} value={bio} onChange={(e) => setBio(e.target.value)} placeholder={t('chat.profile.bioPlaceholder')} />
        </label>
      </div>
    </Modal>
  )
}

/** What someone is called in a chat (everyone in it sees the nickname). */
export function NicknameModal({ conv, uuid, onClose }: { conv: string; uuid: string; onClose: () => void }) {
  const { toast } = useStore()
  const [nick, setNick] = useState(useChat().nicknames[conv]?.[uuid] ?? '')
  const save = (value: string) =>
    void chat.setNickname(conv, uuid, value).then(onClose, (e) => {
      toast((e as Error).message, 'error')
    })
  return (
    <Modal
      title={t('chat.nickname.title', { name: nameOf(uuid) })}
      subtitle={t('chat.nickname.hint')}
      onClose={onClose}
      footer={
        <>
          <button className="btn ghost" onClick={() => save('')}>
            {t('chat.nickname.reset')}
          </button>
          <div className="spacer" />
          <button className="btn primary" onClick={() => save(nick.trim())}>
            {t('common.save')}
          </button>
        </>
      }
    >
      <input className="input" autoFocus maxLength={32} value={nick} onChange={(e) => setNick(e.target.value)} placeholder={nameOf(uuid)} />
    </Modal>
  )
}

/** Someone's profile card, opened from their name or picture. `conv`: the chat it was opened from (for nicknames). */
export function ProfileCard({ uuid, conv, onClose, onGift }: { uuid: string; conv: string | null; onClose: () => void; onGift?: () => void }) {
  const v = useChat()
  const { toast } = useStore()
  const [p, setP] = useState<ChatProfile | null>(null)
  const [nick, setNick] = useState(false)
  useEffect(() => {
    void chat.profile(uuid).then(setP, (e) => toast((e as Error).message, 'error'))
  }, [uuid, toast])
  const shown = displayName(conv, uuid)
  const status = p?.status ?? 'offline'
  return (
    <Modal title={shown} onClose={onClose}>
      <div className="chat-card">
        <div className="chat-card-banner" style={{ background: p?.banner || undefined }} />
        <div className="chat-card-head">
          <ChatAvatar uuid={uuid} size={72} dot status={status === 'invisible' ? 'offline' : status} />
          <span className="grow">
            <b>{shown}</b>
            {shown !== (p?.name ?? nameOf(uuid)) && <span className="hint"> {p?.name ?? nameOf(uuid)}</span>}
            <span className="hint">
              {t(STATUS_LABEL[status])}
              {p?.statusText ? ` · ${p.statusText}` : ''}
            </span>
          </span>
        </div>
        {p?.bio && <p className="chat-card-bio">{p.bio}</p>}
        {p && (
          <p className="hint">
            {t('chat.profile.since', { date: new Date(p.since * 1000).toLocaleDateString(currentLanguage(), { dateStyle: 'medium' }) })}
            {p.mutualGroups.length > 0 && ` · ${t('chat.profile.mutual', { groups: p.mutualGroups.map((g) => g.name).join(', ') })}`}
          </p>
        )}
        {uuid !== v.me && (
          <div className="row wrap">
            {p?.friend && (
              <button
                className="btn sm primary"
                onClick={() => {
                  chat.open(dmId(v.me, uuid))
                  onClose()
                }}
              >
                <MessageCircle /> {t('chat.profile.message')}
              </button>
            )}
            {onGift && (
              <button className="btn sm" onClick={onGift}>
                <Gift /> {t('chat.sendGift')}
              </button>
            )}
            {conv && (
              <button className="btn sm ghost" onClick={() => setNick(true)}>
                <Pencil /> {t('chat.nickname.set')}
              </button>
            )}
          </div>
        )}
        {uuid === v.me && conv && (
          <button className="btn sm ghost" onClick={() => setNick(true)}>
            <Pencil /> {t('chat.nickname.setMine')}
          </button>
        )}
      </div>
      {nick && conv && <NicknameModal conv={conv} uuid={uuid} onClose={() => setNick(false)} />}
    </Modal>
  )
}

/**
 * The Inbox's right-hand panel for a friend's chat: their banner, picture, status, bio, how long you've been friends,
 * the groups you share (click: open it) and what you can do (gift, nickname, remove, block).
 */
export function ProfileSide({ uuid, conv, onClose, onGift, onRemove, onBlock }: { uuid: string; conv: string; onClose: () => void; onGift?: () => void; onRemove?: () => void; onBlock?: () => void }) {
  const { toast } = useStore()
  const [p, setP] = useState<ChatProfile | null>(null)
  const [nick, setNick] = useState(false)
  useEffect(() => {
    setP(null)
    void chat.profile(uuid).then(setP, (e) => toast((e as Error).message, 'error'))
  }, [uuid, toast])
  const shown = displayName(conv, uuid)
  const status = p?.status ?? 'offline'
  return (
    <aside className="chat-members chat-profile-side" aria-label={shown}>
      <div className="chat-side-banner" style={{ background: p?.banner || undefined }}>
        <button className="btn ghost icon sm" aria-label={t('common.close')} title={t('common.close')} onClick={onClose}>
          <X />
        </button>
      </div>
      <div className="chat-side-who">
        <ChatAvatar uuid={uuid} size={72} dot status={status === 'invisible' ? 'offline' : status} />
        <b>{shown}</b>
        {shown !== (p?.name ?? nameOf(uuid)) && <span className="hint">{p?.name ?? nameOf(uuid)}</span>}
        <span className="hint">
          {t(STATUS_LABEL[status])}
          {p?.statusText ? ` · ${p.statusText}` : ''}
        </span>
      </div>
      {p?.bio && (
        <section className="chat-side-sect">
          <h4>{t('chat.side.about')}</h4>
          <p className="chat-card-bio">{p.bio}</p>
        </section>
      )}
      {p && (
        <section className="chat-side-sect">
          <h4>{t('chat.side.mutual', { count: p.mutualGroups.length })}</h4>
          {p.mutualGroups.length === 0 ? (
            <span className="hint">{t('chat.side.noMutual')}</span>
          ) : (
            p.mutualGroups.map((g) => (
              <button key={g.conv} className="chat-side-link" onClick={() => chat.open(g.conv)}>
                <GroupTile name={g.name} size={26} />
                <span className="ellipsis">{g.name}</span>
              </button>
            ))
          )}
          <span className="hint">{t('chat.profile.since', { date: new Date(p.since * 1000).toLocaleDateString(currentLanguage(), { dateStyle: 'medium' }) })}</span>
        </section>
      )}
      <section className="chat-side-sect chat-side-acts">
        {onGift && (
          <button className="btn sm" onClick={onGift}>
            <Gift /> {t('chat.sendGift')}
          </button>
        )}
        <button className="btn sm" onClick={() => setNick(true)}>
          <Pencil /> {t('chat.nickname.set')}
        </button>
        {onRemove && (
          <button className="btn sm ghost" onClick={onRemove}>
            <UserMinus /> {t('chat.removeFriend')}
          </button>
        )}
        {onBlock && (
          <button className="btn sm ghost danger" onClick={onBlock}>
            <Ban /> {t('chat.block')}
          </button>
        )}
      </section>
      {nick && <NicknameModal conv={conv} uuid={uuid} onClose={() => setNick(false)} />}
    </aside>
  )
}

