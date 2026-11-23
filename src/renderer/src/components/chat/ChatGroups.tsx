// Group chats in the Inbox's Chat tab: the group's tile, a new group, adding friends, the members panel (invite code,
// roles, rename, leave) and the notes a group's chat shows (made, joined, left, …).
import { useRef, useState } from 'react'
import { Copy, Crown, Gift, LogOut, MoreHorizontal, Pencil, RefreshCw, Shield, Smile, UserPlus, Users, X } from 'lucide-react'
import type { ChatGroup, ChatMessage, ChatSystemData } from '@shared/types'
import { chat, displayName, nameOf, useChat } from '../../chatState'
import { ChatAvatar, FileImage } from './ChatMedia'
import { useStore } from '../../store'
import { t } from '../../i18n'
import { ConfirmModal, Modal, openContextMenu, type ContextEntry } from '../ui'
import PlayerHead from '../PlayerHead'

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
  const addEmoji = async (file: File | undefined) => {
    const name = emojiName.trim().toLowerCase()
    if (!file) return
    if (!/^[a-z0-9_]{2,32}$/.test(name)) {
      toast(t('chat.emoji.nameRule'), 'error')
      return
    }
    if (!/^image\/(png|jpeg|gif|webp)$/.test(file.type) || file.size > 512_000) {
      toast(t('chat.emoji.fileRule'), 'error')
      return
    }
    try {
      const f = await chat.uploadBlob(g.conv, file, file.name, 'emoji')
      await chat.group('emoji', g.conv, { name, file: f.id })
      setEmojiName('')
    } catch (e) {
      fail(e)
    }
  }
  const memberMenu = (uuid: string, role: string): ContextEntry[] => [
    { label: t('chat.profile.view'), icon: <Users size={14} />, onSelect: () => onProfile(uuid) },
    { label: t('chat.nickname.set'), icon: <Pencil size={14} />, onSelect: () => onNick(uuid) },
    { label: t('chat.sendGift'), icon: <Gift size={14} />, onSelect: () => onGift(uuid) },
    '-',
    g.role === 'owner' && role === 'member' && { label: t('chat.group.makeAdmin'), icon: <Shield size={14} />, onSelect: () => void chat.group('role', g.conv, { uuid, role: 'admin' }).catch(fail) },
    g.role === 'owner' && role === 'admin' && { label: t('chat.group.makeMember'), onSelect: () => void chat.group('role', g.conv, { uuid, role: 'member' }).catch(fail) },
    ((g.role === 'owner' && role !== 'owner') || (g.role === 'admin' && role === 'member')) && {
      label: t('chat.group.remove'),
      icon: <X size={14} />,
      danger: true,
      onSelect: () => void chat.group('remove', g.conv, { uuid }).catch(fail)
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
          const menu = m.uuid === v.me ? [{ label: t('chat.nickname.setMine'), icon: <Pencil size={14} />, onSelect: () => onNick(m.uuid) }] : memberMenu(m.uuid, m.role).filter(Boolean)
          return (
            <div key={m.uuid} className="chat-member">
              <button className="chat-avatar-btn" onClick={() => onProfile(m.uuid)} aria-label={t('chat.profile.view')}>
                <ChatAvatar uuid={m.uuid} size={28} dot status={m.status ?? (m.online ? 'online' : 'offline')} />
              </button>
              <span className="grow ellipsis chat-name" onClick={() => onProfile(m.uuid)}>
                {displayName(g.conv, m.uuid) || m.name || nameOf(m.uuid)}
                {m.uuid === v.me && <span className="hint"> ({t('chat.you')})</span>}
              </span>
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
          <input ref={emojiInput} type="file" accept="image/png,image/jpeg,image/gif,image/webp" hidden onChange={(e) => void addEmoji(e.target.files?.[0])} />
        </div>
      )}
      <button className="btn sm ghost danger chat-leave" onClick={() => setLeaving(true)}>
        <LogOut /> {t('chat.group.leave')}
      </button>
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
                : d.role === 'admin'
                  ? t('chat.note.madeAdmin', { by, who })
                  : t('chat.note.madeMember', { by, who })
  return <div className="chat-note">{text}</div>
}
