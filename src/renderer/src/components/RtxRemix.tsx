import { useEffect, useRef, useState } from 'react'
import { CircleCheck, ExternalLink, Loader2, Sparkles, TriangleAlert } from 'lucide-react'
import type { Instance, RtxRemixCheck } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { bytes } from '../util'
import { t } from '../i18n'
import { Segmented, Switch, celebrate } from './ui'
import '../rtx-remix.css'

/** NVIDIA's oldest supported driver; the same as `DRIVER` in content/rtxremix.rs. */
const DRIVER = '616.92'

function Row({ ok, title, detail }: { ok: boolean; title: string; detail?: string }) {
  return (
    <div className="setting">
      <div className="text">
        <b className={ok ? 'rtx-ok' : 'rtx-warn'}>
          {ok ? <CircleCheck /> : <TriangleAlert />} {title}
        </b>
        {detail && <span>{detail}</span>}
      </div>
    </div>
  )
}

/**
 * New instance → RTX Remix: NVIDIA RTX Remix for Minecraft (path tracing, beta) in a new Fabric 1.21.1 instance, or
 * added to one the player already has. The setup keeps going in the task list if the dialog closes.
 */
export function RtxRemix({ onDone, onBusy }: { onDone: (inst: Instance) => void; onBusy: (busy: boolean) => void }) {
  const { instances, refreshInstances, toast } = useStore()
  const [check, setCheck] = useState<RtxRemixCheck>()
  const [target, setTarget] = useState<'new' | 'existing'>('new')
  const [name, setName] = useState('')
  const [existing, setExisting] = useState<string>()
  const [world, setWorld] = useState(true)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string>()
  // The dialog can close while the setup runs on (it's in the task list); then only the toast is left to show.
  const open = useRef(true)
  useEffect(() => {
    open.current = true
    return () => {
      open.current = false
    }
  }, [])

  useEffect(() => {
    let live = true
    api.content.rtxRemixCheck().then((c) => live && setCheck(c), (e) => live && setError((e as Error).message))
    return () => {
      live = false
    }
  }, [])

  const fits = instances.filter((i) => i.loader === 'fabric' && i.mcVersion === (check?.mcVersion ?? '1.21.1'))
  useEffect(() => {
    if (!existing && fits.length) setExisting(fits[0].id)
  }, [fits, existing])

  if (!check)
    return (
      <div className="row" style={{ justifyContent: 'center', padding: 24 }}>
        {error ? <div className="callout danger">{error}</div> : <Loader2 className="spin" />}
      </div>
    )

  const gpuOk = check.gpus.some((g) => g.rtx && g.driverOk)
  const canStart = check.windows && !!check.version && !busy && (target === 'new' || !!existing)

  const start = async (e: React.MouseEvent) => {
    const from = e.currentTarget
    setBusy(true)
    onBusy(true)
    setError(undefined)
    let made: Instance | undefined
    try {
      const inst = target === 'new' ? await api.content.rtxRemixCreate(name.trim()) : instances.find((i) => i.id === existing)!
      if (target === 'new') {
        made = inst
        await refreshInstances()
      }
      const done = await api.content.rtxRemixInstall(inst.id, world)
      await refreshInstances()
      toast(t('rtxRemix.ready', { name: done.name }))
      if (!open.current) return
      celebrate(from)
      onDone(done)
    } catch (err) {
      await refreshInstances()
      if (!open.current) return toast((err as Error).message, 'error')
      setError((err as Error).message)
      // A new instance stays even when a download fails: pick it here to try again.
      if (made) {
        setExisting(made.id)
        setTarget('existing')
      }
    } finally {
      if (open.current) {
        setBusy(false)
        onBusy(false)
      }
    }
  }

  return (
    <div className="rtx-remix">
      <p className="muted">{t('rtxRemix.intro')}</p>

      <div className="card">
        <Row ok={check.windows} title={check.windows ? t('rtxRemix.windows') : t('rtxRemix.notWindows')} />
        {check.gpus.length === 0 ? (
          <Row ok={false} title={t('rtxRemix.noNvidia')} detail={t('rtxRemix.needCard', { driver: DRIVER })} />
        ) : (
          check.gpus.map((g, i) => (
            <Row
              key={i}
              ok={g.rtx && g.driverOk}
              title={g.name}
              detail={
                !g.rtx
                  ? t('rtxRemix.oldCard')
                  : !g.driverOk
                    ? t('rtxRemix.oldDriver', { driver: g.driver, need: DRIVER })
                    : t('rtxRemix.driver', { driver: g.driver })
              }
            />
          ))
        )}
        {check.windows && !gpuOk && <p className="hint rtx-hint">{t('rtxRemix.anyway')}</p>}
      </div>

      <div className="card">
        <div className="setting">
          <div className="text">
            <b>
              <Sparkles /> RTX Remix for Minecraft {check.version ?? ''}
            </b>
            <span>{check.version ? t('rtxRemix.contents', { mc: check.mcVersion, size: bytes(check.downloadSize) }) : check.error}</span>
          </div>
          {check.releaseUrl && (
            <button className="btn sm ghost" onClick={() => void api.app.openExternal(check.releaseUrl!)} title={t('rtxRemix.releasePage')}>
              <ExternalLink />
            </button>
          )}
        </div>
        <div className="setting">
          <div className="text">
            <b>{t('rtxRemix.world')}</b>
            <span>{t('rtxRemix.worldHint', { size: bytes(check.worldSize) })}</span>
          </div>
          <div className="control">
            <Switch on={world} onChange={setWorld} label={t('rtxRemix.world')} disabled={busy} />
          </div>
        </div>
      </div>

      <div className="field">
        <Segmented<'new' | 'existing'>
          value={target}
          onChange={(v) => !busy && setTarget(v)}
          options={[
            { value: 'new', label: t('rtxRemix.target.new') },
            { value: 'existing', label: t('rtxRemix.target.existing') }
          ]}
        />
      </div>
      {target === 'new' ? (
        <div className="field">
          <label htmlFor="rtx-name">{t('newInstance.name')}</label>
          <input id="rtx-name" className="input" placeholder="RTX Remix" value={name} disabled={busy} onChange={(e) => setName(e.target.value)} />
        </div>
      ) : (
        <div className="field">
          <label htmlFor="rtx-inst">{t('rtxRemix.pick')}</label>
          {fits.length ? (
            <select id="rtx-inst" className="input" value={existing} disabled={busy} onChange={(e) => setExisting(e.target.value)}>
              {fits.map((i) => (
                <option key={i.id} value={i.id}>
                  {i.name}
                </option>
              ))}
            </select>
          ) : (
            <span className="hint">{t('rtxRemix.noneFit', { mc: check.mcVersion })}</span>
          )}
          <span className="hint">{t('rtxRemix.existingHint')}</span>
        </div>
      )}

      <p className="hint">{t('rtxRemix.beta')}</p>
      {busy && <div className="callout">{t('rtxRemix.working')}</div>}
      {error && <div className="callout danger">{error}</div>}

      <div className="row" style={{ justifyContent: 'flex-end' }}>
        <button className={`btn ${gpuOk ? 'primary' : ''}`} disabled={!canStart} onClick={(e) => void start(e)}>
          {busy ? <Loader2 className="spin" /> : <Sparkles />} {t(target === 'new' ? 'rtxRemix.create' : 'rtxRemix.add')}
        </button>
      </div>
    </div>
  )
}
