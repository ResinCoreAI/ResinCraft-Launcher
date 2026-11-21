import { useCallback, useEffect, useMemo, useState } from 'react'
import { AlertTriangle, Archive, ArchiveRestore, ExternalLink, FolderOpen, History, Loader2, RefreshCw, Search, Undo2 } from 'lucide-react'
import type { BedrockBackup, BedrockEdition, BedrockStatus, BedrockSwitchBlocker, BedrockSwitchPlan, BedrockVersion } from '@shared/types'
import { api } from '../api'
import { useInstanceTask, useStore } from '../store'
import { fixed, timeAgo } from '../util'
import { EngineError, t, type Key } from '../i18n'
import { ConfirmModal, Modal, Segmented } from './ui'

/** Why something on the Minecraft for Windows card failed, with the engine's key to pick the way out. */
export type BedrockFailure = { message: string; key?: string }

const GIB = 1024 * 1024 * 1024

/** A size for people: megabytes, or gigabytes from 1 GB up. */
function size(n: number): string {
  return n >= GIB ? t('common.size.gb', { n: fixed(n / GIB, 1) }) : t('common.size.mb', { n: Math.max(1, Math.round(n / (1024 * 1024))) })
}

/** "1.21.9" before "1.21.10". */
function compareVersions(a: string, b: string): number {
  const [x, y] = [a, b].map((v) => v.split('.').map(Number))
  for (let i = 0; i < Math.max(x.length, y.length); i++) if ((x[i] ?? 0) !== (y[i] ?? 0)) return (x[i] ?? 0) - (y[i] ?? 0)
  return 0
}

const BLOCKER_TEXT: Record<BedrockSwitchBlocker, Key> = {
  notInstalled: 'bedrock.switch.block.notInstalled',
  sameVersion: 'bedrock.switch.block.sameVersion',
  running: 'bedrock.switch.block.running',
  developerMode: 'bedrock.switch.block.developerMode',
  gamingServices: 'bedrock.switch.block.gamingServices',
  diskSpace: 'bedrock.switch.block.diskSpace'
}

/** How many backups the list shows before "Show all". */
const BACKUPS_SHOWN = 4

type Props = {
  /** Minecraft for Windows as the page's status has it (none while the status loads). */
  release?: BedrockEdition
  status?: BedrockStatus
  /** Reads the page's status again. */
  refresh: () => Promise<void>
  /** Shows (or clears) a failure on the Minecraft for Windows card. */
  onFailure: (f?: BedrockFailure) => void
}

/**
 * Switching Minecraft for Windows to another version and its world backups, for the Bedrock page to place: the
 * version plate on the cover (`VersionPlate`), the backups card beside Preview (`BackupsCard`) and the backups list
 * (`BackupsList`) all share this. It says exactly what a switch will do and what is in the way before doing it, has
 * the way back to the Store version, and Restore. The engine does the work as tasks tied to `bedrock:release`, which
 * the cover shows. `modals` must be rendered once by the page.
 */
export function useBedrockVersions({ release, status, refresh, onFailure }: Props) {
  const { toast } = useStore()
  const task = useInstanceTask('bedrock:release')
  const previewTask = useInstanceTask('bedrock:preview')
  const [picking, setPicking] = useState(false)
  const [goingBack, setGoingBack] = useState(false)
  const [backups, setBackups] = useState<BedrockBackup[]>()
  const [allBackups, setAllBackups] = useState(false)
  const [restoring, setRestoring] = useState<BedrockBackup>()

  const loadBackups = useCallback(
    () =>
      api.bedrock
        .backups()
        .then(setBackups)
        .catch(() => setBackups([])),
    []
  )
  useEffect(() => void loadBackups(), [loadBackups])

  /** Runs one of the engine's tasks: a failure goes on the card, and the page and the backups are read again. */
  const run = async (work: () => Promise<void>) => {
    onFailure(undefined)
    try {
      await work()
    } catch (e) {
      const key = e instanceof EngineError ? e.key : undefined
      if (key !== 'task.cancelled') onFailure({ message: (e as Error).message, key })
    } finally {
      await Promise.all([refresh().catch(() => {}), loadBackups()])
    }
  }

  const switchTo = (version: string) =>
    run(async () => {
      const r = await api.bedrock.switchTo(version)
      toast(r.backup ? t('bedrock.switchedBackup', { version: r.version, folder: r.backup.path }) : t('bedrock.switched', { version: r.version }), 'success')
    })

  const backToStore = () =>
    run(async () => {
      const r = await api.bedrock.backToStore()
      toast(t('bedrock.backToStoreDone', { version: r.version }), 'success')
    })

  const backUpNow = () =>
    run(async () => {
      const b = await api.bedrock.backup('release')
      toast(b ? t('bedrock.backupDone', { count: b.worlds.length, folder: b.path }) : t('bedrock.backupNone'), b ? 'success' : 'info')
    })

  const restore = (b: BedrockBackup) =>
    run(async () => {
      const r = await api.bedrock.restore(b.id)
      toast(
        r.restored === 0 ? t('bedrock.restoredNone') : r.kept > 0 ? t('bedrock.restoredKept', { count: r.restored, kept: r.kept }) : t('bedrock.restored', { count: r.restored }),
        r.restored === 0 ? 'info' : 'success'
      )
    })

  const openBackup = (b: BedrockBackup) => void api.bedrock.openBackup(b.id).catch((e) => toast((e as Error).message, 'error'))

  // A world from a newer version may not open in an older one: said before restoring into an older install.
  const installedOf = (b: BedrockBackup) => status?.editions.find((e) => e.edition === b.edition)?.version
  const newerThanInstalled = (b: BedrockBackup) => {
    const installed = installedOf(b)
    return !!b.version && !!installed && compareVersions(b.version, installed) > 0
  }

  const modals = (
    <>
      {picking && release && status && (
        <SwitchModal
          release={release}
          status={status}
          onClose={() => setPicking(false)}
          onSwitch={(version) => {
            setPicking(false)
            void switchTo(version)
          }}
        />
      )}

      {goingBack && (
        <Modal
          title={t('bedrock.backToStoreTitle')}
          onClose={() => setGoingBack(false)}
          footer={
            <>
              <button className="btn ghost" onClick={() => setGoingBack(false)}>
                {t('common.cancel')}
              </button>
              <button
                className="btn primary"
                onClick={() => {
                  setGoingBack(false)
                  void backToStore()
                }}
              >
                <Undo2 /> {t('bedrock.backToStore')}
              </button>
            </>
          }
        >
          <div className="muted">{t('bedrock.backToStoreBody')}</div>
        </Modal>
      )}

      {restoring && (
        <Modal
          title={t('bedrock.restoreTitle')}
          onClose={() => setRestoring(undefined)}
          footer={
            <>
              <button className="btn ghost" onClick={() => setRestoring(undefined)}>
                {t('common.cancel')}
              </button>
              <button
                className="btn primary"
                onClick={() => {
                  const b = restoring
                  setRestoring(undefined)
                  void restore(b)
                }}
              >
                <ArchiveRestore /> {t('bedrock.restore')}
              </button>
            </>
          }
        >
          <div className="muted">{t('bedrock.restoreBody', { worlds: restoring.worlds.join(', ') })}</div>
          {newerThanInstalled(restoring) && (
            <div className="callout" style={{ marginTop: 12 }}>
              <AlertTriangle />
              <span>{t('bedrock.restoreNewer', { backup: restoring.version ?? '?', installed: installedOf(restoring) ?? '?' })}</span>
            </div>
          )}
        </Modal>
      )}
    </>
  )

  return {
    busy: !!task,
    previewBusy: !!previewTask,
    backups,
    allBackups,
    showAllBackups: () => setAllBackups(true),
    pickVersion: () => setPicking(true),
    goBackToStore: () => setGoingBack(true),
    backUpNow: () => void backUpNow(),
    askRestore: setRestoring,
    openBackup,
    modals
  }
}

export type BedrockVersionsState = ReturnType<typeof useBedrockVersions>

/** On the cover: the version Minecraft for Windows is on, where it came from, and the way to another one. */
export function VersionPlate({ v, release }: { v: BedrockVersionsState; release: BedrockEdition }) {
  return (
    <div className="bd-version">
      <div className="bd-version-text">
        <b>{t('bedrock.versionCurrent', { version: release.version ?? '?' })}</b>
        <span>{t(!release.switched ? 'bedrock.versionFromStore' : release.format === 'uwp' ? 'bedrock.versionSwitchedUwp' : 'bedrock.versionSwitchedGdk')}</span>
      </div>
      <div className="bd-version-acts">
        {release.switched && (
          <button className="btn sm" disabled={v.busy} onClick={v.goBackToStore}>
            <Undo2 /> {t('bedrock.backToStore')}
          </button>
        )}
        <button className="btn sm" disabled={v.busy} onClick={v.pickVersion}>
          <History /> {t('bedrock.switchVersion')}
        </button>
      </div>
    </div>
  )
}

/** Beside Preview: how many backups there are, the newest one's age, and Back up now. */
export function BackupsCard({ v, release }: { v: BedrockVersionsState; release: BedrockEdition }) {
  const { backups } = v
  const newest = backups?.[0]
  return (
    <div className="card bd-mini">
      <span className="bd-mini-icon">
        <Archive />
      </span>
      <div className="grow" style={{ minWidth: 0 }}>
        <b className="ellipsis">{t('bedrock.backups')}</b>
        <div className="hint">
          {!backups
            ? t('common.loading')
            : newest
              ? t('bedrock.backupWhen', { when: timeAgo(new Date(newest.created).toISOString(), true) })
              : t('bedrock.backupsHint')}
        </div>
      </div>
      <button className="btn" disabled={v.busy || !release.installed || !release.worlds.length} onClick={v.backUpNow}>
        <Archive /> {t('bedrock.backupNow')}
      </button>
    </div>
  )
}

/** The backups, newest first, each with Restore and its folder. */
export function BackupsList({ v }: { v: BedrockVersionsState }) {
  const { backups } = v
  if (!backups?.length) return null
  const shown = v.allBackups ? backups : backups.slice(0, BACKUPS_SHOWN)
  return (
    <>
      <p className="bd-section-hint">{t('bedrock.backupsHint')}</p>
      <div className="card list">
        {shown.map((b) => (
          <div key={b.id} className="list-row">
            <Archive size={18} className="faint" />
            <div className="grow" style={{ minWidth: 0 }}>
              <div className="title ellipsis" title={b.worlds.join(', ')}>
                {t('bedrock.backupWhen', { when: timeAgo(new Date(b.created).toISOString(), true) })}
                {b.edition === 'preview' ? ` · ${t('bedrock.preview')}` : b.version ? ` · ${t('bedrock.backupVersion', { version: b.version })}` : ''}
              </div>
              <div className="file ellipsis">
                {t('bedrock.backupDetail', { count: b.worlds.length, size: size(b.bytes) })} · {b.worlds.join(', ')}
              </div>
            </div>
            <button className="btn sm" disabled={v.busy || (b.edition === 'preview' && v.previewBusy)} onClick={() => v.askRestore(b)}>
              <ArchiveRestore /> {t('bedrock.restore')}
            </button>
            <button className="btn ghost icon sm" title={t('bedrock.openBackup')} aria-label={t('bedrock.openBackup')} onClick={() => v.openBackup(b)}>
              <FolderOpen />
            </button>
          </div>
        ))}
        {backups.length > BACKUPS_SHOWN && !v.allBackups && (
          <div className="list-row">
            <span className="grow" />
            <button className="btn sm ghost" onClick={v.showAllBackups}>
              {t('bedrock.showAllBackups', { count: backups.length })}
            </button>
          </div>
        )}
      </div>
    </>
  )
}

type Filter = 'all' | 'gdk' | 'uwp'

/**
 * Picking a version, then agreeing to the switch. The second step shows the engine's plan for this PC: what is
 * backed up, downloaded and replaced, what the version means for the worlds, and anything that has to be put right
 * first (Developer Mode is the player's to turn on: the button only opens Windows' settings page).
 */
function SwitchModal({ release, status, onClose, onSwitch }: { release: BedrockEdition; status: BedrockStatus; onClose: () => void; onSwitch: (version: string) => void }) {
  const [versions, setVersions] = useState<BedrockVersion[]>()
  const [error, setError] = useState<string>()
  const [query, setQuery] = useState('')
  const [filter, setFilter] = useState<Filter>('all')
  const [picked, setPicked] = useState<BedrockVersion>()
  const [plan, setPlan] = useState<BedrockSwitchPlan>()
  const [planError, setPlanError] = useState<string>()
  const [opening, setOpening] = useState(false)

  useEffect(() => {
    void api.bedrock
      .versions()
      .then(setVersions)
      .catch((e) => setError((e as Error).message))
  }, [])

  const check = useCallback((version: string) => {
    setPlan(undefined)
    setPlanError(undefined)
    void api.bedrock
      .switchPlan(version)
      .then(setPlan)
      .catch((e) => setPlanError((e as Error).message))
  }, [])

  const pick = (v: BedrockVersion) => {
    setPicked(v)
    check(v.version)
  }

  const list = useMemo(() => {
    const q = query.trim()
    return (versions ?? []).filter((v) => (filter === 'all' || v.format === filter) && (!q || v.version.includes(q)))
  }, [versions, query, filter])

  if (picked) {
    const blocked = !plan || plan.blockers.length > 0
    return (
      <Modal
        wide
        title={t('bedrock.switch.confirmTitle', { version: picked.version })}
        subtitle={release.version ? t('bedrock.switch.confirmFrom', { version: release.version }) : undefined}
        onClose={onClose}
        footer={
          <>
            <button className="btn ghost" onClick={() => setPicked(undefined)}>
              {t('bedrock.switch.otherVersion')}
            </button>
            <span className="grow" />
            <button className="btn ghost" onClick={onClose}>
              {t('common.cancel')}
            </button>
            <button className="btn primary" disabled={blocked} onClick={() => onSwitch(picked.version)}>
              {t('bedrock.switch.confirm')}
            </button>
          </>
        }
      >
        {planError ? (
          <div className="callout danger">
            <AlertTriangle />
            <span className="grow">{t('bedrock.switch.planFailed', { error: planError })}</span>
            <button className="btn sm" onClick={() => check(picked.version)}>
              <RefreshCw /> {t('bedrock.switch.recheck')}
            </button>
          </div>
        ) : !plan ? (
          <div className="hint row" style={{ gap: 8 }}>
            <Loader2 className="spin" /> {t('bedrock.switch.checking')}
          </div>
        ) : (
          <>
            {plan.blockers.map((b) => (
              <div key={b} className="callout danger" style={{ marginBottom: 10, alignItems: 'flex-start' }}>
                <AlertTriangle />
                <div className="grow" style={{ minWidth: 0 }}>
                  <div>{b === 'diskSpace' ? t(BLOCKER_TEXT[b], { needed: size(plan.neededBytes), free: size(plan.freeBytes ?? 0) }) : t(BLOCKER_TEXT[b])}</div>
                  {(b === 'developerMode' || b === 'gamingServices') && (
                    <div className="row wrap" style={{ marginTop: 8 }}>
                      {b === 'developerMode' ? (
                        <button
                          className="btn sm"
                          onClick={() => {
                            setOpening(true)
                            void api.bedrock
                              .developerSettings()
                              .catch(() => {})
                              .finally(() => setOpening(false))
                          }}
                        >
                          {opening ? <Loader2 className="spin" /> : <ExternalLink />} {t('bedrock.switch.openDeveloperSettings')}
                        </button>
                      ) : (
                        <button className="btn sm" onClick={() => void api.bedrock.gamingServices().catch(() => {})}>
                          <ExternalLink /> {t('bedrock.getGamingServices')}
                        </button>
                      )}
                      <button className="btn sm ghost" onClick={() => check(picked.version)}>
                        <RefreshCw /> {t('bedrock.switch.recheck')}
                      </button>
                    </div>
                  )}
                </div>
              </div>
            ))}
            <ol className="muted" style={{ margin: 0, paddingInlineStart: 20, display: 'grid', gap: 8 }}>
              <li>{plan.worlds > 0 ? t('bedrock.switch.stepBackup', { count: plan.worlds, size: size(plan.worldsBytes) }) : t('bedrock.switch.stepBackupNone')}</li>
              <li>
                {plan.size ? t('bedrock.switch.stepDownload', { version: plan.version, size: size(plan.size) }) : t('bedrock.switch.stepDownloadUnknown', { version: plan.version })}
              </li>
              <li>{t('bedrock.switch.stepReplace', { version: plan.version })}</li>
            </ol>
            <div className="callout" style={{ marginTop: 14, display: 'grid', gap: 8 }}>
              {plan.worlds > 0 && <div>{t(plan.sharesWorlds ? 'bedrock.switch.worldsShared' : 'bedrock.switch.worldsOwn')}</div>}
              {plan.format === 'gdk' && <div>{t('bedrock.switch.storeUpdates')}</div>}
              <div>{t('bedrock.switch.license')}</div>
              <div>{t('bedrock.switch.unsupported')}</div>
            </div>
          </>
        )}
      </Modal>
    )
  }

  return (
    <Modal wide title={t('bedrock.switch.title')} subtitle={t('bedrock.switch.subtitle')} onClose={onClose}>
      <div className="row wrap" style={{ marginBottom: 12 }}>
        <div className="search grow" style={{ minWidth: 180 }}>
          <Search />
          <input className="input" autoFocus placeholder={t('bedrock.switch.search')} aria-label={t('bedrock.switch.search')} value={query} onChange={(e) => setQuery(e.target.value)} />
        </div>
        <Segmented<Filter>
          value={filter}
          onChange={setFilter}
          options={[
            { value: 'all', label: t('bedrock.switch.filterAll') },
            { value: 'gdk', label: t('bedrock.switch.filterGdk') },
            { value: 'uwp', label: t('bedrock.switch.filterUwp') }
          ]}
        />
      </div>
      {error ? (
        <div className="callout danger">
          <AlertTriangle />
          <span>{t('bedrock.switch.loadFailed', { error })}</span>
        </div>
      ) : !versions ? (
        <div className="hint row" style={{ gap: 8 }}>
          <Loader2 className="spin" /> {t('bedrock.switch.loading')}
        </div>
      ) : (
        <div className="card list" style={{ maxHeight: 'min(52vh, 420px)', overflowY: 'auto' }}>
          {list.map((v) => {
            const installed = release.version === v.version
            return (
              <div key={v.version} className="list-row">
                <div className="grow" style={{ minWidth: 0 }}>
                  <div className="title">{v.version}</div>
                  {v.size ? <div className="file">{size(v.size)}</div> : null}
                </div>
                {v.format === 'uwp' && status.developerMode === false && <span className="chip warn">{t('bedrock.switch.needsDeveloperMode')}</span>}
                {v.format === 'gdk' && status.gamingServices === false && <span className="chip warn">{t('bedrock.switch.needsGamingServices')}</span>}
                {installed ? (
                  <span className="chip accent">{t('bedrock.switch.installedChip')}</span>
                ) : (
                  <button className="btn sm" onClick={() => pick(v)}>
                    {t('bedrock.switch.choose')}
                  </button>
                )}
              </div>
            )
          })}
          {!list.length && (
            <div className="list-row">
              <span className="hint">{t('bedrock.switch.none')}</span>
            </div>
          )}
        </div>
      )}
    </Modal>
  )
}
