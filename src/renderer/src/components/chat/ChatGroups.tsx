// Group chats in the Inbox's Chat tab: the group's tile, a new group, adding friends, the members panel (invite code,
// roles, rename, leave) and the notes a group's chat shows (made, joined, left, …).
import { useRef, useState } from 'react'
import { Ban, Clock, Copy, Crown, Gift, LogOut, MoreHorizontal, Pencil, RefreshCw, Shield, Smile, UserPlus, Users, X } from 'lucide-react'
import type { ChatGroup, ChatMessage, ChatSystemData } from '@shared/types'
import { chat, displayName, nameOf, useChat } from '../../chatState'
import { ChatAvatar, FileImage } from './ChatMedia'
import { useStore } from '../../store'
import { t, type Key } from '../../i18n'
import { ConfirmModal, Modal, openContextMenu, type ContextEntry } from '../ui'
import PlayerHead from '../PlayerHead'
import { CropModal } from '../ImagePicker'

export function GroupTile({ name, size = 32 }: { name: string; size?: number }) {
  const letters = name
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((w) => [...w][0]?.toUpperCase())
    .join('')
  return (
    <span className="chat-group-tile" style={{ width: size, height: size, fontSize: size * 0.38 }} aria-hidden>
      {letters || <Users size={size * 0.5} />}
    </span>
  )
}

/** Friends to pick (checkboxes), for a new group or adding to one. */
function FriendPicker({ picked, setPicked, skip = [] }: { picked: string[]; setPicked: (p: string[]) => void; skip?: string[] }) {
  const v = useChat()
  const list = v.friends.friends.filter((f) => !skip.includes(f.uuid))
  if (!list.length) return <p className="hint">{t('chat.group.noFriendsToAdd')}</p>
  return (
    <div className="chat-pick-list">
      {list.map((f) => (
        <label key={f.uuid} className="chat-pick-row">
          <input type="checkbox" checked={picked.includes(f.uuid)} onChange={(e) => setPicked(e.target.checked ? [...picked, f.uuid] : picked.filter((x) => x !== f.uuid))} />
          <PlayerHead who={f.uuid} name={f.name} size={24} className="chat-head" />
          <span className="ellipsis">{f.name}</span>
        </label>
      ))}
    </div>
  )
}

export function NewGroupModal({ onClose }: { onClose: () => void }) {
  const { toast } = useStore()
  const [name, setName] = useState('')
  const [picked, setPicked] = useState<string[]>([])
  const [busy, setBusy] = useState(false)
  const make = async () => {
    setBusy(true)
    try {
      const conv = await chat.createGroup(name, picked)
      chat.open(conv)
      onClose()
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(false)
    }
  }
  return (
    <Modal
      title={t('chat.group.new')}
      subtitle={t('chat.group.newHint')}
      onClose={onClose}
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={busy || !name.trim()} onClick={() => void make()}>
            <Users /> {t('chat.group.create')}
          </button>
        </>
      }
    >
      <div className="chat-gift">
        <label className="field">
          <span>{t('chat.group.name')}</span>
          <input className="input" autoFocus maxLength={50} value={name} onChange={(e) => setName(e.target.value)} placeholder={t('chat.group.namePlaceholder')} />
        </label>
        <div className="field">
          <span>{t('chat.group.pickFriends', { count: picked.length })}</span>
          <FriendPicker picked={picked} setPicked={setPicked} />
        </div>
      </div>
    </Modal>
  )
}

function AddMembersModal({ g, onClose }: { g: ChatGroup; onClose: () => void }) {
  const { toast } = useStore()
  const [picked, setPicked] = useState<string[]>([])
  return (
    <Modal
      title={t('chat.group.addTitle', { name: g.name })}
      onClose={onClose}
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button
            className="btn primary"
            disabled={!picked.length}
            onClick={() =>
              void chat.group('add', g.conv, { members: picked }).then(
                () => onClose(),
                (e) => toast((e as Error).message, 'error')
              )
            }
          >
            <UserPlus /> {t('chat.group.add')}
          </button>
        </>
      }
    >
      <FriendPicker picked={picked} setPicked={setPicked} skip={g.members.map((m) => m.uuid)} />
    </Modal>
  )
}

/** The members panel at the side of a group's chat. */
export function GroupPanel({ g, onClose, onProfile, onNick, onGift }: { g: ChatGroup; onClose: () => void; onProfile: (uuid: string) => void; onNick: (uuid: string) => void; onGift: (uuid: string) => void }) {
  const { toast } = useStore()
  const v = useChat()
  const [adding, setAdding] = useState(false)
  const [renaming, setRenaming] = useState<string | null>(null)
  const [leaving, setLeaving] = useState(false)
  const boss = g.role === 'owner' || g.role === 'admin'
  const fail = (e: unknown) => toast((e as Error).message, 'error')
  const [emojiName, setEmojiName] = useState('')
  const emojiInput = useRef<HTMLInputElement>(null)
  // A picture for an emoji: cropped to a square and made 128 px (any size comes in); a small GIF can stay as it is,
  // moving (cropping makes a still picture).
  const [emojiCrop, setEmojiCrop] = useState<{ src: string; file: File } | null>(null)
  const pickEmoji = (file: File | undefined) => {
    if (emojiInput.current) emojiInput.current.value = ''
    const name = emojiName.trim().toLowerCase()
    if (!file) return
    if (!/^[a-z0-9_]{2,32}$/.test(name)) {
      toast(t('chat.emoji.nameRule'), 'error')
      return
    }
    if (!/^image\/(png|jpeg|gif|webp)$/.test(file.type)) {
      toast(t('chat.emoji.fileType'), 'error')
      return
    }
    if (file.size > 15 * 1024 * 1024) {
      toast(t('art.tooBig'), 'error')
      return
    }
    const reader = new FileReader()
    reader.onload = () => typeof reader.result === 'string' && setEmojiCrop({ src: reader.result, file })
    reader.onerror = () => toast(t('chat.emoji.fileType'), 'error')
    reader.readAsDataURL(file)
  }
  const addEmoji = async (file: Blob, fileName: string) => {
    const name = emojiName.trim().toLowerCase()
    setEmojiCrop(null)
    try {
      const f = await chat.uploadBlob(g.conv, file, fileName, 'emoji')
      await chat.group('emoji', g.conv, { name, file: f.id })
      setEmojiName('')
    } catch (e) {
      fail(e)
    }
  }
  // Moderation: like a Discord server's owner and admins (an admin can't act on other admins or the owner).
  const [banning, setBanning] = useState<{ uuid: string; name: string } | null>(null)
  const canModerate = (role: string) => (g.role === 'owner' && role !== 'owner') || (g.role === 'admin' && role === 'member')
  const timeoutFor = (uuid: string, minutes: number) => void chat.group('timeout', g.conv, { uuid, minutes }).catch(fail)
  const TIMEOUTS: [number, Key][] = [
    [5, 'chat.mod.min5'],
    [60, 'chat.mod.hour1'],
    [1440, 'chat.mod.day1'],
    [10080, 'chat.mod.week1']
  ]
  const memberMenu = (uuid: string, role: string, timeout?: number | null): ContextEntry[] => [
    { label: t('chat.profile.view'), icon: <Users size={14} />, onSelect: () => onProfile(uuid) },
    { label: t('chat.nickname.set'), icon: <Pencil size={14} />, onSelect: () => onNick(uuid) },
    { label: t('chat.sendGift'), icon: <Gift size={14} />, onSelect: () => onGift(uuid) },
    '-',
    g.role === 'owner' && role === 'member' && { label: t('chat.group.makeAdmin'), icon: <Shield size={14} />, onSelect: () => void chat.group('role', g.conv, { uuid, role: 'admin' }).catch(fail) },
    g.role === 'owner' && role === 'admin' && { label: t('chat.group.makeMember'), onSelect: () => void chat.group('role', g.conv, { uuid, role: 'member' }).catch(fail) },
    canModerate(role) &&
      (timeout && timeout * 1000 > Date.now()
        ? { label: t('chat.mod.untimeout'), icon: <Clock size={14} />, onSelect: () => void chat.group('untimeout', g.conv, { uuid }).catch(fail) }
        : {
            label: t('chat.mod.timeout'),
            icon: <Clock size={14} />,
            items: TIMEOUTS.map(([m, k]) => ({ label: t(k), onSelect: () => timeoutFor(uuid, m) }))
          }),
    canModerate(role) && {
      label: t('chat.mod.kick'),
      icon: <X size={14} />,
      danger: true,
      onSelect: () => void chat.group('kick', g.conv, { uuid }).catch(fail)
    },
    canModerate(role) && {
      label: t('chat.mod.ban'),
      icon: <Ban size={14} />,
      danger: true,
      onSelect: () => setBanning({ uuid, name: displayName(g.conv, uuid) || nameOf(uuid) })
    }
  ]
  const invite = async (reset: boolean) => {
    try {
      const r = await chat.group('invite', g.conv, reset ? { reset: true } : {})
      if (r.code) {
        await navigator.clipboard.writeText(r.code)
        toast(t(reset ? 'chat.group.codeChanged' : 'chat.group.codeCopied', { code: r.code }), 'success')
      }
    } catch (e) {
      fail(e)
    }
  }
  const online = g.members.filter((m) => m.online).length
  return (
    <aside className="chat-members">
      <div className="chat-members-head">
        <b className="grow">{t('chat.group.members', { count: g.members.length })}</b>
        <button className="btn ghost icon sm" aria-label={t('common.close')} onClick={onClose}>
          <X />
        </button>
      </div>
      <p className="hint">{t('chat.group.onlineCount', { count: online })}</p>
      {boss && (
        <div className="chat-invite">
          <span className="hint">{t('chat.group.inviteCode')}</span>
          <b className="data">{g.invite ?? '—'}</b>
          <button className="btn ghost icon sm" title={t('chat.group.copyInvite')} aria-label={t('chat.group.copyInvite')} onClick={() => void invite(false)}>
            <Copy />
          </button>
          <button className="btn ghost icon sm" title={t('chat.group.newInvite')} aria-label={t('chat.group.newInvite')} onClick={() => void invite(true)}>
            <RefreshCw />
          </button>
        </div>
      )}
      {boss && <p className="hint">{t('chat.group.inviteHint')}</p>}
      <div className="row wrap">
        <button className="btn sm" onClick={() => setAdding(true)}>
          <UserPlus /> {t('chat.group.add')}
        </button>
        {boss && (
          <button className="btn sm ghost" onClick={() => setRenaming(g.name)}>
            <Pencil /> {t('chat.group.rename')}
          </button>
        )}
      </div>
      <div className="chat-member-list">
        {g.members.map((m) => {
          const menu = m.uuid === v.me ? [{ label: t('chat.nickname.setMine'), icon: <Pencil size={14} />, onSelect: () => onNick(m.uuid) }] : memberMenu(m.uuid, m.role, m.timeout).filter(Boolean)
          const timedOut = !!m.timeout && m.timeout * 1000 > Date.now()
          return (
            <div key={m.uuid} className="chat-member">
              <button className="chat-avatar-btn" onClick={() => onProfile(m.uuid)} aria-label={t('chat.profile.view')}>
                <ChatAvatar uuid={m.uuid} size={28} dot status={m.status ?? (m.online ? 'online' : 'offline')} />
              </button>
              <span className="grow ellipsis chat-name" onClick={() => onProfile(m.uuid)}>
                {displayName(g.conv, m.uuid) || m.name || nameOf(m.uuid)}
                {m.uuid === v.me && <span className="hint"> ({t('chat.you')})</span>}
              </span>
              {timedOut && (
                <Clock size={14} className="chat-role timeout" aria-label={t('chat.mod.timedOutUntil', { when: new Date(m.timeout! * 1000).toLocaleString() })}>
                  <title>{t('chat.mod.timedOutUntil', { when: new Date(m.timeout! * 1000).toLocaleString() })}</title>
                </Clock>
              )}
              {m.role === 'owner' && <Crown size={14} className="chat-role owner" aria-label={t('chat.group.owner')} />}
              {m.role === 'admin' && <Shield size={14} className="chat-role" aria-label={t('chat.group.admin')} />}
              {menu.length > 0 && (
                <button className="btn ghost icon sm" aria-label={t('chat.more')} onClick={(e) => openContextMenu(e, menu, m.name)}>
                  <MoreHorizontal />
                </button>
              )}
            </div>
          )
        })}
      </div>
      {boss && (g.bans?.length ?? 0) > 0 && (
        <details className="chat-bans">
          <summary className="hint">
            <Ban size={14} className="inline" /> {t('chat.mod.bannedCount', { count: g.bans!.length })}
          </summary>
          {g.bans!.map((b) => (
            <div key={b.uuid} className="chat-member">
              <ChatAvatar uuid={b.uuid} size={24} />
              <span className="grow ellipsis" title={b.reason || undefined}>
                {b.name || nameOf(b.uuid)}
                {b.reason && <span className="hint"> · {b.reason}</span>}
              </span>
              <button className="btn sm ghost" onClick={() => void chat.group('unban', g.conv, { uuid: b.uuid }).catch(fail)}>
                {t('chat.mod.unban')}
              </button>
            </div>
          ))}
        </details>
      )}
      <div className="chat-members-head">
        <b className="grow">
          <Smile size={14} className="inline" /> {t('chat.emoji.groupCount', { count: g.emoji?.length ?? 0 })}
        </b>
      </div>
      {(g.emoji?.length ?? 0) > 0 && (
        <div className="chat-emoji-list">
          {g.emoji!.map((e) => (
            <span key={e.name} className="chat-emoji-item" title={`:${e.name}:`}>
              <FileImage id={e.file} alt={e.name} className="ct-emoji" />
              <span className="ellipsis">:{e.name}:</span>
              {boss && (
                <button className="btn ghost icon sm" aria-label={t('common.remove')} onClick={() => void chat.group('emoji/remove', g.conv, { name: e.name }).catch(fail)}>
                  <X />
                </button>
              )}
            </span>
          ))}
        </div>
      )}
      {boss && v.drive && (
        <div className="row">
          <input className="input grow" maxLength={32} value={emojiName} onChange={(e) => setEmojiName(e.target.value.replace(/[^a-zA-Z0-9_]/g, ''))} placeholder={t('chat.emoji.namePlaceholder')} />
          <button className="btn sm" disabled={!emojiName.trim()} onClick={() => emojiInput.current?.click()}>
            {t('chat.emoji.add')}
          </button>
          <input ref={emojiInput} type="file" accept="image/png,image/jpeg,image/gif,image/webp" hidden onChange={(e) => pickEmoji(e.target.files?.[0])} />
        </div>
      )}
      <button className="btn sm ghost danger chat-leave" onClick={() => setLeaving(true)}>
        <LogOut /> {t('chat.group.leave')}
      </button>
      {emojiCrop && (
        <CropModal
          src={emojiCrop.src}
          kind="emoji"
          onCancel={() => setEmojiCrop(null)}
          // Not fetch(): the app's connect-src doesn't allow data: URLs, so that failed without a word.
          onDone={(png) => {
            try {
              const bytes = Uint8Array.from(atob(png.slice(png.indexOf(',') + 1)), (c) => c.charCodeAt(0))
              void addEmoji(new Blob([bytes], { type: 'image/png' }), 'emoji.png')
            } catch (e) {
              fail(e)
            }
          }}
          extra={
            emojiCrop.file.type === 'image/gif' && emojiCrop.file.size <= 512_000 ? (
              <button className="btn ghost" title={t('chat.emoji.keepGifHint')} onClick={() => void addEmoji(emojiCrop.file, emojiCrop.file.name)}>
                {t('chat.emoji.keepGif')}
              </button>
            ) : undefined
          }
        />
      )}
      {adding && <AddMembersModal g={g} onClose={() => setAdding(false)} />}
      {renaming !== null && (
        <Modal
          title={t('chat.group.rename')}
          onClose={() => setRenaming(null)}
          footer={
            <>
              <button className="btn ghost" onClick={() => setRenaming(null)}>
                {t('common.cancel')}
              </button>
              <button
                className="btn primary"
                disabled={!renaming.trim()}
                onClick={() =>
                  void chat.group('rename', g.conv, { name: renaming }).then(
                    () => setRenaming(null),
                    (e) => fail(e)
                  )
                }
              >
                {t('common.save')}
              </button>
            </>
          }
        >
          <input className="input" autoFocus maxLength={50} value={renaming} onChange={(e) => setRenaming(e.target.value)} />
        </Modal>
      )}
      {banning && (
        <BanModal
          name={banning.name}
          onBan={(reason) =>
            chat.group('ban', g.conv, { uuid: banning.uuid, reason }).then(
              () => setBanning(null),
              (e) => fail(e)
            )
          }
          onClose={() => setBanning(null)}
        />
      )}
      {leaving && (
        <ConfirmModal
          title={t('chat.group.leaveTitle', { name: g.name })}
          body={g.role === 'owner' ? t('chat.group.leaveOwnerBody') : t('chat.group.leaveBody')}
          confirm={t('chat.group.leave')}
          onConfirm={async () => {
            await chat.group('leave', g.conv).catch(fail)
            setLeaving(false)
          }}
          onClose={() => setLeaving(false)}
        />
      )}
    </aside>
  )
}

/** A group note: "Steve added Alex", "Alex joined", … */
/** Ban someone from a group: an optional reason (they don't see it; owners and admins do). */
function BanModal({ name, onBan, onClose }: { name: string; onBan: (reason: string) => Promise<void>; onClose: () => void }) {
  const [reason, setReason] = useState('')
  const [busy, setBusy] = useState(false)
  return (
    <Modal
      title={t('chat.mod.banTitle', { name })}
      onClose={onClose}
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button
            className="btn danger"
            disabled={busy}
            onClick={() => {
              setBusy(true)
              void onBan(reason.trim()).finally(() => setBusy(false))
            }}
          >
            <Ban /> {t('chat.mod.ban')}
          </button>
        </>
      }
    >
      <p className="hint">{t('chat.mod.banBody')}</p>
      <input className="input" autoFocus maxLength={200} value={reason} placeholder={t('chat.mod.reason')} onChange={(e) => setReason(e.target.value)} />
    </Modal>
  )
}

export function SystemNote({ m }: { m: ChatMessage }) {
  const d = m.data as ChatSystemData | null
  if (!d) return null
  const by = nameOf(d.by)
  const who = (d.who ?? []).map(nameOf).join(', ')
  const text =
    d.event === 'created'
      ? t('chat.note.created', { by, name: d.name ?? '' })
      : d.event === 'joined'
        ? t('chat.note.joined', { by })
        : d.event === 'left'
          ? t('chat.note.left', { by })
          : d.event === 'added'
            ? t('chat.note.added', { by, who })
            : d.event === 'removed'
              ? t('chat.note.removed', { by, who })
              : d.event === 'renamed'
                ? t('chat.note.renamed', { by, name: d.name ?? '' })
                : d.event === 'nickname'
                  ? d.name
                    ? t('chat.note.nickname', { by, who, name: d.name })
                    : t('chat.note.nicknameCleared', { by, who })
                : d.event === 'timeout'
                  ? t('chat.note.timeout', { by, who, when: d.until ? new Date(d.until * 1000).toLocaleString() : '' })
                : d.event === 'untimeout'
                  ? t('chat.note.untimeout', { by, who })
                : d.event === 'banned'
                  ? t('chat.note.banned', { by, who })
                : d.role === 'admin'
                  ? t('chat.note.madeAdmin', { by, who })
                  : t('chat.note.madeMember', { by, who })
  return <div className="chat-note">{text}</div>
}
