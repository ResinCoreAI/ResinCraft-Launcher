import { useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore, type CSSProperties, type ReactNode } from 'react'
import {
  ArrowDown,
  ArrowLeftRight,
  ArrowUp,
  ArrowUpRight,
  ChevronRight,
  ClipboardCopy,
  Copy,
  FileDown,
  FolderInput,
  FolderOpen,
  FolderPlus,
  GripVertical,
  ImagePlus,
  Layers,
  Loader2,
  PackageOpen,
  Palette,
  Pencil,
  Play,
  Plus,
  RotateCcw,
  Search,
  Settings2,
  Square,
  Trash2,
  UserRound
} from 'lucide-react'
import { createPortal } from 'react-dom'
import type { Instance, InstancePatch } from '@shared/types'
import { api } from '../api'
import ExportModal from '../components/ExportModal'
import MigrateModal from '../components/MigrateModal'
import { packDupesNote } from '../components/DuplicatesDialog'
import {
  celebrate,
  ConfirmModal,
  Empty,
  InstanceTile,
  Modal,
  MoreButton,
  openContextMenu,
  playAsEntries,
  tilt,
  useCopy,
  type ContextEntry,
  type ContextItem
} from '../components/ui'
import { useInstanceTask, useStore } from '../store'
import { COLOR_NAMES, COLORS, describe, hasCustomIcon, instancePath, playtime } from '../util'
import { t } from '../i18n'
import { OwnPackChip } from '../components/OwnPack'
import { SelectBox, SelectionBar, SelectionContext, useBulkActions, useSelection, type Selection } from './LibrarySelection'

type Sort = 'recent' | 'name' | 'version' | 'playtime' | 'custom'
const SORTS: Sort[] = ['recent', 'name', 'version', 'playtime', 'custom']

/** What's being dragged and where the pointer is. */
interface DragState {
  inst: Instance
  x: number
  y: number
  /** Where inside the card it was grabbed, so the card doesn't jump under the cursor. */
  dx: number
  dy: number
  width: number
  /** Group under the pointer ('' = Ungrouped), or null when over nothing droppable. */
  over: string | null
  /** The card under the pointer, and whether the dragged one would go after it (else before). */
  target: { id: string; after: boolean } | null
}

/** A group header being dragged to a new place among the groups. */
interface GroupDrag {
  name: string
  /** The group it would land next to, and on which side. */
  over: string | null
  after: boolean
}

export default function Library() {
  const { instances, setNewInstanceOpen, refreshInstances, toast, go } = useStore()
  const [q, setQ] = useState('')
  const [sort, setSortState] = useState<Sort>(loadSort)
  const setSort = (next: Sort) => {
    setSortState(next)
    saveSort(next)
  }
  const order = useInstanceOrder()
  const [importing, setImporting] = useState(false)
  const [creating, setCreating] = useState(false)
  const [drag, setDrag] = useState<DragState | null>(null)
  const [groupDrag, setGroupDrag] = useState<GroupDrag | null>(null)
  const [landed, setLanded] = useState<string | null>(null)
  const names = useGroupNames()

  const shown = useMemo(() => {
    const list = instances.filter((i) => `${i.name} ${i.mcVersion} ${i.loader}`.toLowerCase().includes(q.toLowerCase()))
    const cmp: Record<Sort, (a: Instance, b: Instance) => number> = {
      recent: (a, b) => (b.lastPlayed ?? b.createdAt) - (a.lastPlayed ?? a.createdAt),
      name: (a, b) => a.name.localeCompare(b.name),
      version: (a, b) => b.mcVersion.localeCompare(a.mcVersion, undefined, { numeric: true }),
      playtime: (a, b) => b.playtime - a.playtime,
      custom: byOrder(order)
    }
    return list.sort(cmp[sort])
  }, [instances, q, sort, order])

  // Groups in the player's order, Ungrouped ('') among them wherever it was put. Headers appear once any group exists.
  const grouped = names.length > 0
  const ungroupedAt = useUngroupedAt()
  const sectionOrder = useMemo(() => withUngrouped(names, ungroupedAt), [names, ungroupedAt])
  const sections = useMemo(() => {
    const byGroup = (g: string) => shown.filter((i) => (i.group?.trim() ?? '') === g)
    const all: [string, Instance[]][] = sectionOrder.map((g): [string, Instance[]] => [g, byGroup(g)])
    // While searching, only show groups with matches; otherwise keep empty groups visible as drop targets.
    return all.filter(([g, list]) => list.length > 0 || (!q && (g !== '' || !!drag)))
  }, [shown, sectionOrder, q, drag])

  const [collapsed, setCollapsed] = useState<Set<string>>(loadCollapsed)
  const toggle = (name: string) =>
    setCollapsed((c) => {
      const next = new Set(c)
      if (next.has(name)) next.delete(name)
      else next.add(name)
      saveCollapsed(next)
      return next
    })

  // ---------- Selection ----------

  // Cards picked with their checkbox, Ctrl+click or Shift+click; the bar and the right-click menu act on all of them.
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const anchor = useRef<string>(undefined)
  // Only what's shown counts: a search hides the rest of the selection from every action.
  const picked = useMemo(() => shown.filter((i) => selected.has(i.id)), [shown, selected])
  // Cards in the order they appear (for Shift+click ranges); folded groups are skipped.
  const flat = useMemo(() => (grouped ? sections.flatMap(([g, list]) => (collapsed.has(g) ? [] : list)) : shown), [grouped, sections, collapsed, shown])
  const clearSelection = useCallback(() => setSelected(new Set()), [])
  const bulk = useBulkActions(picked, clearSelection)
  const pickedRef = useRef(picked)
  pickedRef.current = picked

  const selection: Selection = {
    active: picked.length > 0,
    count: picked.length,
    has: (id) => selected.has(id),
    click: (e, inst, force) => {
      const id = inst.id
      if (e.shiftKey && anchor.current && anchor.current !== id) {
        const a = flat.findIndex((x) => x.id === anchor.current)
        const b = flat.findIndex((x) => x.id === id)
        if (a >= 0 && b >= 0) {
          const range = flat.slice(Math.min(a, b), Math.max(a, b) + 1).map((x) => x.id)
          setSelected((s) => new Set([...(e.ctrlKey || e.metaKey ? s : []), ...range]))
          return true
        }
      }
      if (!force && !e.ctrlKey && !e.metaKey && picked.length === 0) return false
      anchor.current = id
      setSelected((s) => {
        const next = new Set([...s].filter((x) => shown.some((i) => i.id === x)))
        if (next.has(id)) next.delete(id)
        else next.add(id)
        return next
      })
      return true
    },
    menuFor: (inst) => (picked.length > 1 && selected.has(inst.id) ? bulk.entries : undefined),
    clearIfOutside: (inst) => {
      if (!selected.has(inst.id)) clearSelection()
    },
    remove: bulk.askDelete
  }

  /** The group header's checkbox: picks (or drops) every card in that group. */
  const selectGroup = (list: Instance[]) => {
    const all = list.every((i) => selected.has(i.id))
    setSelected((s) => {
      const next = new Set(s)
      for (const i of list) {
        if (all) next.delete(i.id)
        else next.add(i.id)
      }
      return next
    })
  }

  // Ctrl+A picks every card shown, Esc forgets the selection (not while typing, in a dialog or in a menu).
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      const el = e.target
      if ((el instanceof Element && el.closest('input, textarea, select, [contenteditable="true"]')) || document.querySelector('.backdrop, .ctx-menu')) return
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'a' && shown.length) {
        e.preventDefault()
        setSelected(new Set(shown.map((i) => i.id)))
      } else if (e.key === 'Escape' && selected.size && !dragRef.current) {
        e.preventDefault()
        clearSelection()
      }
    }
    window.addEventListener('keydown', key)
    return () => window.removeEventListener('keydown', key)
  }, [shown, selected.size, clearSelection])

  // ---------- Dragging ----------

  const dragRef = useRef<DragState | null>(null)
  dragRef.current = drag

  const startDrag = useCallback((inst: Instance, e: PointerEvent, card: HTMLElement) => {
    const r = card.getBoundingClientRect()
    setDrag({ inst, x: e.clientX, y: e.clientY, dx: e.clientX - r.left, dy: e.clientY - r.top, width: r.width, over: null, target: null })
  }, [])

  useEffect(() => {
    if (!drag) return
    const content = document.querySelector('.content')
    let raf = 0
    // Scroll the list while the pointer rests near its top or bottom edge.
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

    const move = (e: PointerEvent) => {
      const hit = document.elementFromPoint(e.clientX, e.clientY)
      const el = hit?.closest<HTMLElement>('[data-group]')
      const over = el ? (el.dataset.group ?? null) : grouped ? null : ''
      // Over another card: left half drops before it, right half after it.
      const cardEl = hit?.closest<HTMLElement>('.icard[data-id]')
      const id = cardEl?.dataset.id
      let target: DragState['target'] = null
      if (cardEl && id && id !== dragRef.current?.inst.id) {
        const r = cardEl.getBoundingClientRect()
        target = { id, after: e.clientX > r.left + r.width / 2 }
      }
      setDrag((d) => (d ? { ...d, x: e.clientX, y: e.clientY, over, target } : d))
    }
    const up = async () => {
      const d = dragRef.current
      setDrag(null)
      if (!d || d.over === null) return
      const from = d.inst.group?.trim() ?? ''
      const moved = d.over !== from
      if (!moved && !d.target) return
      try {
        // Dropped next to a card: save that spot as the custom order and show the Library in it.
        if (d.target) {
          const base = [...instances].sort(sort === 'custom' ? byOrder(order) : sortCompare(sort)).map((i) => i.id)
          const ids = base.filter((id) => id !== d.inst.id)
          const at = ids.indexOf(d.target.id)
          ids.splice(at < 0 ? ids.length : at + (d.target.after ? 1 : 0), 0, d.inst.id)
          setInstanceOrder(ids)
          if (sort !== 'custom') {
            setSort('custom')
            toast(t('library.order.switched'), 'info')
          }
        }
        // A picked card carries the rest of the selection with it into the group.
        const carried = pickedRef.current.some((i) => i.id === d.inst.id) ? pickedRef.current : [d.inst]
        if (moved) for (const i of carried) if ((i.group?.trim() ?? '') !== d.over) await api.instances.update(i.id, { group: d.over || null })
        if (moved && carried.length > 1) toast(t('library.sel.moved', { count: carried.length, group: d.over || t('library.group.ungrouped') }))
        await refreshInstances()
        setLanded(d.inst.id)
        setTimeout(() => setLanded((l) => (l === d.inst.id ? null : l)), 900)
        // Dropping onto a folded group opens it, so you see where the card went.
        if (collapsed.has(d.over)) toggle(d.over)
      } catch (err) {
        toast((err as Error).message, 'error')
      }
    }
    const key = (e: KeyboardEvent) => e.key === 'Escape' && setDrag(null)
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
    // Listeners are installed once per drag; the latest state is read through dragRef.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [!!drag])

  const groupDragRef = useRef<GroupDrag | null>(null)
  groupDragRef.current = groupDrag

  useEffect(() => {
    if (!groupDrag) return
    const move = (e: PointerEvent) => {
      const el = document.elementFromPoint(e.clientX, e.clientY)?.closest<HTMLElement>('[data-group]')
      const name = el?.dataset.group
      if (el === null || el === undefined || name === undefined) return setGroupDrag((g) => (g ? { ...g, over: null } : g))
      const r = el.getBoundingClientRect()
      const after = e.clientY > r.top + r.height / 2
      setGroupDrag((g) => (g ? { ...g, over: name, after } : g))
    }
    const up = () => {
      const g = groupDragRef.current
      setGroupDrag(null)
      if (!g || g.over === null || g.over === g.name) return
      const list = sectionOrder.filter((n) => n !== g.name)
      const at = list.indexOf(g.over)
      list.splice(at < 0 ? list.length : at + (g.after ? 1 : 0), 0, g.name)
      setGroupOrder(list)
    }
    const key = (e: KeyboardEvent) => e.key === 'Escape' && setGroupDrag(null)
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', up, { once: true })
    window.addEventListener('keydown', key)
    document.body.classList.add('dragging')
    return () => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', up)
      window.removeEventListener('keydown', key)
      document.body.classList.remove('dragging')
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [!!groupDrag])

  const [migrating, setMigrating] = useState(false)

  const importPack = async (from?: Element) => {
    setImporting(true)
    try {
      const inst = await api.content.importModpack()
      if (inst) {
        await refreshInstances()
        toast(t('library.imported', { name: inst.name }))
        const dupes = packDupesNote(inst)
        if (dupes) toast(dupes, 'info')
        celebrate(from)
        go({ page: 'instance', id: inst.id })
      }
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setImporting(false)
    }
  }

  const card = (i: Instance, index: number) => (
    <InstanceCard
      key={i.id}
      inst={i}
      withMenu
      index={index}
      onDragStart={startDrag}
      dragging={drag?.inst.id === i.id}
      landed={landed === i.id}
      dropSide={drag?.target?.id === i.id ? (drag.target.after ? 'after' : 'before') : undefined}
    />
  )

  // Right-clicking empty space in the Library.
  const pageMenu = (e: React.MouseEvent) =>
    openContextMenu(e, [
      { label: t('library.newInstance'), icon: <Plus />, onSelect: () => setNewInstanceOpen(true) },
      { label: t('library.importModpack'), icon: <FileDown />, disabled: importing, onSelect: () => void importPack() },
      { label: t('migrate.button'), icon: <ArrowLeftRight />, onSelect: () => setMigrating(true) },
      instances.length > 0 && { label: t('library.group.create'), icon: <FolderPlus />, onSelect: () => setCreating(true) }
    ])

  return (
    <div className={`page fill ${picked.length ? 'selecting' : ''}`} onContextMenu={pageMenu}>
      <div className="page-head">
        <div>
          <h1>{t('library.title')}</h1>
          <p>{t('library.subtitle')}</p>
        </div>
        <div className="spacer" />
        <button className="btn" onClick={() => setMigrating(true)}>
          <ArrowLeftRight /> {t('migrate.button')}
        </button>
        <button className="btn" onClick={(e) => void importPack(e.currentTarget)} disabled={importing}>
          {importing ? <Loader2 className="spin" /> : <FileDown />} {t('library.importModpack')}
        </button>
        <button className="btn primary" onClick={() => setNewInstanceOpen(true)}>
          <Plus /> {t('library.newInstance')}
        </button>
      </div>

      {instances.length === 0 ? (
        <Empty
          icon={<Layers />}
          title={t('library.empty.title')}
          action={
            <div className="row" style={{ justifyContent: 'center' }}>
              <button className="btn primary" onClick={() => setNewInstanceOpen(true)}>
                <Plus /> {t('library.empty.create')}
              </button>
              <button className="btn" onClick={() => go({ page: 'discover', kind: 'modpack' })}>
                {t('library.empty.browse')}
              </button>
              <button className="btn" onClick={() => setMigrating(true)}>
                <ArrowLeftRight /> {t('migrate.button')}
              </button>
            </div>
          }
        >
          {t('library.empty.body')}
        </Empty>
      ) : (
        <>
          <div className="toolbar">
            <div className="search">
              <Search />
              <input className="input" placeholder={t('library.search')} value={q} onChange={(e) => setQ(e.target.value)} />
            </div>
            <select className="select" value={sort} onChange={(e) => setSort(e.target.value as Sort)} aria-label={t('library.sort.label')}>
              {SORTS.map((v) => (
                <option key={v} value={v}>
                  {t(`library.sort.${v}`)}
                </option>
              ))}
            </select>
            <button className="btn" onClick={() => setCreating(true)}>
              <FolderPlus /> {t('library.group.create')}
            </button>
          </div>
          {grouped && <p className="hint drag-hint">{t('library.group.dragHint')}</p>}

          <SelectionContext.Provider value={selection}>
          {grouped ? (
            sections.map(([name, list]) => (
              <section
                key={name || '~ungrouped'}
                data-group={name}
                className={[
                  'inst-group',
                  drag ? 'droppable' : '',
                  drag?.over === name && (drag.inst.group?.trim() ?? '') !== name && !drag.target ? 'drop-target' : '',
                  groupDrag?.name === name ? 'group-dragging' : '',
                  groupDrag && groupDrag.over === name && groupDrag.name !== name ? (groupDrag.after ? 'group-drop-after' : 'group-drop-before') : ''
                ].join(' ')}
              >
                <GroupHeader
                  name={name}
                  count={list.length}
                  collapsed={collapsed.has(name)}
                  onToggle={() => toggle(name)}
                  order={sectionOrder}
                  onGripDown={() => setGroupDrag({ name, over: null, after: false })}
                  picked={list.filter((i) => selected.has(i.id)).length}
                  onSelectAll={list.length ? () => selectGroup(list) : undefined}
                />
                <Collapsible open={!collapsed.has(name)}>
                  {list.length > 0 ? <div className="grid">{list.map(card)}</div> : <div className="group-empty">{t('library.group.empty')}</div>}
                </Collapsible>
              </section>
            ))
          ) : (
            <div className="grid">{shown.map(card)}</div>
          )}
          </SelectionContext.Provider>

          {shown.length === 0 && (
            <Empty icon={<Search />} title={t('library.noMatch.title')}>
              {t('library.noMatch.body')}
            </Empty>
          )}
        </>
      )}

      {(picked.length > 0 || bulk.progress) && (
        <SelectionBar
          count={picked.length}
          total={shown.length}
          bulk={bulk}
          onSelectAll={() => setSelected(new Set(shown.map((i) => i.id)))}
          onClear={clearSelection}
        />
      )}
      {bulk.dialogs}

      {drag && <DragGhost drag={drag} more={picked.some((i) => i.id === drag.inst.id) ? picked.length - 1 : 0} />}

      {creating && (
        <NameModal
          title={t('library.group.createTitle')}
          initial=""
          placeholder={t('library.group.new')}
          onClose={() => setCreating(false)}
          onSave={async (name) => {
            addSavedGroup(name)
            if (collapsed.has(name)) toggle(name)
            toast(t('library.group.created', { name }))
          }}
        />
      )}
      {migrating && <MigrateModal onClose={() => setMigrating(false)} onImported={() => void refreshInstances()} />}
    </div>
  )
}

/** The card that follows the pointer while dragging. */
function DragGhost({ drag, more }: { drag: DragState; more: number }) {
  const style: CSSProperties = { width: drag.width, transform: `translate(${drag.x - drag.dx}px, ${drag.y - drag.dy}px) rotate(-2deg) scale(1.04)` }
  // In <body>, so the page's transform can't offset it from the pointer.
  return createPortal(
    <div className="drag-ghost" style={style} aria-hidden>
      <InstanceTile inst={drag.inst} wide />
      {more > 0 && <span className="drag-more">+{more}</span>}
      <div className="body">
        <div className="name ellipsis">{drag.inst.name}</div>
        <div className="sub">{describe(drag.inst)}</div>
      </div>
    </div>,
    document.body
  )
}

/** Height-animated fold. Content is clipped only while folded or folding, so card menus can still overflow. */
export function Collapsible({ open, children }: { open: boolean; children: ReactNode }) {
  const [clip, setClip] = useState(!open)
  useEffect(() => {
    if (!open) setClip(true)
  }, [open])
  return (
    <div
      className={`group-body ${open ? 'open' : ''}`}
      onTransitionEnd={(e) => {
        if (e.target === e.currentTarget && e.propertyName === 'grid-template-rows' && open) setClip(false)
      }}
    >
      <div className="group-inner" style={{ overflow: clip ? 'hidden' : 'visible' }}>
        {children}
      </div>
    </div>
  )
}

export function InstanceCard({
  inst,
  withMenu,
  index = 0,
  onDragStart,
  dragging,
  landed,
  dropSide
}: {
  inst: Instance
  withMenu?: boolean
  /** Position in its grid, for the staggered entrance. */
  index?: number
  onDragStart?: (inst: Instance, e: PointerEvent, card: HTMLElement) => void
  dragging?: boolean
  landed?: boolean
  /** Another card is being dragged over this one: where it would land. */
  dropSide?: 'before' | 'after'
}) {
  const { go, games, play, stop } = useStore()
  const task = useInstanceTask(inst.id)
  const menu = useInstanceMenu(inst)
  const sel = useSelection()
  const picked = !!sel?.has(inst.id)
  // On a card in a selection of several, the menu (right-click and "…") is the selection's.
  const bulkItems = sel?.menuFor(inst)
  const state = games[inst.id]
  const ref = useRef<HTMLDivElement>(null)
  const dragged = useRef(false)

  // A press becomes a drag after the pointer moves a few pixels; a plain click still opens the instance.
  const onPointerDown = (e: React.PointerEvent) => {
    if (!onDragStart || e.button !== 0 || (e.target as HTMLElement).closest('button, .menu')) return
    const sx = e.clientX
    const sy = e.clientY
    dragged.current = false
    const move = (ev: PointerEvent) => {
      if (Math.hypot(ev.clientX - sx, ev.clientY - sy) < 6 || !ref.current) return
      dragged.current = true
      cleanup()
      onDragStart(inst, ev, ref.current)
    }
    const cleanup = () => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', cleanup)
    }
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', cleanup)
  }

  return (
    <>
      <div
        ref={ref}
        data-id={inst.id}
        className={`icard ${dragging ? 'is-dragging' : ''} ${landed ? 'landed' : ''} ${dropSide ? `drop-${dropSide}` : ''} ${picked ? 'selected' : ''}`}
        style={{ '--i': Math.min(index, 12) } as CSSProperties}
        role="button"
        tabIndex={0}
        aria-pressed={sel ? picked : undefined}
        onPointerDown={onPointerDown}
        onPointerMove={tilt.onPointerMove}
        onPointerLeave={tilt.onPointerLeave}
        onContextMenu={(e) => {
          if (bulkItems) return openContextMenu(e, bulkItems(), t('menu.selected', { count: sel!.count }))
          sel?.clearIfOutside(inst)
          menu.open(e)
        }}
        onClick={(e) => {
          if (dragged.current) {
            dragged.current = false
            return
          }
          if (sel?.click(e, inst)) return
          go({ page: 'instance', id: inst.id })
        }}
        onKeyDown={(e) => {
          if (sel && e.target === e.currentTarget) {
            // Space picks the focused card; Delete on a picked card deletes the whole selection.
            if (e.key === ' ') {
              e.preventDefault()
              sel.click(e, inst, true)
              return
            }
            if (e.key === 'Delete' && bulkItems) {
              e.preventDefault()
              sel.remove()
              return
            }
          }
          menu.onKeyDown(e)
        }}
      >
        <InstanceTile inst={inst} wide />
        {sel && (
          <SelectBox
            className="card-check"
            checked={picked}
            label={t('library.sel.select', { name: inst.name })}
            onToggle={(e) => sel.click(e, inst, true)}
          />
        )}
        {state === 'running' && <span className="chip success badge-live">● {t('library.card.running')}</span>}
        <button
          className={`quick ${state === 'running' ? 'running busy' : state === 'preparing' ? 'busy' : ''}`}
          title={state === 'running' ? t('common.stop') : t('common.play')}
          onClick={(e) => {
            e.stopPropagation()
            if (state === 'running') void stop(inst.id)
            else if (state !== 'preparing') void play(inst.id)
          }}
        >
          {state === 'preparing' ? <Loader2 className="spin" /> : state === 'running' ? <Square fill="currentColor" /> : <Play fill="currentColor" />}
        </button>
        <div className="body">
          <div className="row">
            <div className="grow">
              <div className="name ellipsis">{inst.name}</div>
              <div className="sub">
                {state === 'preparing' ? (
                  <span style={{ color: 'var(--accent)' }}>
                    {task?.progress != null ? t('library.card.preparingProgress', { percent: Math.round(task.progress * 100) }) : t('library.card.preparing')}
                  </span>
                ) : (
                  <>
                    {inst.ownPack && <OwnPackChip pack={inst.ownPack} compact />}
                    <span className="ellipsis">{describe(inst)}</span>
                    <span>·</span>
                    <span>{playtime(inst.playtime)}</span>
                  </>
                )}
              </div>
            </div>
            {withMenu && <MoreButton items={bulkItems ?? menu.items} className="card-more" />}
          </div>
        </div>
      </div>
      {menu.dialogs}
    </>
  )
}

/**
 * What you can do to an instance from its right-click menu (Library and Home cards, the Home hero, the
 * sidebar). Render `dialogs` beside the element, not inside it: events from a dialog bubble up the React tree.
 */
export function useInstanceMenu(inst: Instance) {
  const { go, games, runs, accounts, play, stop, refreshInstances, toast, info } = useStore()
  const names = useGroupNames()
  const copy = useCopy()
  const [dialog, setDialog] = useState<'delete' | 'rename' | 'newGroup' | 'export'>()
  const close = () => setDialog(undefined)
  const state = games[inst.id]

  const update = async (patch: InstancePatch, done?: string) => {
    try {
      await api.instances.update(inst.id, patch)
      await refreshInstances()
      if (done) toast(done)
    } catch (e) {
      toast((e as Error).message, 'error')
    }
  }
  const moveTo = (group: string | null) => update({ group }, t('library.group.moved', { name: inst.name, group: group || t('library.group.ungrouped') }))
  const duplicate = async () => {
    try {
      await api.instances.duplicate(inst.id)
      await refreshInstances()
      toast(t('library.card.duplicated', { name: inst.name }))
    } catch (e) {
      toast((e as Error).message, 'error')
    }
  }
  const resetIcon = async () => {
    try {
      await api.instances.resetIcon(inst.id)
      await refreshInstances()
      toast(t('menu.instance.iconReset', { name: inst.name }))
    } catch (e) {
      toast((e as Error).message, 'error')
    }
  }

  const items = (): ContextEntry[] => {
    const current = inst.group?.trim() ?? ''
    return [
      state === 'running'
        ? { label: t('common.stop'), icon: <Square />, onSelect: () => void stop(inst.id) }
        : {
            label: t(state === 'preparing' ? 'common.preparing' : 'common.play'),
            icon: <Play />,
            disabled: state === 'preparing',
            onSelect: () => void play(inst.id)
          },
      // Pick who plays this time; while it runs, another account can join it.
      accounts.length > 1 && {
        label: t(state === 'running' || state === 'preparing' ? 'play.alsoPlayAs' : 'play.playAs'),
        icon: <UserRound />,
        items: playAsEntries(inst, accounts, runs[inst.id] ?? [], (id, accountId) => void play(id, accountId))
      },
      { label: t('menu.open'), icon: <ArrowUpRight />, shortcut: 'Enter', onSelect: () => go({ page: 'instance', id: inst.id }) },
      { label: t('menu.instance.settings'), icon: <Settings2 />, onSelect: () => go({ page: 'instance', id: inst.id, tab: 'settings' }) },
      '-',
      {
        label: t('menu.instance.group'),
        icon: <FolderInput />,
        items: [
          { label: t('library.group.none'), checked: current === '', onSelect: () => void moveTo(null) },
          ...names.map((g): ContextItem => ({ label: g, checked: current === g, onSelect: () => void moveTo(g) })),
          '-',
          { label: t('menu.instance.newGroup'), icon: <FolderPlus />, onSelect: () => setDialog('newGroup') }
        ]
      },
      { label: t('menu.rename'), icon: <Pencil />, shortcut: 'F2', onSelect: () => setDialog('rename') },
      {
        label: t('menu.instance.color'),
        icon: <Palette />,
        items: COLORS.map((c, i) => ({
          label: t(COLOR_NAMES[i]),
          icon: <span className="ctx-swatch" style={{ background: c }} />,
          checked: inst.color === c,
          onSelect: () => void update({ color: c })
        }))
      },
      { label: t('art.menu'), icon: <ImagePlus />, onSelect: () => go({ page: 'instance', id: inst.id, tab: 'settings' }) },
      hasCustomIcon(inst) && { label: t('menu.instance.resetIcon'), icon: <RotateCcw />, onSelect: () => void resetIcon() },
      { label: t('library.card.duplicate'), icon: <Copy />, onSelect: () => void duplicate() },
      { label: t('menu.instance.export'), icon: <PackageOpen />, onSelect: () => setDialog('export') },
      '-',
      { label: t('common.openFolder'), icon: <FolderOpen />, onSelect: () => void api.instances.openFolder(inst.id) },
      info && { label: t('menu.copyPath'), icon: <ClipboardCopy />, onSelect: () => void copy(instancePath(info.dataDir, inst.id)) },
      '-',
      { label: t('common.delete'), icon: <Trash2 />, danger: true, shortcut: 'Del', onSelect: () => setDialog('delete') }
    ]
  }

  const dialogs = (
    <>
      {dialog === 'delete' && <DeleteInstance inst={inst} onClose={close} />}
      {dialog === 'rename' && (
        <NameModal
          title={t('menu.instance.renameTitle')}
          initial={inst.name}
          onClose={close}
          onSave={async (name) => {
            await api.instances.update(inst.id, { name })
            await refreshInstances()
            toast(t('menu.instance.renamed', { name }))
          }}
        />
      )}
      {dialog === 'newGroup' && (
        <NameModal
          title={t('library.group.createTitle')}
          initial=""
          placeholder={t('library.group.new')}
          onClose={close}
          onSave={async (group) => {
            addSavedGroup(group)
            await moveTo(group)
          }}
        />
      )}
      {dialog === 'export' && <ExportModal inst={inst} onClose={close} />}
    </>
  )

  /** For a focused card or row: Enter opens, F2 renames, Delete deletes. */
  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.target !== e.currentTarget) return
    if (e.key === 'Enter') go({ page: 'instance', id: inst.id })
    else if (e.key === 'F2') setDialog('rename')
    else if (e.key === 'Delete') setDialog('delete')
    else return
    e.preventDefault()
  }

  return { items, open: (e: React.MouseEvent) => openContextMenu(e, items()), dialogs, onKeyDown }
}

export function DeleteInstance({ inst, onClose, onDeleted }: { inst: Instance; onClose: () => void; onDeleted?: () => void }) {
  const { refreshInstances, toast } = useStore()
  return (
    <ConfirmModal
      title={t('library.delete.title', { name: inst.name })}
      body={<>{t('library.delete.body')}</>}
      confirm={t('library.delete.confirm')}
      onClose={onClose}
      onConfirm={async () => {
        try {
          await api.instances.remove(inst.id)
          await refreshInstances()
          toast(t('library.delete.done', { name: inst.name }))
          onDeleted?.()
        } catch (e) {
          toast((e as Error).message, 'error')
        }
      }}
    />
  )
}

// ---------- Groups ----------

const SORT_KEY = 'resincraft.library.sort'
const ORDER_KEY = 'resincraft.library.order'

function loadSort(): Sort {
  try {
    const v = localStorage.getItem(SORT_KEY) as Sort | null
    return v && SORTS.includes(v) ? v : 'recent'
  } catch {
    return 'recent'
  }
}

function saveSort(v: Sort) {
  try {
    localStorage.setItem(SORT_KEY, v)
  } catch {
    // storage unavailable: the choice lasts until the launcher closes
  }
}

function sortCompare(sort: Exclude<Sort, 'custom'>): (a: Instance, b: Instance) => number {
  return {
    recent: (a: Instance, b: Instance) => (b.lastPlayed ?? b.createdAt) - (a.lastPlayed ?? a.createdAt),
    name: (a: Instance, b: Instance) => a.name.localeCompare(b.name),
    version: (a: Instance, b: Instance) => b.mcVersion.localeCompare(a.mcVersion, undefined, { numeric: true }),
    playtime: (a: Instance, b: Instance) => b.playtime - a.playtime
  }[sort]
}

// The player's own order of instances (ids), set by dragging cards between each other. Instances that aren't
// in it yet (new ones) come after, oldest first, so a new instance lands at the end.
let instanceOrder: string[] = (() => {
  try {
    return (JSON.parse(localStorage.getItem(ORDER_KEY) ?? '[]') as string[]).filter((id) => typeof id === 'string')
  } catch {
    return []
  }
})()
const orderListeners = new Set<() => void>()

function setInstanceOrder(next: string[]) {
  instanceOrder = next
  try {
    localStorage.setItem(ORDER_KEY, JSON.stringify(next))
  } catch {
    // storage unavailable
  }
  orderListeners.forEach((f) => f())
}

function useInstanceOrder(): string[] {
  return useSyncExternalStore(
    (cb) => {
      orderListeners.add(cb)
      return () => orderListeners.delete(cb)
    },
    () => instanceOrder
  )
}

function byOrder(order: string[]): (a: Instance, b: Instance) => number {
  const pos = new Map(order.map((id, i) => [id, i]))
  return (a, b) => (pos.get(a.id) ?? Infinity) - (pos.get(b.id) ?? Infinity) || a.createdAt - b.createdAt
}

const COLLAPSED_KEY = 'resincraft.library.collapsedGroups'
const GROUPS_KEY = 'resincraft.library.groups'

function loadCollapsed(): Set<string> {
  try {
    return new Set(JSON.parse(localStorage.getItem(COLLAPSED_KEY) ?? '[]') as string[])
  } catch {
    return new Set()
  }
}

function saveCollapsed(set: Set<string>) {
  try {
    localStorage.setItem(COLLAPSED_KEY, JSON.stringify([...set]))
  } catch {
    // storage unavailable: folding just won't be remembered
  }
}

// Groups the player created, in order, including ones that are still empty. Membership itself lives in
// each instance (instance.json), so nothing is lost if this list is cleared; empty groups just disappear.
let savedGroups: string[] = (() => {
  try {
    return (JSON.parse(localStorage.getItem(GROUPS_KEY) ?? '[]') as string[]).filter((g) => typeof g === 'string' && g.trim())
  } catch {
    return []
  }
})()
const groupListeners = new Set<() => void>()

function setSavedGroups(next: string[]) {
  savedGroups = next
  try {
    localStorage.setItem(GROUPS_KEY, JSON.stringify(next))
  } catch {
    // storage unavailable
  }
  groupListeners.forEach((f) => f())
}

// Where Ungrouped sits among the groups (it isn't a group, so it has no place in the saved list).
const UNGROUPED_AT_KEY = 'resincraft.library.ungroupedAt'
let ungroupedAt: number = (() => {
  try {
    const n = Number(localStorage.getItem(UNGROUPED_AT_KEY) ?? 0)
    return Number.isInteger(n) && n >= 0 ? n : 0
  } catch {
    return 0
  }
})()

function useUngroupedAt(): number {
  return useSyncExternalStore(
    (cb) => {
      groupListeners.add(cb)
      return () => groupListeners.delete(cb)
    },
    () => ungroupedAt
  )
}

/** The section order: the groups, with Ungrouped ('') at its place. */
function withUngrouped(names: string[], at: number): string[] {
  const list = [...names]
  list.splice(Math.min(at, list.length), 0, '')
  return list
}

/** Saves a new section order (groups and Ungrouped). */
function setGroupOrder(list: string[]) {
  ungroupedAt = Math.max(0, list.indexOf(''))
  try {
    localStorage.setItem(UNGROUPED_AT_KEY, String(ungroupedAt))
  } catch {
    // storage unavailable
  }
  setSavedGroups(list.filter((g) => g !== ''))
}

export function addSavedGroup(name: string) {
  const n = name.trim()
  if (n && !savedGroups.includes(n)) setSavedGroups([...savedGroups, n])
}

function useSavedGroups(): string[] {
  return useSyncExternalStore(
    (cb) => {
      groupListeners.add(cb)
      return () => groupListeners.delete(cb)
    },
    () => savedGroups
  )
}

/** Every group: the ones the player created (in their order), then any others found on instances, A to Z. */
export function useGroupNames(): string[] {
  const { instances } = useStore()
  const saved = useSavedGroups()
  return useMemo(() => {
    const onInstances = [...new Set(instances.map((i) => i.group?.trim()).filter((g): g is string => !!g))]
      .filter((g) => !saved.includes(g))
      .sort((a, b) => a.localeCompare(b, undefined, { sensitivity: 'base' }))
    return [...saved, ...onInstances]
  }, [instances, saved])
}

/** Moves every instance in group `from` to `to` (null = Ungrouped). */
async function moveGroup(instances: Instance[], from: string, to: string | null) {
  for (const i of instances.filter((x) => (x.group?.trim() ?? '') === from)) await api.instances.update(i.id, { group: to })
}

function GroupHeader({
  name,
  count,
  collapsed,
  onToggle,
  order,
  onGripDown,
  picked = 0,
  onSelectAll
}: {
  name: string
  count: number
  collapsed: boolean
  onToggle: () => void
  /** Every section, Ungrouped ('') included, in their current order (for Move up / Move down). */
  order: string[]
  onGripDown?: () => void
  /** How many of this group's cards are picked, and the checkbox that picks them all (none for an empty group). */
  picked?: number
  onSelectAll?: () => void
}) {
  const { instances, refreshInstances, toast } = useStore()
  const [renaming, setRenaming] = useState(false)
  const [removing, setRemoving] = useState(false)
  const label = name || t('library.group.ungrouped')
  const at = order.indexOf(name)
  const shift = (by: -1 | 1) => {
    const list = [...order]
    list.splice(at, 1)
    list.splice(at + by, 0, name)
    setGroupOrder(list)
  }
  const items = (): ContextEntry[] => [
    { label: t(collapsed ? 'menu.group.expand' : 'menu.group.collapse'), icon: <ChevronRight />, onSelect: onToggle },
    '-',
    { label: t('library.group.moveUp'), icon: <ArrowUp />, disabled: at <= 0, onSelect: () => shift(-1) },
    { label: t('library.group.moveDown'), icon: <ArrowDown />, disabled: at < 0 || at >= order.length - 1, onSelect: () => shift(1) },
    name ? '-' : null,
    name ? { label: t('library.group.rename'), icon: <Pencil />, onSelect: () => setRenaming(true) } : null,
    name ? { label: t('library.group.remove'), icon: <Trash2 />, danger: true, onSelect: () => setRemoving(true) } : null
  ]
  return (
    <div className="group-head" onContextMenu={(e) => openContextMenu(e, items())}>
      {onGripDown && (
        <span
          className="group-grip"
          role="button"
          tabIndex={-1}
          title={t('library.group.reorder', { name: label })}
          aria-label={t('library.group.reorder', { name: label })}
          onPointerDown={(e) => {
            if (e.button !== 0) return
            e.preventDefault()
            onGripDown()
          }}
        >
          <GripVertical />
        </span>
      )}
      {onSelectAll && (
        <SelectBox
          className={`group-check ${picked ? 'shown' : ''}`}
          checked={picked > 0 && picked === count}
          mixed={picked > 0 && picked < count}
          label={t('library.sel.selectGroup', { name: label })}
          onToggle={onSelectAll}
        />
      )}
      <button
        className={`group-toggle ${collapsed ? '' : 'open'}`}
        onClick={onToggle}
        aria-expanded={!collapsed}
        aria-label={t('library.group.toggle', { name: label })}
      >
        <ChevronRight />
        <span className="group-name">{label}</span>
        <span className="group-count">{count}</span>
      </button>
      <span className="group-line" />
      <MoreButton items={items} className="card-more" />
      {renaming && (
        <NameModal
          title={t('library.group.rename')}
          initial={name}
          onClose={() => setRenaming(false)}
          onSave={async (to) => {
            setSavedGroups(savedGroups.includes(name) ? savedGroups.map((g) => (g === name ? to : g)) : [...savedGroups, to])
            await moveGroup(instances, name, to)
            await refreshInstances()
          }}
        />
      )}
      {removing && (
        <ConfirmModal
          title={t('library.group.remove')}
          body={t('library.group.removeBody', { name })}
          confirm={t('common.remove')}
          onClose={() => setRemoving(false)}
          onConfirm={async () => {
            try {
              setSavedGroups(savedGroups.filter((g) => g !== name))
              await moveGroup(instances, name, null)
              await refreshInstances()
            } catch (e) {
              toast((e as Error).message, 'error')
            }
          }}
        />
      )}
    </div>
  )
}

export function NameModal({
  title,
  initial,
  placeholder,
  onClose,
  onSave
}: {
  title: string
  initial: string
  placeholder?: string
  onClose: () => void
  onSave: (name: string) => Promise<void>
}) {
  const { toast } = useStore()
  const [value, setValue] = useState(initial)
  const [busy, setBusy] = useState(false)
  useEffect(() => setValue(initial), [initial])
  const save = async () => {
    const v = value.trim()
    if (!v || v === initial) return onClose()
    setBusy(true)
    try {
      await onSave(v)
      onClose()
    } catch (e) {
      toast((e as Error).message, 'error')
      setBusy(false)
    }
  }
  return (
    <Modal
      title={title}
      onClose={onClose}
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={busy || !value.trim()} onClick={save}>
            {busy && <Loader2 className="spin" />} {t('common.save')}
          </button>
        </>
      }
    >
      <input
        className="input"
        value={value}
        placeholder={placeholder}
        autoFocus
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={(e) => e.key === 'Enter' && void save()}
      />
    </Modal>
  )
}
