import { useEffect, useMemo, useRef, useState, type CSSProperties } from 'react'
import { createPortal } from 'react-dom'
import {
  ArrowDown,
  ArrowUp,
  ChevronRight,
  Copy,
  FolderInput,
  FolderPlus,
  GripVertical,
  History,
  Pencil,
  Pin,
  PinOff,
  Plus,
  RefreshCw,
  Search,
  Server,
  Trash2,
  X,
  Zap
} from 'lucide-react'
import type { QuickJoinData, QuickJoinGroup } from '@shared/types'
import {
  addEntry,
  addressKey,
  EntryModal,
  itemIcon,
  JoinButton,
  newId,
  pingAddress,
  SourceChips,
  usePing,
  useQuickJoin,
  type EntryDraft,
  type QjItem,
  type QuickJoinState
} from '../components/QuickJoin'
import { Motd, PingStatus } from '../components/ServersTab'
import { ConfirmModal, Empty, MoreButton, openContextMenu, useCopy, type ContextEntry } from '../components/ui'
import { Collapsible, NameModal } from './Library'
import { useStore } from '../store'
import { timeAgo } from '../util'
import { api } from '../api'
import { t } from '../i18n'

/** What's being dragged (a server row or a group header) and where the pointer is. */
type Drag = { kind: 'entry' | 'group'; id: string; label: string; x: number; y: number }
/** Where it would land: before/after a row, at the end of a group ('' = Ungrouped), or next to a group. */
type Drop = { kind: 'row'; id: string; after: boolean } | { kind: 'group'; id: string } | { kind: 'groupEdge'; id: string; after: boolean }

const UNGROUPED = ''

export default function QuickJoin() {
  const q = useQuickJoin()
  const { toast } = useStore()
  const [query, setQuery] = useState('')
  const [editing, setEditing] = useState<{ id?: string; draft: EntryDraft }>()
  const [removing, setRemoving] = useState<QjItem>()
  const [naming, setNaming] = useState<{ group?: QuickJoinGroup }>()
  const [removingGroup, setRemovingGroup] = useState<QuickJoinGroup>()
  const [drag, setDrag] = useState<Drag>()
  const [drop, setDrop] = useState<Drop>()
  const dragRef = useRef<Drag | undefined>(undefined)
  dragRef.current = drag

  const match = (i: QjItem) => {
    const s = query.trim().toLowerCase()
    return !s || i.name.toLowerCase().includes(s) || i.address.toLowerCase().includes(s)
  }
  const { mine, saved, hosted } = q.items

  // Ping everything shown once it's loaded (cached for a minute, so Home → here doesn't ask twice).
  const addresses = useMemo(
    () => q.items.all.filter((i) => i.edition === 'java' && (!i.hosted || i.hosted.state === 'running')).map((i) => i.address),
    [q.items]
  )
  useEffect(() => addresses.forEach((a) => pingAddress(a)), [addresses])

  const groups = q.data?.groups ?? []
  const sections: [string, QuickJoinGroup | undefined, QjItem[]][] = [
    ...groups.map((g): [string, QuickJoinGroup, QjItem[]] => [g.id, g, mine.filter((i) => i.entry?.groupId === g.id)]),
    [UNGROUPED, undefined, mine.filter((i) => !i.entry?.groupId)]
  ]
  // Ungrouped shows when it has entries, or as the drop target / empty state when there are no groups.
  const shownSections = sections.filter(
    ([id, , list]) => (id !== UNGROUPED || list.length > 0 || groups.length === 0 || drag?.kind === 'entry') && (!query || list.some(match))
  )

  const newDraft = (p: Partial<EntryDraft> = {}): EntryDraft => ({ name: '', address: '', edition: 'java', pinned: false, ...p })

  const updateGroups = (change: (g: QuickJoinGroup[]) => QuickJoinGroup[]) => q.update((d) => ({ ...d, groups: change(d.groups) }))
  const patchEntry = (id: string, p: Partial<EntryDraft>) => q.update((d) => ({ ...d, entries: d.entries.map((e) => (e.id === id ? { ...e, ...p } : e)) }))

  /** Moves an entry into a group, before/after another entry or at the group's end. */
  const moveEntry = (id: string, groupId: string, target?: { id: string; after: boolean }) =>
    q.update((d) => {
      const entry = d.entries.find((e) => e.id === id)
      if (!entry || target?.id === id) return d
      const rest = d.entries.filter((e) => e.id !== id)
      const moved = { ...entry, groupId: groupId || undefined }
      let at = target ? rest.findIndex((e) => e.id === target.id) + (target.after ? 1 : 0) : -1
      if (at < 0) {
        // The end of the group: after its last entry (or at the very end).
        const last = rest.map((e) => (e.groupId ?? '') === groupId).lastIndexOf(true)
        at = last < 0 ? rest.length : last + 1
      }
      rest.splice(at, 0, moved)
      return { ...d, entries: rest }
    })

  const shiftEntry = (item: QjItem, by: -1 | 1) => {
    const list = mine.filter((i) => (i.entry?.groupId ?? '') === (item.entry?.groupId ?? ''))
    const at = list.findIndex((i) => i.key === item.key)
    const other = list[at + by]
    if (other?.entry && item.entry) void moveEntry(item.entry.id, item.entry.groupId ?? '', { id: other.entry.id, after: by > 0 })
  }

  const shiftGroup = (id: string, by: -1 | 1) =>
    updateGroups((list) => {
      const at = list.findIndex((g) => g.id === id)
      if (at < 0 || at + by < 0 || at + by >= list.length) return list
      const next = [...list]
      ;[next[at], next[at + by]] = [next[at + by], next[at]]
      return next
    })

  // ---------- Drag & drop (pointer events, like the Library: the window's file-drop handling blocks HTML5 drag) ----------

  const endDrag = () => {
    setDrag(undefined)
    setDrop(undefined)
  }
  const onDropAt = () => {
    if (drag?.kind === 'entry' && drop) {
      if (drop.kind === 'row') {
        const target = mine.find((i) => i.entry?.id === drop.id)?.entry
        if (target) void moveEntry(drag.id, target.groupId ?? '', { id: drop.id, after: drop.after })
      } else if (drop.kind === 'group') {
        const from = mine.find((i) => i.entry?.id === drag.id)?.entry?.groupId ?? ''
        if (from !== drop.id) void moveEntry(drag.id, drop.id)
        // Dropping onto a folded group opens it, so you see where the server went.
        const g = groups.find((x) => x.id === drop.id)
        if (g?.collapsed) toggle(g)
      }
    }
    if (drag?.kind === 'group' && drop?.kind === 'groupEdge' && drop.id !== drag.id && drop.id !== UNGROUPED)
      void updateGroups((list) => {
        const g = list.find((x) => x.id === drag.id)
        if (!g) return list
        const rest = list.filter((x) => x.id !== drag.id)
        rest.splice(rest.findIndex((x) => x.id === drop.id) + (drop.after ? 1 : 0), 0, g)
        return rest
      })
    endDrag()
  }
  // The pointer-up handler is installed once per drag; it calls the newest onDropAt.
  const finish = useRef(onDropAt)
  finish.current = onDropAt

  useEffect(() => {
    if (!drag) return
    const content = document.querySelector('.content')
    let raf = 0
    // Scroll while the pointer rests near the top or bottom edge.
    const autoscroll = () => {
      const d = dragRef.current
      if (d && content) {
        const r = content.getBoundingClientRect()
        const edge = 70
        if (d.y < r.top + edge) content.scrollTop -= Math.ceil((r.top + edge - d.y) / 5)
        else if (d.y > r.bottom - edge) content.scrollTop += Math.ceil((d.y - (r.bottom - edge)) / 5)
      }
      raf = requestAnimationFrame(autoscroll)
    }
    raf = requestAnimationFrame(autoscroll)
    const lowerHalf = (el: Element, y: number) => {
      const r = el.getBoundingClientRect()
      return y > r.top + r.height / 2
    }
    const move = (e: PointerEvent) => {
      const d = dragRef.current
      if (!d) return
      const hit = document.elementFromPoint(e.clientX, e.clientY)
      const section = hit?.closest<HTMLElement>('[data-qj-group]')
      let next: Drop | undefined
      if (d.kind === 'entry') {
        const rowEl = hit?.closest<HTMLElement>('[data-qj-entry]')
        const id = rowEl?.dataset.qjEntry
        if (rowEl && id && id !== d.id) next = { kind: 'row', id, after: lowerHalf(rowEl, e.clientY) }
        else if (section) next = { kind: 'group', id: section.dataset.qjGroup ?? '' }
      } else if (section?.dataset.qjGroup) next = { kind: 'groupEdge', id: section.dataset.qjGroup, after: lowerHalf(section, e.clientY) }
      setDrag({ ...d, x: e.clientX, y: e.clientY })
      setDrop(next)
    }
    const up = () => finish.current()
    const key = (e: KeyboardEvent) => e.key === 'Escape' && endDrag()
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', up, { once: true })
    window.addEventListener('keydown', key)
    document.body.classList.add('dragging')
    return () => {
      cancelAnimationFrame(raf)
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', up)
      window.removeEventListener('keydown', key)
      document.body.classList.remove('dragging')
    }
    // Listeners are installed once per drag; the latest state is read through refs.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [!!drag])

  // ---------- Menus ----------

  const groupMoveItems = (item: QjItem): ContextEntry[] =>
    [{ id: UNGROUPED, name: t('library.group.ungrouped') }, ...groups].map((g) => ({
      label: g.name,
      checked: (item.entry?.groupId ?? '') === g.id,
      onSelect: () => item.entry && void moveEntry(item.entry.id, g.id)
    }))

  const copy = useCopy()
  const rowMenu = (item: QjItem): ContextEntry[] => {
    const e = item.entry
    const inGroup = mine.filter((i) => (i.entry?.groupId ?? '') === (e?.groupId ?? ''))
    const at = inGroup.findIndex((i) => i.key === item.key)
    return [
      e && { label: t('common.edit'), icon: <Pencil />, onSelect: () => setEditing({ id: e.id, draft: { ...e } }) },
      e && {
        label: t(e.pinned ? 'quickjoin.unpin' : 'quickjoin.pin'),
        icon: e.pinned ? <PinOff /> : <Pin />,
        onSelect: () => void patchEntry(e.id, { pinned: !e.pinned })
      },
      !e &&
        item.source === 'saved' && {
          label: t('quickjoin.addToQuickJoin'),
          icon: <Plus />,
          onSelect: () => setEditing({ draft: newDraft({ name: item.name, address: item.address, instanceId: item.instanceIds[0] }) })
        },
      { label: t('servers.connect.copy'), icon: <Copy />, onSelect: () => void copy(item.address) },
      item.edition === 'java' && { label: t('common.refresh'), icon: <RefreshCw />, onSelect: () => pingAddress(item.address, true) },
      e && '-',
      e && groups.length > 0 && { label: t('quickjoin.moveToGroup'), icon: <FolderInput />, items: groupMoveItems(item) },
      e && { label: t('servers.moveUp'), icon: <ArrowUp />, disabled: at <= 0, onSelect: () => shiftEntry(item, -1) },
      e && { label: t('servers.moveDown'), icon: <ArrowDown />, disabled: at < 0 || at >= inGroup.length - 1, onSelect: () => shiftEntry(item, 1) },
      e && '-',
      e && { label: t('common.remove'), icon: <Trash2 />, danger: true, onSelect: () => setRemoving(item) }
    ]
  }

  const groupMenu = (g: QuickJoinGroup | undefined): ContextEntry[] => {
    const at = g ? groups.findIndex((x) => x.id === g.id) : -1
    return [
      g && { label: t(g.collapsed ? 'menu.group.expand' : 'menu.group.collapse'), icon: <ChevronRight />, onSelect: () => toggle(g) },
      { label: t('quickjoin.add'), icon: <Plus />, onSelect: () => setEditing({ draft: newDraft({ groupId: g?.id }) }) },
      g && '-',
      g && { label: t('library.group.moveUp'), icon: <ArrowUp />, disabled: at <= 0, onSelect: () => void shiftGroup(g.id, -1) },
      g && { label: t('library.group.moveDown'), icon: <ArrowDown />, disabled: at >= groups.length - 1, onSelect: () => void shiftGroup(g.id, 1) },
      g && '-',
      g && { label: t('library.group.rename'), icon: <Pencil />, onSelect: () => setNaming({ group: g }) },
      g && { label: t('library.group.remove'), icon: <Trash2 />, danger: true, onSelect: () => setRemovingGroup(g) }
    ]
  }

  const toggle = (g: QuickJoinGroup) => void updateGroups((list) => list.map((x) => (x.id === g.id ? { ...x, collapsed: !x.collapsed } : x)))

  const row = (item: QjItem) => (
    <QjRow
      key={item.key}
      item={item}
      q={q}
      menu={() => rowMenu(item)}
      draggable={!!item.entry && !query}
      dragging={drag?.kind === 'entry' && drag.id === item.entry?.id}
      dropSide={drag?.kind === 'entry' && drop?.kind === 'row' && drop.id === item.entry?.id ? (drop.after ? 'after' : 'before') : undefined}
      onDragStart={(e) => item.entry && setDrag({ kind: 'entry', id: item.entry.id, label: item.name, x: e.clientX, y: e.clientY })}
      onAdd={
        item.source === 'saved' ? () => setEditing({ draft: newDraft({ name: item.name, address: item.address, instanceId: item.instanceIds[0] }) }) : undefined
      }
    />
  )

  const recent = q.data?.recent ?? []

  return (
    <div className="page qj-page">
      <div className="page-head">
        <div>
          <h1>{t('quickjoin.title')}</h1>
          <p>{t('quickjoin.subtitle')}</p>
        </div>
        <div className="spacer" />
        <button className="btn" onClick={() => setNaming({})}>
          <FolderPlus /> {t('library.group.create')}
        </button>
        <button className="btn primary" onClick={() => setEditing({ draft: newDraft() })}>
          <Plus /> {t('quickjoin.add')}
        </button>
      </div>

      <div className="toolbar">
        <div className="search">
          <Search />
          <input className="input" placeholder={t('quickjoin.search')} value={query} onChange={(e) => setQuery(e.target.value)} />
        </div>
        <button className="btn" disabled={!addresses.length} onClick={() => addresses.forEach((a) => pingAddress(a, true))}>
          <RefreshCw /> {t('common.refresh')}
        </button>
      </div>

      {!q.loaded ? (
        <div className="results">
          <div className="skeleton" style={{ height: 62 }} />
          <div className="skeleton" style={{ height: 62 }} />
          <div className="skeleton" style={{ height: 62 }} />
        </div>
      ) : (
        <>
          {recent.length > 0 && !query && (
            <div className="qj-recent">
              <span className="label">
                <History /> {t('quickjoin.recent')}
              </span>
              {recent.slice(0, 8).map((r) => (
                <span key={r.address} className="chip" title={`${r.address} · ${timeAgo(r.at, true)}`}>
                  {r.name}
                </span>
              ))}
              <button className="btn ghost sm" onClick={() => void api.quickjoin.clearRecent().then(() => q.refreshRecent())}>
                <X /> {t('quickjoin.clearRecent')}
              </button>
            </div>
          )}

          <h2 className="section">
            <Zap size={18} /> {t('quickjoin.mine')}
          </h2>
          {groups.length > 0 && !query && <p className="hint drag-hint">{t('quickjoin.dragHint')}</p>}
          {mine.length === 0 && groups.length === 0 ? (
            <Empty
              icon={<Zap />}
              title={t('quickjoin.empty.title')}
              action={
                <button className="btn primary" onClick={() => setEditing({ draft: newDraft() })}>
                  <Plus /> {t('quickjoin.add')}
                </button>
              }
            >
              {t('quickjoin.empty.body')}
            </Empty>
          ) : (
            shownSections.map(([id, g, list]) => {
              const shown = list.filter(match)
              const label = g?.name ?? t('library.group.ungrouped')
              return (
                <section
                  key={id || '~ungrouped'}
                  className={[
                    'inst-group',
                    drag?.kind === 'entry' ? 'droppable' : '',
                    drop?.kind === 'group' && drop.id === id ? 'drop-target' : '',
                    drag?.kind === 'group' && drag.id === id ? 'group-dragging' : '',
                    drop?.kind === 'groupEdge' && drop.id === id && drag?.id !== id ? (drop.after ? 'group-drop-after' : 'group-drop-before') : ''
                  ].join(' ')}
                  data-qj-group={id}
                >
                  {(groups.length > 0 || id !== UNGROUPED) && (
                    <div className="group-head" onContextMenu={(e) => openContextMenu(e, groupMenu(g))}>
                      {g && !query && (
                        <span
                          className="group-grip"
                          role="button"
                          tabIndex={-1}
                          title={t('library.group.reorder', { name: label })}
                          aria-label={t('library.group.reorder', { name: label })}
                          onPointerDown={(e) => {
                            if (e.button !== 0) return
                            e.preventDefault()
                            setDrag({ kind: 'group', id, label, x: e.clientX, y: e.clientY })
                          }}
                        >
                          <GripVertical />
                        </span>
                      )}
                      <button
                        className={`group-toggle ${g?.collapsed ? '' : 'open'}`}
                        onClick={() => g && toggle(g)}
                        aria-expanded={!g?.collapsed}
                        aria-label={t('library.group.toggle', { name: label })}
                      >
                        <ChevronRight />
                        <span className="group-name">{label}</span>
                        <span className="group-count">{list.length}</span>
                      </button>
                      <span className="group-line" />
                      <MoreButton items={() => groupMenu(g)} className="card-more" />
                    </div>
                  )}
                  <Collapsible open={!g?.collapsed || !!query}>
                    {shown.length > 0 ? <div className="list">{shown.map(row)}</div> : <div className="group-empty">{t('quickjoin.group.empty')}</div>}
                  </Collapsible>
                </section>
              )
            })
          )}

          {saved.some(match) && (
            <>
              <h2 className="section">
                <Server size={18} /> {t('quickjoin.saved')}
              </h2>
              <p className="hint" style={{ marginTop: -6, marginBottom: 10 }}>
                {t('quickjoin.saved.hint')}
              </p>
              <div className="list">{saved.filter(match).map(row)}</div>
            </>
          )}

          {hosted.some(match) && (
            <>
              <h2 className="section">
                <Server size={18} /> {t('quickjoin.hosted')}
              </h2>
              <div className="list">{hosted.filter(match).map(row)}</div>
            </>
          )}

          {query && !q.items.all.some(match) && (
            <Empty icon={<Search />} title={t('library.noMatch.title')}>
              {t('quickjoin.noMatch')}
            </Empty>
          )}
        </>
      )}

      {drag && <DragGhost drag={drag} />}

      {editing && q.data && (
        <EntryModal
          initial={editing.draft}
          isNew={!editing.id}
          groups={groups}
          onClose={() => setEditing(undefined)}
          onSave={async (draft) => {
            if (editing.id) return patchEntry(editing.id, draft)
            const k = addressKey(draft.address)
            if (q.data?.entries.some((e) => addressKey(e.address) === k)) toast(t('quickjoin.alreadyThere', { address: draft.address }), 'info')
            await q.update((d) => addEntry(d, draft))
            pingAddress(draft.address, true)
          }}
        />
      )}
      {removing?.entry && (
        <ConfirmModal
          title={t('quickjoin.remove.title', { name: removing.name })}
          body={t('quickjoin.remove.body')}
          confirm={t('common.remove')}
          onClose={() => setRemoving(undefined)}
          onConfirm={() => q.update((d) => ({ ...d, entries: d.entries.filter((e) => e.id !== removing.entry!.id) }))}
        />
      )}
      {naming && (
        <NameModal
          title={naming.group ? t('library.group.rename') : t('library.group.createTitle')}
          initial={naming.group?.name ?? ''}
          placeholder={t('library.group.new')}
          onClose={() => setNaming(undefined)}
          onSave={async (name) => {
            if (name.length > 60) throw new Error(t('quickjoin.group.tooLong'))
            const g = naming.group
            await updateGroups((list) => (g ? list.map((x) => (x.id === g.id ? { ...x, name } : x)) : [...list, { id: newId(), name, collapsed: false }]))
            if (!g) toast(t('library.group.created', { name }))
          }}
        />
      )}
      {removingGroup && (
        <ConfirmModal
          title={t('library.group.remove')}
          body={t('quickjoin.group.removeBody', { name: removingGroup.name })}
          confirm={t('common.remove')}
          onClose={() => setRemovingGroup(undefined)}
          onConfirm={() =>
            q.update((d: QuickJoinData) => ({
              ...d,
              groups: d.groups.filter((g) => g.id !== removingGroup.id),
              entries: d.entries.map((e) => (e.groupId === removingGroup.id ? { ...e, groupId: undefined } : e))
            }))
          }
        />
      )}
    </div>
  )
}

function QjRow({
  item,
  q,
  menu,
  draggable,
  dragging,
  dropSide,
  onDragStart,
  onAdd
}: {
  item: QjItem
  q: QuickJoinState
  menu: () => ContextEntry[]
  draggable: boolean
  dragging: boolean
  dropSide?: 'before' | 'after'
  onDragStart: (e: PointerEvent) => void
  onAdd?: () => void
}) {
  const ping = usePing(item)
  const icon = itemIcon(item, ping)
  const hosted = item.hosted
  // A press becomes a drag after the pointer moves a few pixels; buttons keep working as usual.
  const onPointerDown = (e: React.PointerEvent) => {
    if (!draggable || e.button !== 0 || (e.target as HTMLElement).closest('button, a, input, select, .menu')) return
    const sx = e.clientX
    const sy = e.clientY
    const move = (ev: PointerEvent) => {
      if (Math.hypot(ev.clientX - sx, ev.clientY - sy) < 6) return
      cleanup()
      window.getSelection()?.removeAllRanges()
      onDragStart(ev)
    }
    const cleanup = () => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', cleanup)
    }
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', cleanup)
  }
  return (
    <div
      className={['list-row', 'qj-row', draggable ? 'qj-draggable' : '', dragging ? 'qj-dragging' : '', dropSide ? `qj-drop-${dropSide}` : ''].join(' ')}
      data-qj-entry={item.entry?.id}
      onPointerDown={onPointerDown}
      onContextMenu={(e) => openContextMenu(e, menu(), item.name)}
    >
      {draggable && (
        <span className="qj-grip" aria-hidden>
          <GripVertical />
        </span>
      )}
      <div className="ico">{icon ? <img src={icon} alt="" style={{ imageRendering: 'pixelated' }} /> : <Server />}</div>
      <div className="grow" style={{ minWidth: 0 }}>
        <div className="title ellipsis">
          {item.entry?.pinned && <Pin className="qj-pin" aria-label={t('quickjoin.pinned')} />}
          {item.name}
        </div>
        <div className="it-meta">
          <span className="mono">{item.address}</span>
          {ping?.state === 'ok' && ping.status.version && <span>{ping.status.version}</span>}
          <SourceChips item={item} />
          {hosted && <span>{t(`servers.state.${hosted.state}`)}</span>}
          {item.lastJoined && <span>{t('quickjoin.lastJoined', { when: timeAgo(item.lastJoined, true) })}</span>}
        </div>
        {ping?.state === 'ok' && ping.status.motd.trim() && (
          <div className="it-motd">
            <Motd text={ping.status.motd} />
          </div>
        )}
      </div>
      {item.edition === 'java' && (!hosted || hosted.state === 'running') && <PingStatus ping={ping} />}
      <div className="it-actions qj-actions">
        {onAdd && (
          <button className="btn ghost icon sm" title={t('quickjoin.addToQuickJoin')} aria-label={t('quickjoin.addToQuickJoin')} onClick={onAdd}>
            <Plus />
          </button>
        )}
        <JoinButton item={item} q={q} />
        <MoreButton items={menu} className="card-more" />
      </div>
    </div>
  )
}

/** The label that follows the pointer while dragging (in <body>, so the page's transform can't offset it). */
function DragGhost({ drag }: { drag: Drag }) {
  const style: CSSProperties = { transform: `translate(${drag.x + 12}px, ${drag.y - 14}px)` }
  return createPortal(
    <div className="qj-ghost" style={style} aria-hidden>
      {drag.kind === 'group' ? <FolderInput /> : <Server />} {drag.label}
    </div>,
    document.body
  )
}
