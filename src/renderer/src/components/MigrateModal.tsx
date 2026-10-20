import { useEffect, useMemo, useRef, useState } from 'react'
import { AlertTriangle, ArrowLeftRight, Check, FolderOpen, Loader2, Server, Trash2, X } from 'lucide-react'
import type { MigrateItem, MigrateOutcome, MigrateRef, MigrateSource } from '@shared/migrate'
import { api } from '../api'
import { BlockArt, Empty, Modal } from './ui'
import { SERVER_LABEL } from './NewServerModal'
import { useStore } from '../store'
import { bytes, fixed, LOADER_LABEL } from '../util'
import { engineError, t, tx } from '../i18n'
import '../migrate.css'

const keyOf = (r: MigrateRef) => [r.launcher, r.server ? 's' : 'i', r.path, r.profile ?? ''].join('|')
const importable = (i: MigrateItem) => !i.problem
const PLUGIN_SERVERS = new Set(['paper', 'purpur', 'folia', 'spigot', 'leaf'])

/** Sizes up to many GB (instances with big worlds). */
const size = (n: number) => (n >= 1 << 30 ? t('common.size.gb', { n: fixed(n / (1 << 30), 1) }) : bytes(n))

/** "1.20.1 · Fabric 0.15.0" (or the server software). */
function versionLine(i: MigrateItem): string {
  const parts = [i.mcVersion ?? t('migrate.unknownVersion')]
  if (i.server && i.serverType) parts.push(SERVER_LABEL[i.serverType] + (i.loaderVersion ? ` ${i.loaderVersion}` : ''))
  else if (i.loader !== 'vanilla') parts.push(LOADER_LABEL[i.loader] + (i.loaderVersion ? ` ${i.loaderVersion}` : ''))
  return parts.join(' · ')
}

/**
 * "Move from another launcher": lists what other launchers on this PC have (or what's in a folder the player
 * picks), then copies the chosen instances and servers into ResinCraft. `servers` shows only server folders.
 */
export default function MigrateModal({ servers, onClose, onImported }: { servers?: boolean; onClose: () => void; onImported?: () => void }) {
  const { toast, go } = useStore()
  const [sources, setSources] = useState<MigrateSource[]>()
  const [picked, setPicked] = useState<Set<string>>(new Set())
  const [browsing, setBrowsing] = useState(false)
  const [busy, setBusy] = useState(false)
  const [outcomes, setOutcomes] = useState<MigrateOutcome[]>()
  /** "Remove the originals after moving" (off until ticked), and its confirm step. */
  const [removeOriginals, setRemoveOriginals] = useState(false)
  const [confirming, setConfirming] = useState(false)
  /** Bytes per item, filled in after the list shows (walking big instances takes a while). */
  const [sizes, setSizes] = useState<Map<string, number>>(new Map())
  const asked = useRef(new Set<string>())
  const open = useRef(true)
  useEffect(() => {
    open.current = true
    return () => {
      open.current = false
    }
  }, [])

  const only = (list: MigrateSource[]) => list.map((s) => ({ ...s, items: s.items.filter((i) => !servers || i.server) })).filter((s) => s.items.length > 0)

  /** New items start ticked unless they can't be imported or were moved before. */
  const tick = (list: MigrateSource[]) =>
    setPicked((p) => {
      const next = new Set(p)
      for (const s of list) for (const i of s.items) if (importable(i) && !i.imported) next.add(keyOf(i))
      return next
    })

  useEffect(() => {
    let live = true
    api.migrate
      .scan()
      .then((list) => {
        if (!live) return
        const shown = only(list)
        setSources(shown)
        tick(shown)
      })
      .catch((e) => {
        toast(engineError(e).message, 'error')
        if (live) setSources([])
      })
    return () => {
      live = false
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const chooseFolder = async () => {
    setBrowsing(true)
    try {
      const found = await api.migrate.scanFolder()
      if (!found) return
      const [shown] = only([found])
      if (!shown) {
        toast(t('migrate.noServerThere'), 'error')
        return
      }
      setSources((cur) => [shown, ...(cur ?? []).filter((s) => !(s.launcher === shown.launcher && s.path === shown.path))])
      tick([shown])
    } catch (e) {
      toast(engineError(e).message, 'error')
    } finally {
      setBrowsing(false)
    }
  }

  const all = useMemo(() => (sources ?? []).flatMap((s) => s.items), [sources])
  const chosen = all.filter((i) => picked.has(keyOf(i)) && importable(i))

  const labelOf = useMemo(() => new Map((sources ?? []).flatMap((s) => s.items.map((i) => [keyOf(i), s.label] as const))), [sources])
  const removable = chosen.filter((i) => !i.removeBlocked)
  const removeFrom = [...new Set(removable.map((i) => labelOf.get(keyOf(i)) ?? ''))]

  // Measures new items three at a time.
  useEffect(() => {
    const queue = all.filter((i) => importable(i) && !asked.current.has(keyOf(i)))
    queue.forEach((i) => asked.current.add(keyOf(i)))
    const next = async (): Promise<void> => {
      const i = queue.shift()
      if (!i || !open.current) return
      const n = await api.migrate.size(i).catch(() => undefined)
      if (n !== undefined && open.current) setSizes((m) => new Map(m).set(keyOf(i), n))
      return next()
    }
    for (let k = 0; k < 3; k++) void next()
  }, [all])

  const toggle = (i: MigrateItem) =>
    setPicked((p) => {
      const next = new Set(p)
      if (next.has(keyOf(i))) next.delete(keyOf(i))
      else next.add(keyOf(i))
      return next
    })

  const toggleSource = (s: MigrateSource, on: boolean) =>
    setPicked((p) => {
      const next = new Set(p)
      for (const i of s.items.filter(importable)) {
        if (on) next.add(keyOf(i))
        else next.delete(keyOf(i))
      }
      return next
    })

  const run = async () => {
    const remove = removeOriginals && removable.length > 0
    if (remove && !confirming) {
      setConfirming(true)
      return
    }
    setConfirming(false)
    setBusy(true)
    try {
      const refs: MigrateRef[] = chosen.map(({ launcher, root, path, profile, server }) => ({ launcher, root, path, profile, server }))
      const out = await api.migrate.import(refs, remove)
      setOutcomes(out)
      const ok = out.filter((o) => o.id).length
      if (ok) toast(t('migrate.done.toast', { count: ok }))
      onImported?.()
    } catch (e) {
      const err = engineError(e)
      if (err.key === 'task.cancelled') onImported?.()
      else toast(err.message, 'error')
    } finally {
      setBusy(false)
    }
  }

  const title = servers ? t('migrate.serversTitle') : t('migrate.title')

  if (outcomes) {
    return (
      <Modal
        title={t('migrate.done.title')}
        onClose={onClose}
        wide
        footer={
          <button className="btn primary" onClick={onClose}>
            {t('common.done')}
          </button>
        }
      >
        <ul className="mg-results">
          {outcomes.map((o, n) => (
            <li key={n} className={o.id ? 'ok' : 'failed'}>
              {o.id ? <Check /> : <X />}
              <div className="grow" style={{ minWidth: 0 }}>
                <div className="row" style={{ gap: 8 }}>
                  <b className="ellipsis">{o.name}</b>
                  {o.server && <span className="chip">{t('migrate.server')}</span>}
                </div>
                {o.error && <div className="hint">{tx(o.error)}</div>}
                {o.removed && (
                  <div className="mg-sub">
                    <Trash2 /> {t('migrate.remove.removed')}
                  </div>
                )}
                {o.kept && (
                  <div className="mg-sub warn">
                    <AlertTriangle /> {t('migrate.remove.kept', { reason: tx(o.kept) })}
                  </div>
                )}
                {o.skipped.length > 0 && (
                  <details className="mg-skipped">
                    <summary>{t('migrate.done.skipped', { count: o.skipped.length })}</summary>
                    <ul>
                      {o.skipped.slice(0, 200).map((s) => (
                        <li key={s.path}>
                          <span className="mono">{s.path || '.'}</span> <span className="hint">{tx(s.reason)}</span>
                        </li>
                      ))}
                    </ul>
                  </details>
                )}
              </div>
              {o.id && (
                <button
                  className="btn sm"
                  onClick={() => {
                    onClose()
                    go(o.server ? { page: 'servers', id: o.id } : { page: 'instance', id: o.id! })
                  }}
                >
                  {t('menu.open')}
                </button>
              )}
            </li>
          ))}
        </ul>
      </Modal>
    )
  }

  if (confirming) {
    const staying = chosen.filter((i) => i.removeBlocked)
    return (
      <Modal
        title={t('migrate.remove.confirmTitle')}
        onClose={() => setConfirming(false)}
        wide
        footer={
          <>
            <button className="btn ghost" onClick={() => setConfirming(false)}>
              {t('common.back')}
            </button>
            <button className="btn primary" onClick={() => void run()}>
              <Trash2 /> {t('migrate.remove.confirm')}
            </button>
          </>
        }
      >
        <p className="hint">{t('migrate.remove.confirmBody', { count: removable.length })}</p>
        <ul className="mg-confirm">
          {removable.map((i) => (
            <li key={keyOf(i)}>
              <b>{i.name}</b> <span className="hint">{labelOf.get(keyOf(i))}</span>
              <div className="mono hint ellipsis" title={i.path}>
                {i.path}
              </div>
            </li>
          ))}
        </ul>
        {staying.length > 0 && (
          <>
            <p className="hint">{t('migrate.remove.confirmKept')}</p>
            <ul className="mg-confirm">
              {staying.map((i) => (
                <li key={keyOf(i)}>
                  <b>{i.name}</b> <span className="hint">{tx(i.removeBlocked)}</span>
                </li>
              ))}
            </ul>
          </>
        )}
      </Modal>
    )
  }

  return (
    <Modal
      title={title}
      subtitle={servers ? t('migrate.serversSubtitle') : t('migrate.subtitle')}
      onClose={onClose}
      wide
      footer={
        <>
          <button className="btn" onClick={() => void chooseFolder()} disabled={browsing || busy}>
            {browsing ? <Loader2 className="spin" /> : <FolderOpen />} {t('migrate.chooseFolder')}
          </button>
          <div className="spacer" />
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={busy || chosen.length === 0} onClick={() => void run()}>
            {busy ? <Loader2 className="spin" /> : <ArrowLeftRight />} {t('migrate.import', { count: chosen.length })}
          </button>
        </>
      }
    >
      {!sources ? (
        <div className="mg-scanning">
          <Loader2 className="spin" /> {t('migrate.scanning')}
        </div>
      ) : sources.length === 0 ? (
        <Empty
          icon={servers ? <Server /> : <ArrowLeftRight />}
          title={servers ? t('migrate.none.serversTitle') : t('migrate.none.title')}
          action={
            <button className="btn primary" onClick={() => void chooseFolder()} disabled={browsing}>
              {browsing ? <Loader2 className="spin" /> : <FolderOpen />} {t('migrate.chooseFolder')}
            </button>
          }
        >
          {servers ? t('migrate.none.serversBody') : t('migrate.none.body')}
        </Empty>
      ) : (
        <div className="mg-sources" aria-busy={busy}>
          {busy && <p className="hint">{t('migrate.importing')}</p>}
          {sources.map((s) => {
            const usable = s.items.filter(importable)
            const on = usable.filter((i) => picked.has(keyOf(i))).length
            return (
              <section key={s.launcher + s.path} className="mg-source">
                <label className="mg-head">
                  <input
                    type="checkbox"
                    disabled={busy || usable.length === 0}
                    checked={usable.length > 0 && on === usable.length}
                    ref={(el) => {
                      if (el) el.indeterminate = on > 0 && on < usable.length
                    }}
                    onChange={(e) => toggleSource(s, e.target.checked)}
                    aria-label={t('migrate.selectAll', { name: s.label })}
                  />
                  <b>{s.launcher === 'folder' ? t('migrate.folder') : s.label}</b>
                  <span className="mono hint ellipsis grow" title={s.path}>
                    {s.path}
                  </span>
                  <span className="hint">{t('migrate.found', { count: s.items.length })}</span>
                </label>
                {s.items.map((i) => (
                  <label key={keyOf(i)} className={`mg-item ${picked.has(keyOf(i)) && importable(i) ? 'on' : ''} ${i.problem ? 'off' : ''}`}>
                    <input type="checkbox" disabled={busy || !importable(i)} checked={picked.has(keyOf(i)) && importable(i)} onChange={() => toggle(i)} />
                    <div className="mg-icon">
                      {i.iconUrl ? <img src={i.iconUrl} alt="" /> : <BlockArt seed={i.path + (i.profile ?? '')} color={i.server ? '#22d3ee' : '#b26bff'} />}
                    </div>
                    <div className="grow" style={{ minWidth: 0 }}>
                      <div className="row" style={{ gap: 8 }}>
                        <span className="name ellipsis">{i.name}</span>
                        {i.server && <span className="chip">{t('migrate.server')}</span>}
                        {i.imported && <span className="chip success">{t('migrate.imported')}</span>}
                      </div>
                      {removeOriginals && i.removeBlocked && picked.has(keyOf(i)) && !i.problem && (
                        <div className="mg-sub">{t('migrate.remove.stays', { reason: tx(i.removeBlocked) })}</div>
                      )}
                      {i.problem ? (
                        <div className="mg-sub warn">
                          <AlertTriangle /> {tx(i.problem)}
                        </div>
                      ) : (
                        <div className="mg-sub">
                          <span className="data">{versionLine(i)}</span>
                          {i.mods > 0 && (
                            <span>{t(i.serverType && PLUGIN_SERVERS.has(i.serverType) ? 'migrate.plugins' : 'migrate.mods', { count: i.mods })}</span>
                          )}
                          {i.worlds > 0 && <span>{t('migrate.worlds', { count: i.worlds })}</span>}
                        </div>
                      )}
                    </div>
                    {!i.problem &&
                      (sizes.has(keyOf(i)) ? (
                        <span className="mg-size data">{size(sizes.get(keyOf(i))!)}</span>
                      ) : (
                        <span className="mg-size measuring" aria-label={t('common.loading')} />
                      ))}
                  </label>
                ))}
              </section>
            )
          })}
          {!servers && (
            <label className={`mg-remove ${removeOriginals ? 'on' : ''}`}>
              <input
                type="checkbox"
                checked={removeOriginals}
                disabled={busy || removable.length === 0}
                onChange={(e) => setRemoveOriginals(e.target.checked)}
              />
              <span className="grow">
                {removeFrom.length ? t('migrate.remove.label', { launchers: removeFrom.join(', ') }) : t('migrate.remove.labelGeneric')}
                <span className="hint">{t('migrate.remove.hint')}</span>
              </span>
            </label>
          )}
          <p className="hint">{t('migrate.tip')}</p>
        </div>
      )}
    </Modal>
  )
}
