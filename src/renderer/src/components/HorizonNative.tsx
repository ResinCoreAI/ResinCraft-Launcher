import { useCallback, useEffect, useState } from 'react'
import { AlertTriangle, Code, Hammer, Loader2, Play, Trash2 } from 'lucide-react'
import type { Instance, NativeCacheInfo, NativeStatus } from '@shared/types'
import { api } from '../api'
import { engineError, t } from '../i18n'
import { useStore } from '../store'
import { Modal } from './ui'

/**
 * Horizon native mods: code compiled into the game by Horizon's own launcher before the instance plays. They run
 * unsandboxed, with the player's rights, so nothing is compiled before the player says they trust them, in red,
 * before every build (as Horizon's launcher does).
 */

const gib = (bytes: number) => (bytes / 1073741824).toFixed(bytes >= 10 * 1073741824 ? 0 : 1)

/** Answers to the build dialog a start opens when the native mods aren't built. */
export type NativeAnswer = 'built' | 'skip' | 'cancel'

type Ask = { instanceId: string; resolve: (a: NativeAnswer) => void }
let host: ((ask: Ask) => void) | null = null

/** Opens the build dialog for an instance (mounted once, in App) and waits for the player's answer. */
export function askNativeBuild(instanceId: string): Promise<NativeAnswer> {
  return new Promise((resolve) => (host ? host({ instanceId, resolve }) : resolve('cancel')))
}

export function NativeBuildHost() {
  const [ask, setAsk] = useState<Ask>()
  useEffect(() => {
    host = (a) => setAsk(a)
    return () => {
      host = null
    }
  }, [])
  if (!ask) return null
  const finish = (a: NativeAnswer) => {
    ask.resolve(a)
    setAsk(undefined)
  }
  return <NativeBuildDialog instanceId={ask.instanceId} onDone={finish} offerSkip />
}

/** The red warning every build waits behind: who made what, and that it runs with full access. */
function TrustWarning({ status }: { status: NativeStatus }) {
  return (
    <div className="callout danger" role="alert">
      <AlertTriangle />
      <div>
        <b>{t('native.trust.title')}</b>
        <p style={{ margin: '4px 0' }}>{t('native.trust.body')}</p>
        <ul style={{ margin: '4px 0 0', paddingLeft: 18 }}>
          {status.mods.map((m) => (
            <li key={m.id}>
              <code>{m.id}</code> {m.version}
            </li>
          ))}
        </ul>
      </div>
    </div>
  )
}

/**
 * Builds an instance's native mods: the warning, then the build with its log, then what happened. With `offerSkip`
 * (a start that found them unbuilt), the player can play once without them.
 */
export function NativeBuildDialog({ instanceId, onDone, offerSkip }: { instanceId: string; onDone: (a: NativeAnswer) => void; offerSkip?: boolean }) {
  const { instances } = useStore()
  const inst = instances.find((i) => i.id === instanceId)
  const [status, setStatus] = useState<NativeStatus>()
  const [phase, setPhase] = useState<'ask' | 'building' | 'failed' | 'done'>('ask')
  const [error, setError] = useState('')
  const [log, setLog] = useState<string[]>([])

  useEffect(() => {
    void api.horizon.nativeStatus(instanceId).then(setStatus, (e) => (setError(engineError(e).message), setPhase('failed')))
  }, [instanceId])

  // The log's last lines while the build runs (read again every two seconds), and once it ends.
  useEffect(() => {
    if (phase !== 'building' && phase !== 'failed') return
    const read = () => void api.horizon.nativeLog(14).then(setLog, () => {})
    read()
    if (phase !== 'building') return
    const timer = window.setInterval(read, 2000)
    return () => window.clearInterval(timer)
  }, [phase])

  const build = async () => {
    setPhase('building')
    try {
      await api.horizon.nativeBuild(instanceId, true)
      setPhase('done')
      onDone('built')
    } catch (e) {
      setError(engineError(e).message)
      setPhase('failed')
    }
  }

  const close = () => (phase === 'building' ? undefined : onDone('cancel'))
  const name = inst?.name ?? instanceId
  const why =
    status?.need === 'modsChanged'
      ? t('native.need.modsChanged')
      : status?.need === 'baseChanged'
        ? t('native.need.baseChanged')
        : t('native.need.firstBuild')

  return (
    <Modal
      title={t('native.build.title', { name })}
      subtitle={phase === 'ask' ? why : undefined}
      onClose={close}
      wide
      footer={
        phase === 'ask' ? (
          <>
            <button className="btn ghost" onClick={() => onDone('cancel')}>
              {t('common.cancel')}
            </button>
            {offerSkip && (
              <button className="btn" onClick={() => onDone('skip')}>
                <Play /> {t('native.playWithout')}
              </button>
            )}
            <button className="btn danger solid" disabled={!status || status.building} onClick={() => void build()}>
              <Hammer /> {t('native.trust.build')}
            </button>
          </>
        ) : phase === 'failed' ? (
          <>
            <button className="btn ghost" onClick={() => onDone('cancel')}>
              {t('common.close')}
            </button>
            {offerSkip && (
              <button className="btn primary" onClick={() => onDone('skip')}>
                <Play /> {t('native.playWithout')}
              </button>
            )}
          </>
        ) : undefined
      }
    >
      {phase === 'ask' && !status && <Loader2 className="spin" />}
      {phase === 'ask' && status && (
        <>
          <TrustWarning status={status} />
          {status.firstEver && <p className="hint">{t('native.firstBuildHint')}</p>}
          {status.building && <p className="hint">{t('native.busy')}</p>}
        </>
      )}
      {phase === 'building' && (
        <p className="row" style={{ gap: 8 }}>
          <Loader2 className="spin" /> {t('native.building')}
        </p>
      )}
      {phase === 'failed' && (
        <div className="callout danger">
          <AlertTriangle />
          <div style={{ whiteSpace: 'pre-wrap' }}>{error}</div>
        </div>
      )}
      {(phase === 'building' || phase === 'failed') && log.length > 0 && (
        <>
          <p className="hint" style={{ marginTop: 10 }}>
            {t('native.logTail')}
          </p>
          <pre className="mono" style={{ maxHeight: 260, overflow: 'auto', whiteSpace: 'pre-wrap', fontSize: 12, margin: 0 }}>
            {log.join('\n')}
          </pre>
        </>
      )}
    </Modal>
  )
}

/** Asked before a native mod is installed: what native code is. Nothing is compiled yet. */
export function NativeInstallNotice({ names, onAnswer }: { names: string[]; onAnswer: (ok: boolean) => void }) {
  return (
    <Modal
      title={t('native.install.title')}
      onClose={() => onAnswer(false)}
      footer={
        <>
          <button className="btn ghost" onClick={() => onAnswer(false)}>
            {t('common.cancel')}
          </button>
          <button className="btn danger solid" onClick={() => onAnswer(true)}>
            {t('native.install.confirm')}
          </button>
        </>
      }
    >
      <div className="callout danger" role="alert">
        <AlertTriangle />
        <div>
          <b>{names.join(', ')}</b>
          <p style={{ margin: '4px 0 0' }}>{t('native.install.body')}</p>
        </div>
      </div>
    </Modal>
  )
}

/** A Horizon instance's native build, above its mod list: up to date or not, and Build. */
export function NativeBuildBar({ inst, changed }: { inst: Instance; changed: unknown }) {
  const [status, setStatus] = useState<NativeStatus>()
  const [open, setOpen] = useState(false)
  const load = useCallback(() => void api.horizon.nativeStatus(inst.id).then(setStatus, () => {}), [inst.id])
  useEffect(load, [load, changed])
  if (!status || status.mods.length === 0) return null
  const fresh = status.need === 'upToDate'
  return (
    <div className={`callout ${fresh ? '' : 'pending'}`} style={{ marginBottom: 10, alignItems: 'center' }}>
      <Code />
      <div className="grow">
        <b>{fresh ? t('native.bar.upToDate') : t('native.bar.needsBuild')}</b>
        <div className="hint">{t('native.bar.hint')}</div>
      </div>
      <button className="btn sm" disabled={status.building} onClick={() => setOpen(true)}>
        <Hammer /> {fresh ? t('native.bar.rebuild') : t('native.bar.build')}
      </button>
      {open && <NativeBuildDialog instanceId={inst.id} onDone={() => (setOpen(false), load())} />}
    </div>
  )
}

/** Settings: the native compile cache's size, its limit, and Clear. */
export function NativeCacheSection() {
  const { toast } = useStore()
  const [info, setInfo] = useState<NativeCacheInfo>()
  const [limit, setLimit] = useState('')
  const [busy, setBusy] = useState(false)
  const show = (i: NativeCacheInfo) => (setInfo(i), setLimit(String(i.limitGib)))
  useEffect(() => void api.horizon.nativeCache().then(show, () => {}), [])

  const run = async (f: () => Promise<NativeCacheInfo>, done?: (i: NativeCacheInfo) => string) => {
    setBusy(true)
    try {
      const i = await f()
      show(i)
      if (done) toast(done(i))
    } catch (e) {
      toast(engineError(e).message, 'error')
    } finally {
      setBusy(false)
    }
  }
  const saveLimit = () => {
    const n = Math.round(Number(limit))
    if (!Number.isFinite(n) || n < 0 || n === info?.limitGib) return setLimit(String(info?.limitGib ?? 30))
    void run(() => api.horizon.nativeCacheLimit(n))
  }

  return (
    <section className="settings-section" id="native-mods">
      <h3>{t('native.settings.title')}</h3>
      <p>{t('native.settings.lead')}</p>
      <div className="card">
        <div className="setting">
          <div className="text">
            <b>{t('native.settings.size')}</b>
            <span className="ellipsis" title={info?.path ?? ''}>
              {!info
                ? t('common.loading')
                : !info.installed
                  ? t('native.settings.none')
                  : info.busy
                    ? t('native.busy')
                    : t('native.settings.sizeValue', { size: gib(info.bytes), path: info.path ?? '' })}
            </span>
          </div>
          <button
            className="btn danger"
            disabled={busy || !info?.installed || info.busy}
            onClick={() => void run(() => api.horizon.nativeCacheClear(), (i) => t('native.settings.cleared', { size: gib(i.clearedBytes) }))}
          >
            {busy ? <Loader2 className="spin" /> : <Trash2 />} {t('native.settings.clear')}
          </button>
        </div>
        <div className="setting">
          <div className="text">
            <b>{t('native.settings.limit')}</b>
            <span>{t('native.settings.limitHint')}</span>
          </div>
          <div className="row" style={{ width: 160 }}>
            <input
              className="input"
              type="number"
              min={0}
              max={10000}
              value={limit}
              disabled={busy || !info?.installed}
              onChange={(e) => setLimit(e.target.value)}
              onBlur={saveLimit}
              onKeyDown={(e) => e.key === 'Enter' && (e.target as HTMLInputElement).blur()}
            />
            <span className="hint">GiB</span>
          </div>
        </div>
      </div>
    </section>
  )
}
