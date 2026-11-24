// The emoji manager: every emoji of the groups you're in (they work in every chat), searchable, by group. A group's
// owner and admins add (pick, crop to a square, 128 px), rename and remove its emoji here or in its members panel.
import { useRef, useState } from 'react'
import { Check, Copy, Pencil, Search, Smile, Trash2, X } from 'lucide-react'
import type { ChatGroup } from '@shared/types'
import { chat, useChat } from '../../chatState'
import { useStore } from '../../store'
import { t } from '../../i18n'
import { Empty, Modal } from '../ui'
import { CropModal } from '../ImagePicker'
import { FileImage } from './ChatMedia'
import { GroupTile } from './ChatGroups'

const MAX = 50
const NAME = /^[a-z0-9_]{2,32}$/

/**
 * Adding an emoji to a group: its name, then a picture, cropped to a square and made 128 px (any size comes in); a
 * small GIF can stay as it is, moving (cropping makes a still picture).
 */
export function EmojiAdd({ conv }: { conv: string }) {
  const { toast } = useStore()
  const [name, setName] = useState('')
  const input = useRef<HTMLInputElement>(null)
  const [crop, setCrop] = useState<{ src: string; file: File } | null>(null)
  const [busy, setBusy] = useState(false)
  const fail = (e: unknown) => toast((e as Error).message, 'error')
  const pick = (file: File | undefined) => {
    if (input.current) input.current.value = ''
    if (!file) return
    if (!NAME.test(name.trim().toLowerCase())) return toast(t('chat.emoji.nameRule'), 'error')
    if (!/^image\/(png|jpeg|gif|webp)$/.test(file.type)) return toast(t('chat.emoji.fileType'), 'error')
    if (file.size > 15 * 1024 * 1024) return toast(t('art.tooBig'), 'error')
    const reader = new FileReader()
    reader.onload = () => typeof reader.result === 'string' && setCrop({ src: reader.result, file })
    reader.onerror = () => toast(t('chat.emoji.fileType'), 'error')
    reader.readAsDataURL(file)
  }
  const add = async (file: Blob, fileName: string) => {
    const n = name.trim().toLowerCase()
    setCrop(null)
    setBusy(true)
    try {
      const f = await chat.uploadBlob(conv, file, fileName, 'emoji')
      await chat.group('emoji', conv, { name: n, file: f.id })
      setName('')
    } catch (e) {
      fail(e)
    } finally {
      setBusy(false)
    }
  }
  return (
    <div className="row emoji-add">
      <input
        className="input grow"
        maxLength={32}
        value={name}
        onChange={(e) => setName(e.target.value.replace(/[^a-zA-Z0-9_]/g, ''))}
        placeholder={t('chat.emoji.namePlaceholder')}
        aria-label={t('chat.emoji.namePlaceholder')}
      />
      <button className="btn sm" disabled={!name.trim() || busy} onClick={() => input.current?.click()}>
        {t('chat.emoji.add')}
      </button>
      <input ref={input} type="file" accept="image/png,image/jpeg,image/gif,image/webp" hidden onChange={(e) => pick(e.target.files?.[0])} />
      {crop && (
        <CropModal
          src={crop.src}
          kind="emoji"
          onCancel={() => setCrop(null)}
          // Not fetch(): the app's connect-src doesn't allow data: URLs.
          onDone={(png) => {
            try {
              const bytes = Uint8Array.from(atob(png.slice(png.indexOf(',') + 1)), (c) => c.charCodeAt(0))
              void add(new Blob([bytes], { type: 'image/png' }), 'emoji.png')
            } catch (e) {
              fail(e)
            }
          }}
          extra={
            crop.file.type === 'image/gif' && crop.file.size <= 512_000 ? (
              <button className="btn ghost" title={t('chat.emoji.keepGifHint')} onClick={() => void add(crop.file, crop.file.name)}>
                {t('chat.emoji.keepGif')}
              </button>
            ) : undefined
          }
        />
      )}
    </div>
  )
}

/** One emoji: its picture and :name:; copy its name; the group's owner and admins rename or remove it. */
function EmojiTile({ g, e, boss }: { g: ChatGroup; e: { name: string; file: string }; boss: boolean }) {
  const { toast } = useStore()
  const [renaming, setRenaming] = useState<string | null>(null)
  const fail = (err: unknown) => toast((err as Error).message, 'error')
  const save = async () => {
    const to = (renaming ?? '').trim().toLowerCase()
    if (to === e.name) return setRenaming(null)
    if (!NAME.test(to)) return toast(t('chat.emoji.nameRule'), 'error')
    try {
      await chat.group('emoji/rename', g.conv, { name: e.name, to })
      setRenaming(null)
    } catch (err) {
      fail(err)
    }
  }
  return (
    <div className="em-tile">
      <FileImage id={e.file} alt={`:${e.name}:`} className="em-pic" />
      {renaming !== null ? (
        <form
          className="em-rename"
          onSubmit={(ev) => {
            ev.preventDefault()
            void save()
          }}
        >
          <input
            className="input"
            autoFocus
            maxLength={32}
            value={renaming}
            onChange={(ev) => setRenaming(ev.target.value.replace(/[^a-zA-Z0-9_]/g, ''))}
            onKeyDown={(ev) => ev.key === 'Escape' && (ev.stopPropagation(), setRenaming(null))}
            aria-label={t('chat.emoji.rename')}
          />
          <button type="submit" className="btn ghost icon sm" aria-label={t('common.save')}>
            <Check />
          </button>
          <button type="button" className="btn ghost icon sm" aria-label={t('common.cancel')} onClick={() => setRenaming(null)}>
            <X />
          </button>
        </form>
      ) : (
        <>
          <span className="em-name ellipsis" title={`:${e.name}:`}>
            :{e.name}:
          </span>
          <div className="em-actions">
            <button
              className="btn ghost icon sm"
              title={t('chat.emoji.copyName')}
              aria-label={t('chat.emoji.copyName')}
              onClick={() => void navigator.clipboard.writeText(`:${e.name}:`).then(() => toast(t('chat.emoji.copied', { name: `:${e.name}:` }), 'success'))}
            >
              <Copy />
            </button>
            {boss && (
              <>
                <button className="btn ghost icon sm" title={t('chat.emoji.rename')} aria-label={t('chat.emoji.rename')} onClick={() => setRenaming(e.name)}>
                  <Pencil />
                </button>
                <button
                  className="btn ghost icon sm danger"
                  title={t('common.remove')}
                  aria-label={t('common.remove')}
                  onClick={() => void chat.group('emoji/remove', g.conv, { name: e.name }).catch(fail)}
                >
                  <Trash2 />
                </button>
              </>
            )}
          </div>
        </>
      )}
    </div>
  )
}

export default function EmojiManager({ onClose }: { onClose: () => void }) {
  const v = useChat()
  const [query, setQuery] = useState('')
  const q = query.trim().toLowerCase().replace(/:/g, '')
  // Groups with emoji, and the ones you can add to (owner or admin), in the order you're in them.
  const groups = v.groups.filter((g) => (g.emoji?.length ?? 0) > 0 || g.role === 'owner' || g.role === 'admin')
  const total = v.groups.reduce((n, g) => n + (g.emoji?.length ?? 0), 0)
  return (
    <Modal title={t('chat.emoji.managerTitle')} subtitle={t('chat.emoji.managerLead')} onClose={onClose} wide>
      {groups.length === 0 ? (
        <Empty icon={<Smile />} title={t('chat.emoji.noGroups')}>
          {t('chat.emoji.noGroupsHint')}
        </Empty>
      ) : (
        <>
          {total > 0 && (
            <div className="em-search">
              <Search aria-hidden />
              <input className="input" placeholder={t('chat.emoji.search')} value={query} onChange={(e) => setQuery(e.target.value)} />
            </div>
          )}
          <div className="em-groups">
            {groups.map((g) => {
              const boss = g.role === 'owner' || g.role === 'admin'
              const list = (g.emoji ?? []).filter((e) => !q || e.name.includes(q))
              if (q && !list.length) return null
              return (
                <section key={g.conv} className="em-group">
                  <div className="em-group-head">
                    <GroupTile name={g.name} size={26} />
                    <b className="grow ellipsis">{g.name}</b>
                    <span className="hint mono">{t('chat.emoji.ofMax', { count: g.emoji?.length ?? 0, max: MAX })}</span>
                  </div>
                  {list.length > 0 ? (
                    <div className="em-grid">
                      {list.map((e) => (
                        <EmojiTile key={e.name} g={g} e={e} boss={boss} />
                      ))}
                    </div>
                  ) : (
                    <p className="hint em-empty">{t('chat.emoji.noneYet')}</p>
                  )}
                  {boss && !q && v.drive && (g.emoji?.length ?? 0) < MAX && <EmojiAdd conv={g.conv} />}
                </section>
              )
            })}
            {q && groups.every((g) => !(g.emoji ?? []).some((e) => e.name.includes(q))) && <p className="hint">{t('chat.emoji.none')}</p>}
          </div>
        </>
      )}
    </Modal>
  )
}
