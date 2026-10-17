import { useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore, type CSSProperties, type ReactNode } from 'react'
import {
  ChevronRight,
  Copy,
  FileDown,
  FolderInput,
  FolderOpen,
  FolderPlus,
  Layers,
  Loader2,
  Pencil,
  Play,
  Plus,
  Search,
  Settings2,
  Square,
  Trash2
} from 'lucide-react'
import { createPortal } from 'react-dom'
import type { Instance } from '@shared/types'
import { api } from '../api'
import { ConfirmModal, Empty, InstanceTile, Menu, Modal } from '../components/ui'
import { useInstanceTask, useStore } from '../store'
import { describe, playtime } from '../util'
import { t } from '../i18n'

type Sort = 'recent' | 'name' | 'version' | 'playtime'

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
}

export default function Library() {
  const { instances, setNewInstanceOpen, refreshInstances, toast, go } = useStore()
  const [q, setQ] = useState('')
  const [sort, setSort] = useState<Sort>('recent')
  const [importing, setImporting] = useState(false)
  const [creating, setCreating] = useState(false)
  const [drag, setDrag] = useState<DragState | null>(null)
  const [landed, setLanded] = useState<string | null>(null)
  const names = useGroupNames()

  const shown = useMemo(() => {
    const list = instances.filter((i) => `${i.name} ${i.mcVersion} ${i.loader}`.toLowerCase().includes(q.toLowerCase()))
    const cmp: Record<Sort, (a: Instance, b: Instance) => number> = {
      recent: (a, b) => (b.lastPlayed ?? b.createdAt) - (a.lastPlayed ?? a.createdAt),
      name: (a, b) => a.name.localeCompare(b.name),
      version: (a, b) => b.mcVersion.localeCompare(a.mcVersion, undefined, { numeric: true }),
      playtime: (a, b) => b.playtime - a.playtime
    }
    return list.sort(cmp[sort])
  }, [instances, q, sort])

  // Groups (like Prism): Ungrouped first, then the saved groups in order. Headers appear once any group exists.
  const grouped = names.length > 0
  const sections = useMemo(() => {
    const byGroup = (g: string) => shown.filter((i) => (i.group?.trim() ?? '') === g)
    const all: [string, Instance[]][] = [['', byGroup('')], ...names.map((g): [string, Instance[]] => [g, byGroup(g)])]
    // While searching, only show groups with matches; otherwise keep empty groups visible as drop targets.
    return all.filter(([g, list]) => list.length > 0 || (!q && (g !== '' || !!drag)))
  }, [shown, names, q, drag])

  const [collapsed, setCollapsed] = useState<Set<string>>(loadCollapsed)
  const toggle = (name: string) =>
    setCollapsed((c) => {
      const next = new Set(c)
      if (next.has(name)) next.delete(name)
      else next.add(name)
      saveCollapsed(next)
      return next
    })

  // ---------- Dragging ----------

  const dragRef = useRef<DragState | null>(null)
  dragRef.current = drag

  const startDrag = useCallback((inst: Instance, e: PointerEvent, card: HTMLElement) => {
    const r = card.getBoundingClientRect()
    setDrag({ inst, x: e.clientX, y: e.clientY, dx: e.clientX - r.left, dy: e.clientY - r.top, width: r.width, over: null })
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
      const el = document.elementFromPoint(e.clientX, e.clientY)?.closest<HTMLElement>('[data-group]')
      const over = el ? (el.dataset.group ?? null) : null
      setDrag((d) => (d ? { ...d, x: e.clientX, y: e.clientY, over } : d))
    }
    const up = async () => {
      const d = dragRef.current
      setDrag(null)
      if (!d || d.over === null) return
      const from = d.inst.group?.trim() ?? ''
      if (d.over === from) return
      try {
        await api.instances.update(d.inst.id, { group: d.over || null })
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

  const importPack = async () => {
    setImporting(true)
    try {
      const inst = await api.content.importModpack()
      if (inst) {
        await refreshInstances()
        toast(t('library.imported', { name: inst.name }))
        go({ page: 'instance', id: inst.id })
      }
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setImporting(false)
    }
  }

  const card = (i: Instance, index: number) => (
    <InstanceCard key={i.id} inst={i} withMenu index={index} onDragStart={startDrag} dragging={drag?.inst.id === i.id} landed={landed === i.id} />
  )

  return (
    <div className="page">
      <div className="page-head">
        <div>
          <h1>{t('library.title')}</h1>
          <p>{t('library.subtitle')}</p>
        </div>
        <div className="spacer" />
        <button className="btn" onClick={importPack} disabled={importing}>
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
              <option value="recent">{t('library.sort.recent')}</option>
              <option value="name">{t('library.sort.name')}</option>
              <option value="version">{t('library.sort.version')}</option>
              <option value="playtime">{t('library.sort.playtime')}</option>
            </select>
            <button className="btn" onClick={() => setCreating(true)}>
              <FolderPlus /> {t('library.group.create')}
            </button>
          </div>
          {grouped && <p className="hint drag-hint">{t('library.group.dragHint')}</p>}

          {grouped ? (
            sections.map(([name, list]) => (
              <section
                key={name || '~ungrouped'}
                data-group={name}
                className={`inst-group ${drag ? 'droppable' : ''} ${drag?.over === name && (drag.inst.group?.trim() ?? '') !== name ? 'drop-target' : ''}`}
              >
                <GroupHeader name={name} count={list.length} collapsed={collapsed.has(name)} onToggle={() => toggle(name)} />
                <Collapsible open={!collapsed.has(name)}>
                  {list.length > 0 ? <div className="grid">{list.map(card)}</div> : <div className="group-empty">{t('library.group.empty')}</div>}
                </Collapsible>
              </section>
            ))
          ) : (
            <div className="grid">{shown.map(card)}</div>
          )}

          {shown.length === 0 && (
            <Empty icon={<Search />} title={t('library.noMatch.title')}>
              {t('library.noMatch.body')}
            </Empty>
          )}
        </>
      )}

      {drag && <DragGhost drag={drag} />}

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
    </div>
  )
}

/** The card that follows the pointer while dragging. */
function DragGhost({ drag }: { drag: DragState }) {
  const style: CSSProperties = { width: drag.width, transform: `translate(${drag.x - drag.dx}px, ${drag.y - drag.dy}px) rotate(-2deg) scale(1.04)` }
  // In <body>, so the page's transform can't offset it from the pointer.
  return createPortal(
    <div className="drag-ghost" style={style} aria-hidden>
      <InstanceTile inst={drag.inst} wide />
      <div className="body">
        <div className="name ellipsis">{drag.inst.name}</div>
        <div className="sub">{describe(drag.inst)}</div>
      </div>
    </div>,
    document.body
  )
}

/** Height-animated fold. Content is clipped only while folded or folding, so card menus can still overflow. */
function Collapsible({ open, children }: { open: boolean; children: ReactNode }) {
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
  landed
}: {
  inst: Instance
  withMenu?: boolean
  /** Position in its grid, for the staggered entrance. */
  index?: number
  onDragStart?: (inst: Instance, e: PointerEvent, card: HTMLElement) => void
  dragging?: boolean
  landed?: boolean
}) {
  const { go, games, play, stop, refreshInstances, toast } = useStore()
  const task = useInstanceTask(inst.id)
  const [confirm, setConfirm] = useState(false)
  const [changingGroup, setChangingGroup] = useState(false)
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
        className={`icard ${dragging ? 'is-dragging' : ''} ${landed ? 'landed' : ''}`}
        style={{ '--i': Math.min(index, 12) } as CSSProperties}
        role="button"
        tabIndex={0}
        onPointerDown={onPointerDown}
        onClick={() => {
          if (dragged.current) {
            dragged.current = false
            return
          }
          go({ page: 'instance', id: inst.id })
        }}
        onKeyDown={(e) => e.key === 'Enter' && go({ page: 'instance', id: inst.id })}
      >
        <InstanceTile inst={inst} wide />
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
                    <span className="ellipsis">{describe(inst)}</span>
                    <span>·</span>
                    <span>{playtime(inst.playtime)}</span>
                  </>
                )}
              </div>
            </div>
            {withMenu && (
              <Menu
                items={[
                  { label: t('library.card.settings'), icon: <Settings2 />, onClick: () => go({ page: 'instance', id: inst.id, tab: 'settings' }) },
                  { label: t('common.openFolder'), icon: <FolderOpen />, onClick: () => void api.instances.openFolder(inst.id) },
                  { label: t('library.group.change'), icon: <FolderInput />, onClick: () => setChangingGroup(true) },
                  {
                    label: t('library.card.duplicate'),
                    icon: <Copy />,
                    onClick: async () => {
                      try {
                        await api.instances.duplicate(inst.id)
                        await refreshInstances()
                        toast(t('library.card.duplicated', { name: inst.name }))
                      } catch (e) {
                        toast((e as Error).message, 'error')
                      }
                    }
                  },
                  { label: t('common.delete'), icon: <Trash2 />, danger: true, divider: true, onClick: () => setConfirm(true) }
                ]}
              />
            )}
          </div>
        </div>
      </div>
      {confirm && <DeleteInstance inst={inst} onClose={() => setConfirm(false)} />}
      {changingGroup && <GroupModal inst={inst} onClose={() => setChangingGroup(false)} />}
    </>
  )
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

function addSavedGroup(name: string) {
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

function GroupHeader({ name, count, collapsed, onToggle }: { name: string; count: number; collapsed: boolean; onToggle: () => void }) {
  const { instances, refreshInstances, toast } = useStore()
  const [renaming, setRenaming] = useState(false)
  const [removing, setRemoving] = useState(false)
  const label = name || t('library.group.ungrouped')
  return (
    <div className="group-head">
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
      {name && (
        <Menu
          items={[
            { label: t('library.group.rename'), icon: <Pencil />, onClick: () => setRenaming(true) },
            { label: t('library.group.remove'), icon: <Trash2 />, danger: true, divider: true, onClick: () => setRemoving(true) }
          ]}
        />
      )}
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

/** Picks an existing group, "No group", or a new group name for one instance. */
function GroupModal({ inst, onClose }: { inst: Instance; onClose: () => void }) {
  const { refreshInstances, toast } = useStore()
  const names = useGroupNames()
  const current = inst.group?.trim() ?? ''
  const [choice, setChoice] = useState(current)
  const [typed, setTyped] = useState('')
  const target = typed.trim() || choice
  const save = async () => {
    try {
      if (target) addSavedGroup(target)
      await api.instances.update(inst.id, { group: target || null })
      await refreshInstances()
      toast(t('library.group.moved', { name: inst.name, group: target || t('library.group.ungrouped') }))
      onClose()
    } catch (e) {
      toast((e as Error).message, 'error')
    }
  }
  return (
    <Modal
      title={t('library.group.title')}
      subtitle={t('library.group.body')}
      onClose={onClose}
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={target === current} onClick={save}>
            {t('common.save')}
          </button>
        </>
      }
    >
      <div className="version-list" style={{ maxHeight: 240 }}>
        {['', ...names].map((g) => (
          <button
            key={g || '~none'}
            className={!typed.trim() && choice === g ? 'on' : ''}
            onClick={() => {
              setChoice(g)
              setTyped('')
            }}
          >
            {g || t('library.group.none')}
          </button>
        ))}
      </div>
      <input
        className="input"
        placeholder={t('library.group.new')}
        value={typed}
        onChange={(e) => setTyped(e.target.value)}
        onKeyDown={(e) => e.key === 'Enter' && target !== current && void save()}
      />
    </Modal>
  )
}

function NameModal({
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
