import { createContext, useContext, useRef, useState } from 'react'
import { ArrowUpCircle, Blocks, Check, ChevronDown, Copy, FolderInput, FolderPlus, Loader2, Minus, PackageOpen, Palette, RefreshCw, Square, Trash2, X } from 'lucide-react'
import type { Instance, InstancePatch, PackFormat } from '@shared/types'
import { api } from '../api'
import { ConfirmModal, Modal, openContextMenu, openMenuBelow, Segmented, type ContextEntry, type ContextItem } from '../components/ui'
import { nextExportVersion } from '../components/OwnPack'
import { useStore } from '../store'
import { COLOR_NAMES, COLORS } from '../util'
import { t } from '../i18n'
import { addSavedGroup, NameModal, useGroupNames } from './Library'
import '../library-select.css'

/** What a Library card needs to know about the multi-selection (null outside the Library, e.g. on Home). */
export interface Selection {
  /** Anything picked (among the instances shown). */
  active: boolean
  count: number
  has: (id: string) => boolean
  /** Ctrl+click toggles, Shift+click picks a range, and while anything is picked a plain click toggles too. True if handled. */
  click: (e: React.MouseEvent | React.KeyboardEvent, inst: Instance, force?: boolean) => boolean
  /** The menu for the whole selection, when `inst` is part of a selection of two or more. */
  menuFor: (inst: Instance) => (() => ContextEntry[]) | undefined
  /** Right-click on a card outside the selection: forget the selection (the card's own menu opens). */
  clearIfOutside: (inst: Instance) => void
  /** Delete pressed on a selected card. */
  remove: () => void
}

export const SelectionContext = createContext<Selection | null>(null)
export const useSelection = () => useContext(SelectionContext)

/** A square checkbox (role="checkbox") that doesn't start a drag or open the card. `mixed` = some of a group. */
export function SelectBox({
  checked,
  mixed,
  label,
  className = '',
  onToggle
}: {
  checked: boolean
  mixed?: boolean
  label: string
  className?: string
  onToggle: (e: React.MouseEvent | React.KeyboardEvent) => void
}) {
  return (
    <button
      type="button"
      role="checkbox"
      aria-checked={mixed ? 'mixed' : checked}
      aria-label={label}
      title={label}
      className={`sel-box ${checked ? 'on' : ''} ${mixed ? 'mixed' : ''} ${className}`}
      onPointerDown={(e) => e.stopPropagation()}
      onClick={(e) => {
        e.stopPropagation()
        onToggle(e)
      }}
      onKeyDown={(e) => {
        // Space / Enter toggle like any button, without the card's own keys (Enter opens, Delete deletes).
        if (e.key === ' ' || e.key === 'Enter') {
          e.preventDefault()
          e.stopPropagation()
          onToggle(e)
        }
      }}
    >
      {mixed ? <Minus /> : checked ? <Check /> : null}
    </button>
  )
}

interface Progress {
  label: string
  done: number
  total: number
  name?: string
}

interface Outcome<T> {
  ok: { inst: Instance; value: T }[]
  failed: { inst: Instance; error: string }[]
  cancelled: boolean
}

const live = (state: string | undefined) => state === 'running' || state === 'preparing'

/**
 * Everything that acts on several instances at once: the selection bar's buttons, the same actions as a
 * right-click menu, their dialogs, and the one cancellable progress line they share.
 */
export function useBulkActions(picked: Instance[], clear: () => void) {
  const { games, stop, refreshInstances, removeInstances, toast, activeAccount } = useStore()
  const names = useGroupNames()
  const [dialog, setDialog] = useState<'delete' | 'newGroup' | 'export'>()
  const [progress, setProgress] = useState<Progress>()
  const cancel = useRef(false)
  // "Stopped after 2 of 5", added to the next summary toast after a cancelled run.
  const stoppedNote = useRef<string>(undefined)

  const running = picked.filter((i) => live(games[i.id]))
  const idle = picked.filter((i) => !live(games[i.id]))
  const modded = picked.filter((i) => i.loader !== 'vanilla')
  const updatable = modded.filter((i) => !live(games[i.id]))
  const busy = !!progress

  /** Runs `fn` for each instance in turn, with progress and Cancel between instances; one refresh at the end. */
  const each = async <T,>(label: string, list: Instance[], fn: (i: Instance) => Promise<T>): Promise<Outcome<T>> => {
    cancel.current = false
    const out: Outcome<T> = { ok: [], failed: [], cancelled: false }
    for (const [n, inst] of list.entries()) {
      if (cancel.current) {
        out.cancelled = true
        break
      }
      setProgress({ label, done: n, total: list.length, name: inst.name })
      try {
        out.ok.push({ inst, value: await fn(inst) })
      } catch (e) {
        out.failed.push({ inst, error: (e as Error).message })
      }
    }
    setProgress(undefined)
    try {
      await refreshInstances()
    } catch (e) {
      toast((e as Error).message, 'error')
    }
    stoppedNote.current = out.cancelled ? t('library.sel.cancelled', { done: out.ok.length + out.failed.length, total: list.length }) : undefined
    if (out.cancelled && !out.ok.length) toast(stoppedNote.current!, 'info')
    if (out.failed.length) {
      const first = out.failed[0]
      toast(t('library.sel.failed', { count: out.failed.length, name: first.inst.name, error: first.error }), 'error')
    }
    return out
  }

  /** "Moved 5 instances to Modpacks · skipped 1 running · Stopped after 6 of 8". */
  const summary = (text: string, skippedRunning = 0) => {
    const parts = [text, skippedRunning ? t('library.sel.skippedRunning', { count: skippedRunning }) : '', stoppedNote.current ?? '']
    stoppedNote.current = undefined
    return parts.filter(Boolean).join(' · ')
  }

  const patchAll = async (patch: InstancePatch, label: string, done: (count: number) => string) => {
    const r = await each(label, picked, (i) => api.instances.update(i.id, patch))
    if (r.ok.length) toast(summary(done(r.ok.length)))
  }

  const moveTo = (group: string | null) =>
    patchAll({ group }, t('library.sel.moving'), (count) => t('library.sel.moved', { count, group: group || t('library.group.ungrouped') }))

  const setColor = (color: string) => patchAll({ color }, t('library.sel.coloring'), (count) => t('library.sel.colored', { count }))

  const duplicate = async () => {
    const r = await each(t('library.sel.duplicating'), picked, (i) => api.instances.duplicate(i.id))
    if (r.ok.length) toast(summary(t('library.sel.duplicated', { count: r.ok.length })))
  }

  const stopAll = async () => {
    const list = running
    for (const i of list) await stop(i.id)
    toast(t('library.sel.stopped', { count: list.length }))
  }

  const checkUpdates = async () => {
    const r = await each(t('library.sel.checking'), modded, (i) => api.content.checkUpdates(i.id))
    if (!r.ok.length) return
    const found = r.ok.reduce((n, { value }) => n + value.filter((m) => m.update).length, 0)
    const where = r.ok.filter(({ value }) => value.some((m) => m.update)).length
    toast(summary(found ? t('library.sel.updatesFound', { count: found, instances: where }) : t('library.sel.upToDate')), 'info')
  }

  const updateAll = async () => {
    const r = await each(t('library.sel.updating'), updatable, async (i) => {
      // Every mod with an update, downloaded side by side.
      const res = await api.content.updateMany(i.id)
      if (res.failed.length && !res.updated.length) throw new Error(res.failed[0].error)
      return res.updated.length
    })
    if (!r.ok.length) return
    const count = r.ok.reduce((n, { value }) => n + value, 0)
    toast(summary(count ? t('library.sel.updated', { count }) : t('library.sel.upToDate'), modded.length - updatable.length), count ? 'success' : 'info')
  }

  // Gone at once; the folders go to the Recycle Bin in the background (any that fail come back, with the reason).
  const remove = async () => {
    const skipped = running.length
    if (idle.length) toast(summary(t('library.sel.deleted', { count: idle.length }), skipped))
    clear()
    void removeInstances(idle.map((i) => i.id))
  }

  const exportAll = async (format: PackFormat) => {
    const folder = await api.content.pickExportFolder()
    if (!folder) return
    const r = await each(t('library.sel.exporting'), picked, async (i) => {
      const own = i.ownPack
      const include = (await api.content.exportPreview(i.id)).filter((e) => e.suggested).map((e) => e.name)
      return api.content.exportModpackTo(
        i.id,
        {
          format,
          name: own?.name || i.modpack?.name || i.name,
          version: own ? nextExportVersion(own) : '1.0.0',
          author: own?.author || activeAccount?.username || '',
          summary: own?.summary ?? '',
          include,
          bumpOnExport: own ? own.bumpOnExport : undefined
        },
        folder
      )
    })
    const first = r.ok[0]?.value.path
    if (first) toast(summary(t('library.sel.exported', { count: r.ok.length })), 'success', { label: t('export.showInFolder'), run: () => void api.app.showItem(first) })
  }

  const ask = (what: typeof dialog) => () => setDialog(what)

  // ---------- Entries (bar dropdowns and the right-click menu share them) ----------

  const groupItems = (): ContextEntry[] => {
    const all = (g: string) => picked.every((i) => (i.group?.trim() ?? '') === g)
    return [
      { label: t('library.group.ungrouped'), checked: all(''), onSelect: () => void moveTo(null) },
      ...names.map((g): ContextItem => ({ label: g, checked: all(g), onSelect: () => void moveTo(g) })),
      '-',
      { label: t('menu.instance.newGroup'), icon: <FolderPlus />, onSelect: ask('newGroup') }
    ]
  }

  const colorItems = (): ContextEntry[] =>
    COLORS.map((c, i) => ({
      label: t(COLOR_NAMES[i]),
      icon: <span className="ctx-swatch" style={{ background: c }} />,
      checked: picked.every((p) => p.color === c),
      onSelect: () => void setColor(c)
    }))

  /** Why an action skips some of the selection (or can't run for any of it). */
  const skips = (left: number, reason: 'running' | 'vanilla'): string | undefined =>
    left ? t(reason === 'running' ? 'library.sel.skipRunning' : 'library.sel.skipVanilla', { count: left }) : undefined

  const updatesHint = (): string | undefined => {
    if (!modded.length) return t('library.sel.allVanilla')
    if (!updatable.length) return t('library.sel.allRunning')
    return [skips(picked.length - modded.length, 'vanilla'), skips(modded.length - updatable.length, 'running')].filter(Boolean).join(' · ') || undefined
  }

  const modItems = (): ContextEntry[] => [
    {
      label: t('library.sel.checkUpdates'),
      icon: <RefreshCw />,
      disabled: busy || !modded.length,
      hint: modded.length ? skips(picked.length - modded.length, 'vanilla') : t('library.sel.allVanilla'),
      onSelect: () => void checkUpdates()
    },
    { label: t('library.sel.updateAll'), icon: <ArrowUpCircle />, disabled: busy || !updatable.length, hint: updatesHint(), onSelect: () => void updateAll() }
  ]

  const entries = (): ContextEntry[] => [
    running.length > 0 && { label: t('library.sel.stop', { count: running.length }), icon: <Square />, onSelect: () => void stopAll() },
    running.length > 0 && '-',
    { label: t('library.sel.moveTo'), icon: <FolderInput />, disabled: busy, items: groupItems() },
    { label: t('menu.instance.color'), icon: <Palette />, disabled: busy, items: colorItems() },
    { label: t('library.sel.duplicate', { count: picked.length }), icon: <Copy />, disabled: busy, onSelect: () => void duplicate() },
    { label: t('library.sel.export', { count: picked.length }), icon: <PackageOpen />, disabled: busy, onSelect: ask('export') },
    '-',
    ...modItems(),
    '-',
    {
      label: t('library.sel.delete', { count: idle.length || picked.length }),
      icon: <Trash2 />,
      danger: true,
      shortcut: 'Del',
      disabled: busy || !idle.length,
      hint: idle.length ? skips(running.length, 'running') : t('library.sel.allRunning'),
      onSelect: ask('delete')
    }
  ]

  const dropdown = (entries: () => ContextEntry[]) => (e: React.MouseEvent<HTMLButtonElement>) =>
    openMenuBelow(e.currentTarget, entries(), e.detail === 0)

  const buttons = (
    <>
      {running.length > 0 && (
        <button className="btn sm" disabled={busy} onClick={() => void stopAll()}>
          <Square /> {t('library.sel.stop', { count: running.length })}
        </button>
      )}
      <button className="btn sm" disabled={busy} aria-haspopup="menu" onClick={dropdown(groupItems)}>
        <FolderInput /> {t('library.sel.moveTo')} <ChevronDown />
      </button>
      <button className="btn sm" disabled={busy} aria-haspopup="menu" onClick={dropdown(colorItems)}>
        <Palette /> {t('library.sel.color')} <ChevronDown />
      </button>
      <button className="btn sm" disabled={busy} onClick={() => void duplicate()}>
        <Copy /> {t('library.card.duplicate')}
      </button>
      <button className="btn sm" disabled={busy || !modded.length} title={updatesHint()} aria-haspopup="menu" onClick={dropdown(modItems)}>
        <Blocks /> {t('library.sel.mods')} <ChevronDown />
      </button>
      <button className="btn sm" disabled={busy} onClick={ask('export')}>
        <PackageOpen /> {t('library.sel.exportShort')}
      </button>
      <button
        className="btn sm danger"
        disabled={busy || !idle.length}
        title={idle.length ? skips(running.length, 'running') : t('library.sel.allRunning')}
        onClick={ask('delete')}
      >
        <Trash2 /> {t('common.delete')}
      </button>
    </>
  )

  const close = () => setDialog(undefined)
  const dialogs = (
    <>
      {dialog === 'delete' && <BulkDelete list={idle} skipped={running} onClose={close} onConfirm={async () => void remove()} />}
      {dialog === 'export' && <BulkExport count={picked.length} onClose={close} onRun={(f) => void exportAll(f)} />}
      {dialog === 'newGroup' && (
        <NameModal
          title={t('library.group.createTitle')}
          initial=""
          placeholder={t('library.group.new')}
          onClose={close}
          onSave={async (group) => {
            addSavedGroup(group)
            void moveTo(group)
          }}
        />
      )}
    </>
  )

  return {
    entries,
    buttons,
    dialogs,
    progress,
    cancel: () => (cancel.current = true),
    askDelete: () => {
      if (idle.length && !busy) setDialog('delete')
    },
    /** Opens the selection's menu at a right-click. */
    open: (e: React.MouseEvent) => openContextMenu(e, entries(), t('menu.selected', { count: picked.length }))
  }
}

/** The bar under the Library while anything is picked: count, Select all, Clear, the actions, or a running action's progress. */
export function SelectionBar({
  count,
  total,
  bulk,
  onSelectAll,
  onClear
}: {
  count: number
  total: number
  bulk: ReturnType<typeof useBulkActions>
  onSelectAll: () => void
  onClear: () => void
}) {
  const p = bulk.progress
  return (
    <div className="sel-bar" role="toolbar" aria-label={t('library.sel.toolbar')} onContextMenu={(e) => e.stopPropagation()}>
      <button className="btn ghost icon sm" onClick={onClear} disabled={!!p} title={t('library.sel.clear')} aria-label={t('library.sel.clear')}>
        <X />
      </button>
      <b className="sel-count">{t('menu.selected', { count })}</b>
      {count < total && !p && (
        <button className="btn ghost sm" onClick={onSelectAll}>
          {t('library.sel.selectAll')}
        </button>
      )}
      <span className="spacer" />
      {p ? (
        <div className="sel-progress" aria-live="polite">
          <Loader2 className="spin" />
          <div className="grow">
            <div className="ellipsis">
              {p.label} <span className="faint">{t('library.sel.progress', { done: p.done + 1, total: p.total })}</span>
            </div>
            {p.name && <div className="faint ellipsis sel-progress-name">{p.name}</div>}
            <div className="bar">
              <i style={{ width: `${(p.done / p.total) * 100}%` }} />
            </div>
          </div>
          <button className="btn sm" onClick={bulk.cancel}>
            {t('app.task.cancel')}
          </button>
        </div>
      ) : (
        <div className="sel-actions">{bulk.buttons}</div>
      )}
    </div>
  )
}

function BulkDelete({ list, skipped, onClose, onConfirm }: { list: Instance[]; skipped: Instance[]; onClose: () => void; onConfirm: () => Promise<void> }) {
  const shown = list.slice(0, 8)
  return (
    <ConfirmModal
      title={t('library.sel.deleteTitle', { count: list.length })}
      body={
        <>
          <ul className="sel-names">
            {shown.map((i) => (
              <li key={i.id}>{i.name}</li>
            ))}
            {list.length > shown.length && <li className="faint">{t('library.sel.andMore', { count: list.length - shown.length })}</li>}
          </ul>
          <p>{t('library.sel.deleteBody')}</p>
          {skipped.length > 0 && <p className="hint">{t('library.sel.deleteSkipped', { count: skipped.length, names: joinNames(skipped) })}</p>}
        </>
      }
      confirm={t('library.delete.confirm')}
      onClose={onClose}
      onConfirm={onConfirm}
    />
  )
}

const joinNames = (list: Instance[]) => list.map((i) => i.name).join(', ')

function BulkExport({ count, onClose, onRun }: { count: number; onClose: () => void; onRun: (format: PackFormat) => void }) {
  const { info } = useStore()
  const [format, setFormat] = useState<PackFormat>('mrpack')
  return (
    <Modal
      title={t('library.sel.exportTitle', { count })}
      onClose={onClose}
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button
            className="btn primary"
            onClick={() => {
              onClose()
              onRun(format)
            }}
          >
            <PackageOpen /> {t('library.sel.exportRun')}
          </button>
        </>
      }
    >
      <div className="field">
        <label>{t('export.format')}</label>
        <Segmented
          value={format}
          onChange={setFormat}
          options={[
            { value: 'mrpack', label: 'Modrinth (.mrpack)' },
            { value: 'curseforge', label: 'CurseForge (.zip)' }
          ]}
        />
        <span className="hint">{format === 'mrpack' ? t('export.mrpackHint') : info?.curseforge ? t('export.curseforgeHint') : t('export.curseforgeOffline')}</span>
      </div>
      <p className="muted">{t('library.sel.exportBody')}</p>
    </Modal>
  )
}
