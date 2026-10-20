// The automatic modpack update check: a quiet check soon after start and every few hours (the engine caches results
// and goes easy on the sources, see src-tauri/src/content/packupdates.rs), one notification listing what has updates,
// the "Update available" / "Sync available" badges on Library and server cards, and its switch in Settings → Updates.
import { useEffect, useState, useSyncExternalStore } from 'react'
import { ArrowUpCircle, PackageCheck, RefreshCw } from 'lucide-react'
import type { PackUpdate, PackUpdates } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { t } from '../i18n'
import { timeAgo } from '../util'
import { Switch } from './ui'
import { instanceOwner, serverOwner, useSwitchPack } from './PackVersion'

const EVERY = 4 * 3600_000
const START_DELAY = 8000
const HIDDEN_KEY = 'resin.packUpdates.hidden'

let snapshot: { data?: PackUpdates; checking: boolean } = { checking: false }
const subs = new Set<() => void>()
const publish = (patch: Partial<typeof snapshot>) => {
  snapshot = { ...snapshot, ...patch }
  subs.forEach((f) => f())
}
const subscribe = (f: () => void) => {
  subs.add(f)
  return () => void subs.delete(f)
}
export const usePackUpdates = () => useSyncExternalStore(subscribe, () => snapshot)

/** Asks the engine again (it answers from its cache unless `force` or the cache is old). */
export async function refreshPackUpdates(force = false): Promise<void> {
  publish({ checking: true })
  const data = await api.content.packUpdates(force).catch(() => undefined)
  publish({ checking: false, ...(data ? { data } : {}) })
}

const keyOf = (u: PackUpdate) => `${u.target}:${u.id}:${u.latestId ?? 'sync'}`

function loadHidden(): string[] {
  try {
    return JSON.parse(localStorage.getItem(HIDDEN_KEY) ?? '[]') as string[]
  } catch {
    return []
  }
}

let started = false
function startSchedule() {
  if (started) return
  started = true
  setTimeout(() => void refreshPackUpdates(), START_DELAY)
  // Every 15 minutes, look again once 4 hours have passed (also right after the computer wakes up).
  setInterval(() => {
    const last = snapshot.data?.checkedAt ?? 0
    if (Date.now() - last >= EVERY && !snapshot.checking) void refreshPackUpdates()
  }, 15 * 60_000)
}

/** The update entry for an instance or server, if the last check found one. */
export function usePackUpdate(target: 'instance' | 'server', id: string): PackUpdate | undefined {
  const { data } = usePackUpdates()
  return data?.updates.find((u) => u.target === target && u.id === id)
}

/** "Update available" / "Sync available" on a Library or server card. */
export function PackUpdateBadge({ target, id, compact }: { target: 'instance' | 'server'; id: string; compact?: boolean }) {
  const u = usePackUpdate(target, id)
  if (!u) return null
  const label = t(u.kind === 'sync' ? 'packUpdates.syncAvailable' : 'packVersion.updateAvailable')
  return (
    <span
      className={`chip success pack-update-badge ${compact ? 'compact' : ''}`}
      title={u.latest ? t('packVersion.updateTo', { version: u.latest }) : label}
      aria-label={label}
    >
      <ArrowUpCircle />
      {!compact && ` ${label}`}
    </span>
  )
}

/** Mounted in App: starts the schedule and lists what has updates in one notification. */
export function PackUpdatesBanner() {
  const { data } = usePackUpdates()
  const { instances, games, go } = useStore()
  const [hidden, setHidden] = useState(loadHidden)
  const run = useSwitchPack()
  useEffect(startSchedule, [])

  const shown = (data?.updates ?? []).filter((u) => !hidden.includes(keyOf(u)))
  if (!shown.length) return null
  const hide = (keys: string[]) => {
    const next = [...new Set([...hidden, ...keys])]
    setHidden(next)
    try {
      localStorage.setItem(HIDDEN_KEY, JSON.stringify(next.slice(-200)))
    } catch {
      // Private window: hidden until the app restarts.
    }
  }
  const open = (u: PackUpdate) => (u.target === 'instance' ? go({ page: 'instance', id: u.id, tab: 'settings' }) : go({ page: 'servers', id: u.id }))
  const update = async (u: PackUpdate) => {
    hide([keyOf(u)])
    if (u.target === 'instance') {
      const inst = instances.find((i) => i.id === u.id)
      if (inst?.modpack) await run(instanceOwner(inst, games), u.latestId!)
    } else {
      const s = await api.server.get(u.id).catch(() => undefined)
      if (s?.modpack) await run(serverOwner(s), u.latestId!)
    }
  }

  return (
    <div className="update-banner pack-updates" role="status">
      <PackageCheck className="icon" />
      <div className="grow">
        <b>{t('packUpdates.title', { count: shown.length })}</b>
        <ul className="pack-updates-list">
          {shown.slice(0, 4).map((u) => (
            <li key={keyOf(u)}>
              <div className="ellipsis">
                <span>{u.name}</span>
                <span className="hint mono"> {u.kind === 'sync' ? t('packUpdates.instanceChanged') : `${u.current ?? '?'} → ${u.latest}`}</span>
              </div>
              <div className="row actions">
                {u.kind === 'pack' && (
                  <button className="btn sm primary" onClick={() => void update(u)}>
                    {t('common.update')}
                  </button>
                )}
                <button className="btn sm ghost" onClick={() => open(u)}>
                  {t(u.kind === 'sync' ? 'packUpdates.review' : 'packVersion.changeEllipsis')}
                </button>
              </div>
            </li>
          ))}
        </ul>
        {shown.length > 4 && <p>{t('packUpdates.more', { count: shown.length - 4 })}</p>}
        <div className="row actions">
          <button className="btn sm ghost" onClick={() => hide(shown.map(keyOf))}>
            {t('updates.later')}
          </button>
        </div>
      </div>
    </div>
  )
}

/** Settings → Updates: the automatic modpack check. */
export function PackUpdatesSetting() {
  const { settings, updateSettings } = useStore()
  const { data, checking } = usePackUpdates()
  if (!settings) return null
  const line = checking
    ? t('packUpdates.checking')
    : data?.checkedAt
      ? data.updates.length
        ? t('packUpdates.found', { count: data.updates.length, when: timeAgo(data.checkedAt) })
        : t('packUpdates.none', { when: timeAgo(data.checkedAt) })
      : ''
  return (
    <div className="setting">
      <div className="text">
        <b>{t('packUpdates.setting')}</b>
        <span>
          {t('packUpdates.settingHint')}
          {line && <> · {line}</>}
        </span>
      </div>
      <div className="control">
        <button className="btn sm ghost" disabled={checking} onClick={() => void refreshPackUpdates(true)}>
          <RefreshCw className={checking ? 'spin' : ''} /> {t('packUpdates.checkNow')}
        </button>
        <Switch on={settings.autoCheckPacks} onChange={(v) => void updateSettings({ autoCheckPacks: v })} label={t('packUpdates.setting')} />
      </div>
    </div>
  )
}
