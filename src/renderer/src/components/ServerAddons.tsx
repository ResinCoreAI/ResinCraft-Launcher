import { useCallback, useEffect, useMemo, useState } from 'react'
import {
  ArrowUpCircle,
  Check,
  ChevronDown,
  Copy,
  ExternalLink,
  FolderOpen,
  ListChecks,
  Loader2,
  Plus,
  Power,
  PowerOff,
  Puzzle,
  RefreshCw,
  Repeat,
  Search,
  Trash2,
  X
} from 'lucide-react'
import type { AddonArea, ServerAddon, ServerInfo } from '@shared/types'
import { PLUGIN_SERVERS } from '@shared/types'
import { api, isConsole } from '../api'
import { useStore } from '../store'
import { bytes, projectUrl } from '../util'
import { rich, t, type Key } from '../i18n'
import AddonBrowser, { ADDON_SOURCE_LABEL } from './AddonBrowser'
import { SERVER_LABEL } from './NewServerModal'
import VersionPicker from './VersionPicker'
import { ConfirmModal, Empty, openContextMenu, Switch, useCopy, type ContextEntry } from './ui'

const isLive = (s: ServerInfo) => s.state === 'starting' || s.state === 'running' || s.state === 'stopping'
const nameOf = (a: ServerAddon) => a.title ?? a.filename.replace(/\.jar(\.disabled)?$/, '')

/** A newer compatible version found by the last update check, per project. */
interface Found {
  update: NonNullable<ServerAddon['update']>
  url?: string
}

/**
 * The server's Plugins (or Mods) tab: a manager like the instance mod list. Version picker per jar, update check
 * with Update / Update all, multi-select for bulk actions. While the server runs, changes are queued and applied
 * when it stops (Java keeps its jars open). Hybrid servers (Arclight) show one for each `area`.
 */
export default function ServerAddons({ s, area }: { s: ServerInfo; area?: AddonArea }) {
  const { toast } = useStore()
  const [items, setItems] = useState<ServerAddon[]>()
  const [found, setFound] = useState<Record<string, Found>>({})
  const [q, setQ] = useState('')
  const [browsing, setBrowsing] = useState(false)
  const [removing, setRemoving] = useState<ServerAddon[]>()
  const [picking, setPicking] = useState<ServerAddon>()
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [checking, setChecking] = useState(false)
  const [busy, setBusy] = useState<Set<string>>(new Set())
  const plugins = area ? area === 'plugins' : PLUGIN_SERVERS.includes(s.type)
  const target = `${SERVER_LABEL[s.type]} ${s.mcVersion}`

  const load = useCallback(async () => {
    try {
      const list = await api.server.addons(s.id, area)
      setItems(list)
      setSelected((sel) => new Set([...sel].filter((f) => list.some((a) => a.filename === f))))
    } catch (e) {
      setItems([])
      toast((e as Error).message, 'error')
    }
  }, [s.id, area, toast])
  useEffect(() => {
    void load()
  }, [load])

  // Update info survives reloads (file names change when a jar is updated) until that version is installed.
  const shown = useMemo(
    () =>
      (items ?? [])
        .map((a) => {
          const f = a.projectId ? found[a.projectId] : undefined
          return f && f.update.versionId !== a.versionId && a.pending !== 'update' ? { ...a, update: f.update, updateUrl: f.url } : { ...a, update: undefined }
        })
        .filter((a) => `${a.title ?? ''} ${a.filename}`.toLowerCase().includes(q.toLowerCase())),
    [items, found, q]
  )
  const updatable = shown.filter((a) => a.update)
  const chosen = shown.filter((a) => selected.has(a.filename))
  const installed = useMemo(() => new Set((items ?? []).map((i) => i.projectId).filter((x): x is string => !!x)), [items])

  const mark = (names: string[], on: boolean) =>
    setBusy((b) => {
      const n = new Set(b)
      names.forEach((x) => (on ? n.add(x) : n.delete(x)))
      return n
    })

  const checkUpdates = async () => {
    setChecking(true)
    try {
      const res = await api.server.addonCheckUpdates(s.id, area)
      const next: Record<string, Found> = {}
      for (const a of res) if (a.update && a.projectId) next[a.projectId] = { update: a.update, url: a.updateUrl }
      setFound(next)
      setItems(res)
      const n = Object.keys(next).length
      toast(n ? t('servers.addons.updatesFound', { count: n }) : t('servers.addons.upToDate'), 'info')
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setChecking(false)
    }
  }

  /** Switches each jar to a version; reports how many were done now and how many wait for a restart. */
  const switchAll = async (list: { a: ServerAddon; versionId: string }[]) => {
    let now = 0
    let queued = 0
    // Several at once: the engine downloads them side by side and swaps them together. The web console keeps
    // one at a time (the launcher on the other PC may not take the batch call yet).
    if (list.length > 1 && !isConsole) {
      mark(
        list.map(({ a }) => a.filename),
        true
      )
      try {
        const r = await api.server.addonUpdateMany(s.id, list.map(({ a, versionId }) => ({ filename: a.filename, versionId })), area)
        now = r.now.length
        queued = r.queued.length
        for (const f of r.failed) {
          const a = list.find((x) => x.a.filename === f.filename)?.a
          toast(t('servers.addons.failed', { name: a ? nameOf(a) : f.filename, error: f.error }), 'error')
        }
      } catch (e) {
        toast((e as Error).message, 'error')
      } finally {
        mark(
          list.map(({ a }) => a.filename),
          false
        )
      }
      if (now) toast(t('servers.addons.updatedCount', { count: now }))
      if (queued) toast(t('servers.addons.queuedCount', { count: queued }), 'info')
      await load()
      return
    }
    for (const { a, versionId } of list) {
      mark([a.filename], true)
      try {
        if (await api.server.addonChangeVersion(s.id, a.filename, versionId, area)) queued++
        else now++
      } catch (e) {
        toast(t('servers.addons.failed', { name: nameOf(a), error: (e as Error).message }), 'error')
      } finally {
        mark([a.filename], false)
      }
    }
    if (now) toast(t('servers.addons.updatedCount', { count: now }))
    if (queued) toast(t('servers.addons.queuedCount', { count: queued }), 'info')
    await load()
  }
  const update = (list: ServerAddon[]) => switchAll(list.filter((a) => a.update).map((a) => ({ a, versionId: a.update!.versionId })))

  const toggleMany = async (list: ServerAddon[], enable: boolean) => {
    const todo = list.filter((a) => a.enabled !== enable && a.pending !== (enable ? 'enable' : 'disable'))
    mark(
      todo.map((a) => a.filename),
      true
    )
    for (const a of todo) {
      try {
        await api.server.addonToggle(s.id, a.filename, area)
      } catch (e) {
        toast(t('servers.addons.failed', { name: nameOf(a), error: (e as Error).message }), 'error')
      }
    }
    mark(
      todo.map((a) => a.filename),
      false
    )
    if (isLive(s) && todo.length) toast(t('servers.addons.queuedCount', { count: todo.length }), 'info')
    await load()
  }

  const toggleSelect = (a: ServerAddon) =>
    setSelected((sel) => {
      const n = new Set(sel)
      if (n.has(a.filename)) n.delete(a.filename)
      else n.add(a.filename)
      return n
    })
  const allSelected = shown.length > 0 && shown.every((a) => selected.has(a.filename))

  // Right-clicking a row: its actions, applied to every ticked row when it's one of several ticked.
  const copy = useCopy()
  const addonMenu = (a: ServerAddon): ContextEntry[] => {
    const list = selected.has(a.filename) && chosen.length > 1 ? chosen : [a]
    const one = list.length === 1
    const idle = busy.size === 0
    const on = list.filter((x) => x.enabled)
    const off = list.filter((x) => !x.enabled)
    const outdated = list.filter((x) => x.update)
    const page = one ? projectUrl(a.projectId) : undefined
    return [
      one
        ? {
            label: t(a.enabled ? 'servers.addons.disable' : 'servers.addons.enable'),
            icon: a.enabled ? <PowerOff /> : <Power />,
            disabled: !idle,
            onSelect: () => void toggleMany([a], !a.enabled)
          }
        : null,
      !one && off.length
        ? { label: t('menu.content.enableMany', { count: off.length }), icon: <Power />, disabled: !idle, onSelect: () => void toggleMany(off, true) }
        : null,
      !one && on.length
        ? { label: t('menu.content.disableMany', { count: on.length }), icon: <PowerOff />, disabled: !idle, onSelect: () => void toggleMany(on, false) }
        : null,
      outdated.length
        ? {
            label:
              one && a.update ? t('servers.addons.updateTo', { version: a.update.versionNumber }) : t('menu.content.updateMany', { count: outdated.length }),
            icon: <ArrowUpCircle />,
            disabled: !idle,
            onSelect: () => void update(outdated)
          }
        : list.some((x) => x.source)
          ? { label: t('servers.addons.checkUpdates'), icon: <RefreshCw />, disabled: checking, onSelect: () => void checkUpdates() }
          : null,
      one && a.projectId && a.source ? { label: t('menu.content.changeVersion'), icon: <Repeat />, disabled: !idle, onSelect: () => setPicking(a) } : null,
      '-',
      page ? { label: t('menu.openProjectPage'), icon: <ExternalLink />, onSelect: () => void api.app.openExternal(page) } : null,
      one
        ? {
            label: t('files.showInExplorer'),
            icon: <FolderOpen />,
            onSelect: () =>
              void api.files.reveal({ kind: 'server', id: s.id }, `${plugins ? 'plugins' : 'mods'}/${a.filename}`).catch((e) => toast(e.message, 'error'))
          }
        : null,
      {
        label: one ? t('menu.copyFileName') : t('menu.copyFileNames', { count: list.length }),
        icon: <Copy />,
        onSelect: () => void copy(list.map((x) => x.filename).join('\n'))
      },
      '-',
      { label: t('common.remove'), icon: <Trash2 />, danger: true, onSelect: () => setRemoving(list) }
    ]
  }

  return (
    <>
      {isLive(s) && (
        <div className="callout" style={{ marginBottom: 12 }}>
          {t('servers.addons.restartHint')}
        </div>
      )}
      <div className="toolbar">
        {shown.length > 0 && (
          <button
            className={`check ${allSelected ? 'on' : ''}`}
            role="checkbox"
            aria-checked={allSelected}
            aria-label={t('servers.addons.selectAll')}
            title={t('servers.addons.selectAll')}
            onClick={() => setSelected(allSelected ? new Set() : new Set(shown.map((a) => a.filename)))}
          >
            {allSelected && <Check strokeWidth={3} />}
          </button>
        )}
        <div className="search">
          <Search />
          <input
            className="input"
            placeholder={t(plugins ? 'servers.addons.filterPlugins' : 'servers.addons.filterMods')}
            value={q}
            onChange={(e) => setQ(e.target.value)}
          />
        </div>
        {updatable.length > 0 ? (
          <button className="btn" disabled={busy.size > 0} onClick={() => void update(updatable)}>
            <ArrowUpCircle /> {t('servers.addons.updateAll', { count: updatable.length })}
          </button>
        ) : (
          <button className="btn" disabled={checking || !items?.some((a) => a.source)} onClick={checkUpdates}>
            {checking ? <Loader2 className="spin" /> : <RefreshCw />} {t('servers.addons.checkUpdates')}
          </button>
        )}
        <button data-pc-only className="btn icon" title={t('common.openFolder')} onClick={() => void api.server.openFolder(s.id, plugins ? 'plugins' : 'mods')}>
          <FolderOpen />
        </button>
        <button className="btn primary" onClick={() => setBrowsing(true)}>
          <Plus /> {t(plugins ? 'servers.addons.addPlugins' : 'servers.addons.addMods')}
        </button>
      </div>

      {!items ? (
        <div className="results">
          <div className="skeleton" style={{ height: 62 }} />
        </div>
      ) : items.length === 0 ? (
        <Empty
          icon={<Puzzle />}
          title={t(plugins ? 'servers.addons.emptyPlugins' : 'servers.addons.emptyMods')}
          action={
            <button className="btn primary" onClick={() => setBrowsing(true)}>
              <Plus /> {t(plugins ? 'servers.addons.addPlugins' : 'servers.addons.addMods')}
            </button>
          }
        >
          {t(plugins ? 'servers.addons.emptyPluginsHint' : 'servers.addons.emptyModsHint')}
        </Empty>
      ) : (
        <div className="list">
          {shown.map((a) => {
            const on = selected.has(a.filename)
            const working = busy.has(a.filename)
            return (
              <div
                key={a.filename}
                className={`list-row sv-addon ${a.enabled ? '' : 'off'} ${on ? 'selected' : ''}`}
                onContextMenu={(e) => {
                  const many = on && chosen.length > 1
                  openContextMenu(e, addonMenu(a), many ? t('menu.selected', { count: chosen.length }) : undefined)
                }}
              >
                <button
                  className={`check ${on ? 'on' : ''}`}
                  role="checkbox"
                  aria-checked={on}
                  aria-label={t('servers.addons.select', { name: nameOf(a) })}
                  onClick={() => toggleSelect(a)}
                >
                  {on && <Check strokeWidth={3} />}
                </button>
                <div className="ico">
                  {a.iconUrl ? <img src={a.iconUrl} alt="" onError={(e) => ((e.target as HTMLImageElement).style.display = 'none')} /> : <Puzzle />}
                </div>
                <div className="grow" style={{ minWidth: 0 }}>
                  <div className="title ellipsis">
                    {nameOf(a)}
                    {a.source ? (
                      <span className={`sv-src ${a.source}`}>{ADDON_SOURCE_LABEL[a.source]}</span>
                    ) : (
                      <span className="sv-src" title={t('servers.addons.unknownSourceHint')}>
                        {t('servers.addons.unknownSource')}
                      </span>
                    )}
                    {a.update && <span className="sv-badge">{t('servers.addons.updateBadge', { version: a.update.versionNumber })}</span>}
                  </div>
                  <div className="file ellipsis">
                    {a.filename} Â· {bytes(a.size)}
                    {a.versionNumber ? ` Â· ${a.versionNumber}` : ''}
                  </div>
                  {a.pending && (
                    <div className="sv-pending">
                      {t(`servers.addons.pending.${a.pending}` as Key, { version: a.pendingVersion ?? '' })}
                      <button
                        className="link"
                        onClick={async () => {
                          await api.server.addonCancelPending(s.id, a.filename, area)
                          await load()
                        }}
                      >
                        <X size={12} /> {t('servers.addons.cancelPending')}
                      </button>
                    </div>
                  )}
                </div>
                {a.update && (
                  <>
                    {a.updateUrl && (
                      <button className="btn ghost icon sm" title={t('servers.addons.releaseNotes')} onClick={() => void api.app.openExternal(a.updateUrl!)}>
                        <ExternalLink />
                      </button>
                    )}
                    <button
                      className="btn sm primary"
                      disabled={working}
                      title={t('servers.addons.updateTo', { version: a.update.versionNumber })}
                      onClick={() => void update([a])}
                    >
                      {working ? <Loader2 className="spin" /> : <ArrowUpCircle />} {t('servers.addons.update')}
                    </button>
                  </>
                )}
                {a.projectId && a.source ? (
                  <button className="btn sm ver-btn" disabled={working} title={t('servers.addons.changeVersion')} onClick={() => setPicking(a)}>
                    {working && !a.update ? <Loader2 className="spin" /> : null}
                    {a.versionNumber ?? t('servers.addons.version')} <ChevronDown />
                  </button>
                ) : null}
                <Switch
                  on={a.enabled}
                  label={a.enabled ? t('servers.addons.disable') : t('servers.addons.enable')}
                  onChange={() => void toggleMany([a], !a.enabled)}
                />
                <button className="btn ghost icon sm danger" title={t('common.remove')} onClick={() => setRemoving([a])}>
                  <Trash2 />
                </button>
              </div>
            )
          })}
        </div>
      )}

      {chosen.length > 0 && (
        <div className="select-bar">
          <ListChecks size={18} />
          <span>{rich('servers.addons.selection', { b: (x) => <b>{x}</b> }, { count: chosen.length })}</span>
          <button className="btn ghost sm" onClick={() => setSelected(new Set())}>
            {t('servers.addons.clearSelection')}
          </button>
          {chosen.some((a) => a.update) && (
            <button className="btn primary sm" disabled={busy.size > 0} onClick={() => void update(chosen)}>
              <ArrowUpCircle /> {t('servers.addons.updateSelected', { count: chosen.filter((a) => a.update).length })}
            </button>
          )}
          <button className="btn sm" disabled={busy.size > 0} onClick={() => void toggleMany(chosen, true)}>
            <Power /> {t('servers.addons.enable')}
          </button>
          <button className="btn sm" disabled={busy.size > 0} onClick={() => void toggleMany(chosen, false)}>
            <PowerOff /> {t('servers.addons.disable')}
          </button>
          <button className="btn sm danger" disabled={busy.size > 0} onClick={() => setRemoving(chosen)}>
            <Trash2 /> {t('common.remove')}
          </button>
        </div>
      )}

      {picking?.projectId && (
        <VersionPicker
          projectId={picking.projectId}
          title={nameOf(picking)}
          target={target}
          load={() => api.server.addonVersions(s.id, picking.projectId!, area)}
          currentVersionId={picking.versionId}
          onClose={() => setPicking(undefined)}
          onPick={async (versionId) => {
            try {
              mark([picking.filename], true)
              const queued = await api.server.addonChangeVersion(s.id, picking.filename, versionId, area)
              toast(
                queued ? t('servers.addons.queuedCount', { count: 1 }) : t('servers.addons.switched', { name: nameOf(picking) }),
                queued ? 'info' : 'success'
              )
              await load()
            } catch (e) {
              toast((e as Error).message, 'error')
              throw e
            } finally {
              mark([picking.filename], false)
            }
          }}
        />
      )}
      {browsing && <AddonBrowser server={s} area={area} installed={installed} onInstalled={() => void load()} onClose={() => setBrowsing(false)} />}
      {removing && (
        <ConfirmModal
          title={
            removing.length === 1 ? t('servers.addons.removeTitle', { name: nameOf(removing[0]) }) : t('servers.addons.removeMany', { count: removing.length })
          }
          body={isLive(s) ? t('servers.addons.removeBodyRunning') : t('servers.addons.removeBody')}
          confirm={t('common.remove')}
          onClose={() => setRemoving(undefined)}
          onConfirm={async () => {
            for (const a of removing) {
              try {
                await api.server.addonRemove(s.id, a.filename, area)
              } catch (e) {
                toast(t('servers.addons.failed', { name: nameOf(a), error: (e as Error).message }), 'error')
              }
            }
            setSelected(new Set())
            await load()
          }}
        />
      )}
    </>
  )
}
