import { useEffect, useRef, useState } from 'react'
import { CircleCheck, Cpu, ExternalLink, FolderOpen, Layers, Loader2, Sparkles, TriangleAlert } from 'lucide-react'
import type { Instance, RtxLookState, RtxPackChoice, RtxPackSpec, RtxRemixCheck, RtxRemixState } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { bytes } from '../util'
import { t, type Key } from '../i18n'
import { Segmented, Switch, celebrate } from './ui'
import '../rtx-remix.css'
import { platform } from '../platform'

/** NVIDIA's oldest supported driver; the same as `DRIVER` in content/rtxremix.rs. */
const DRIVER = '616.92'
/** The one version NVIDIA's mod runs on; the same as `MC` in content/rtxremix.rs. */
const RTX_MC = '1.21.1'

const gb = (mb: number) => `${Math.round(mb / 1024)} GB`

/** The game's memory for a pack; the same rule as `tune` in content/rtxpack.rs. */
function memoryFor(spec: RtxPackSpec, pack: string): number {
  const want = pack === '128' ? 4096 : pack === '256' ? 5120 : pack === 'own' ? (Math.round(spec.ramMb / 1024) >= 32 ? 8192 : 6144) : 3072
  const cap = Math.max(2048, Math.floor(spec.ramMb / 2 / 512) * 512)
  return Math.max(2048, Math.min(want, cap))
}

/**
 * The game's memory for the RTX setup: Auto (sized for the PC and the pack) or the player's own amount, from 2 GB up
 * to all but 2 GB of the PC's RAM.
 */
function MemoryPick({
  spec,
  auto,
  value,
  onChange,
  disabled,
  maxGb
}: {
  spec: RtxPackSpec
  auto: number
  value?: number
  onChange: (mb?: number) => void
  disabled?: boolean
  /** RTX Remix: a big Java heap starves its runtime of memory, so it stops at 8 GB. */
  maxGb?: number
}) {
  const most = Math.min(maxGb ?? Infinity, Math.max(2, Math.floor((spec.ramMb - 2048) / 1024)))
  const sizes = Array.from({ length: most - 1 }, (_, i) => i + 2).filter((g) => g <= 8 || g % 2 === 0 || g === most)
  return (
    <div className="setting">
      <div className="text">
        <b>{t('rtxPack.memory')}</b>
        <span>{t(maxGb ? 'rtxPack.memoryHintRemix' : 'rtxPack.memoryHint', { ram: gb(spec.ramMb), max: `${maxGb ?? ''} GB` })}</span>
      </div>
      <div className="control">
        <select className="input" value={value ?? ''} disabled={disabled} onChange={(e) => onChange(e.target.value ? Number(e.target.value) : undefined)}>
          <option value="">{t('rtxPack.memoryAuto', { size: gb(auto) })}</option>
          {sizes.map((g) => (
            <option key={g} value={g * 1024}>
              {`${g} GB`}
            </option>
          ))}
        </select>
      </div>
    </div>
  )
}

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
  const [pack, setPack] = useState<RtxPackChoice>('auto')
  const [ownPack, setOwnPack] = useState<string>()
  const [tune, setTune] = useState(true)
  const [memoryMb, setMemoryMb] = useState<number>()
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
  const canStart = check.windows && !!check.version && !busy && (target === 'new' || !!existing) && (pack !== 'own' || !!ownPack)
  const rec = check.recommended
  const recPack = check.packs.find((p) => p.key === rec.pack)
  const picked = pack === 'auto' ? rec.pack : pack
  const pickOwn = async () => {
    const path = await api.content.rtxPackPickOwn()
    if (path) setOwnPack(path)
  }

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
      const done = await api.content.rtxRemixInstall(inst.id, { world, pack, ownPack, tune, memoryMb })
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


      <div className="card">
        <div className="setting">
          <div className="text">
            <b>
              <Layers /> {t('rtxPack.title')}
            </b>
            <span>{t('rtxPack.hint')}</span>
          </div>
        </div>
        <div className="setting">
          <div className="text">
            <b>
              <Cpu /> {t('rtxPack.thisPc')}
            </b>
            <span>
              {[
                check.spec.vramMb ? t('rtxPack.vram', { size: gb(check.spec.vramMb) }) : null,
                t('rtxPack.ram', { size: gb(check.spec.ramMb) }),
                t('rtxPack.cores', { count: check.spec.cores })
              ]
                .filter(Boolean)
                .join(' · ')}
            </span>
          </div>
        </div>
        <div className="setting stack">
          <div className="text">
            <b>{t('rtxPack.textures')}</b>
            <span>{t('rtxPack.texturesHint')}</span>
          </div>
          <select className="input" value={pack} disabled={busy} onChange={(e) => setPack(e.target.value as RtxPackChoice)}>
            <option value="auto">{t('rtxPack.auto', { pack: recPack ? `${recPack.res}x ${recPack.title}` : `${rec.pack}x` })}</option>
            {check.packs.map((p) => (
              <option key={p.key} value={p.key}>
                {`${p.res}x · ${p.title}${p.size ? ` (${bytes(p.size)})` : ''}`}
              </option>
            ))}
            <option value="own">{t('rtxPack.own')}</option>
            <option value="none">{t('rtxPack.none')}</option>
          </select>
          {pack === 'own' && (
            <div className="rtx-own">
              <button className="btn sm" disabled={busy} onClick={() => void pickOwn()}>
                <FolderOpen /> {t('rtxPack.pick')}
              </button>
              <span className="hint">{ownPack ? ownPack.split(/[\\/]/).pop() : t('rtxPack.ownHint')}</span>
            </div>
          )}
        </div>
        <div className="setting">
          <div className="text">
            <b>{t('rtxPack.tune')}</b>
            <span>
              {t('rtxPack.tuneHint', {
                render: rec.renderDistance,
                simulation: rec.simulationDistance,
                memory: gb(memoryMb ?? memoryFor(check.spec, picked))
              })}
            </span>
          </div>
          <div className="control">
            <Switch on={tune} onChange={setTune} label={t('rtxPack.tune')} disabled={busy} />
          </div>
        </div>
        <MemoryPick spec={check.spec} auto={memoryFor(check.spec, picked)} value={memoryMb} onChange={setMemoryMb} disabled={busy} maxGb={8} />
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

/**
 * Instance → Settings: RTX Remix on or off, on every instance (Windows). Off plays the instance as plain Fabric (the
 * renderer jar is disabled and its texture packs leave the pack list); nothing is removed. In a Fabric 1.21.1
 * instance without it, turning it on downloads and sets up RTX Remix with the RTX Pack sized for this PC. Other
 * versions show why it can't go there.
 */
export function InstanceRtxRemix({ inst }: { inst: Instance }) {
  const { toast, refreshInstances } = useStore()
  const [state, setState] = useState<RtxRemixState>()
  const [busy, setBusy] = useState(false)
  const fits = inst.loader === 'fabric' && inst.mcVersion === RTX_MC

  useEffect(() => {
    let live = true
    api.content.rtxRemixState(inst.id).then((s) => live && setState(s), () => undefined)
    return () => {
      live = false
    }
  }, [inst.id])

  // RTX Remix can't go here: the RTX look (a shader) instead, on any system.
  if (platform !== 'windows' || (!fits && state && !state.installed)) return <InstanceRtxLook inst={inst} />
  if (!state) return null

  const set = async (on: boolean) => {
    setBusy(true)
    try {
      if (on && !state.installed) {
        // Not there yet: download and set it up, sized for this PC.
        toast(t('rtxRemix.toggle.downloading', { name: inst.name }), 'info')
        const done = await api.content.rtxRemixInstall(inst.id, { world: true, pack: 'auto', tune: true })
        setState(await api.content.rtxRemixState(inst.id))
        await refreshInstances()
        toast(t('rtxRemix.ready', { name: done.name }))
      } else {
        setState(await api.content.rtxRemixSet(inst.id, on))
        toast(t(on ? 'rtxRemix.toggle.onDone' : 'rtxRemix.toggle.offDone', { name: inst.name }))
      }
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(false)
    }
  }

  const hint = !state.installed
    ? fits
      ? t('rtxRemix.toggle.installHint')
      : t('rtxRemix.toggle.wrongVersion', { version: RTX_MC })
    : t(state.on ? 'rtxRemix.toggle.onHint' : 'rtxRemix.toggle.offHint')

  return (
    <div className="settings-section">
      <h3>RTX Remix</h3>
      <div className="card">
        <div className="setting">
          <div className="text">
            <b>
              <Sparkles /> {t('rtxRemix.toggle.label')}
            </b>
            <span>{busy && !state.installed ? t('rtxRemix.toggle.downloading', { name: inst.name }) : hint}</span>
          </div>
          <div className="control rtx-toggle">
            {busy && <Loader2 className="spin" />}
            <Switch
              on={state.installed && state.on}
              onChange={(v) => void set(v)}
              label={t('rtxRemix.toggle.label')}
              disabled={busy || (!state.installed && !fits)}
            />
          </div>
        </div>
      </div>
    </div>
  )
}

/**
 * Instance → Settings, for instances RTX Remix can't run in: the RTX look. A traced-light shader (with Iris, Oculus or
 * the instance's OptiFine), a PBR texture pack from 64x to 256x (or the player's own 512x–4096x zip) and settings sized
 * for this PC, set up with one button and switched on and off after.
 */
export function InstanceRtxLook({ inst }: { inst: Instance }) {
  const { toast, refreshInstances } = useStore()
  const [state, setState] = useState<RtxLookState>()
  const [busy, setBusy] = useState(false)
  const [setup, setSetup] = useState(false)
  const [shader, setShader] = useState('auto')
  const [pack, setPack] = useState<RtxPackChoice>('auto')
  const [ownPack, setOwnPack] = useState<string>()
  const [tune, setTune] = useState(true)
  const [memoryMb, setMemoryMb] = useState<number>()

  useEffect(() => {
    let live = true
    api.content.rtxLookState(inst.id).then((s) => live && setState(s), () => undefined)
    return () => {
      live = false
    }
  }, [inst.id])

  if (!state) return null

  const shaderTitle = (key: string) => state.shaders.find((x) => x.key === key)?.title ?? key
  const recPack = state.packs.find((p) => p.key === state.recommended.pack)
  const picked = pack === 'auto' ? state.recommended.pack : pack

  const install = async () => {
    setBusy(true)
    toast(t('rtxLook.downloading', { name: inst.name }), 'info')
    try {
      setState(await api.content.rtxLookInstall(inst.id, { shader, pack, ownPack, tune, memoryMb, world: false }))
      await refreshInstances()
      setSetup(false)
      toast(t('rtxLook.ready', { name: inst.name }))
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(false)
    }
  }
  const set = async (on: boolean) => {
    setBusy(true)
    try {
      setState(await api.content.rtxLookSet(inst.id, on))
      toast(t(on ? 'rtxLook.onDone' : 'rtxLook.offDone', { name: inst.name }))
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(false)
    }
  }

  const form = (
    <>
      <div className="setting stack">
        <div className="text">
          <b>{t('rtxLook.shader')}</b>
          <span>{t('rtxLook.shaderHint')}</span>
        </div>
        <select className="input" value={shader} disabled={busy} onChange={(e) => setShader(e.target.value)}>
          <option value="auto">{t('rtxLook.auto', { name: shaderTitle(state.recommendedShader) })}</option>
          {state.shaders.map((x) => (
            <option key={x.key} value={x.key}>
              {`${x.title} · ${t(`rtxLook.kind.${x.key}` as Key)}`}
            </option>
          ))}
        </select>
      </div>
      <div className="setting stack">
        <div className="text">
          <b>{t('rtxPack.textures')}</b>
          <span>{t('rtxPack.texturesHint')}</span>
        </div>
        <select className="input" value={pack} disabled={busy} onChange={(e) => setPack(e.target.value as RtxPackChoice)}>
          <option value="auto">{t('rtxPack.auto', { pack: recPack ? `${recPack.res}x ${recPack.title}` : `${state.recommended.pack}x` })}</option>
          {state.packs.map((p) => (
            <option key={p.key} value={p.key}>
              {`${p.res}x · ${p.title}${p.size ? ` (${bytes(p.size)})` : ''}`}
            </option>
          ))}
          <option value="own">{t('rtxPack.own')}</option>
          <option value="none">{t('rtxPack.none')}</option>
        </select>
        {pack === 'own' && (
          <div className="rtx-own">
            <button
              className="btn sm"
              disabled={busy}
              onClick={() => void api.content.rtxPackPickOwn().then((p) => p && setOwnPack(p))}
            >
              <FolderOpen /> {t('rtxPack.pick')}
            </button>
            <span className="hint">{ownPack ? ownPack.split(/[\\/]/).pop() : t('rtxPack.ownHint')}</span>
          </div>
        )}
      </div>
      <div className="setting">
        <div className="text">
          <b>{t('rtxPack.tune')}</b>
          <span>
            {t('rtxLook.tuneHint', {
              render: state.recommended.renderDistance,
              simulation: state.recommended.simulationDistance,
              memory: gb(memoryMb ?? memoryFor(state.spec, picked))
            })}
          </span>
        </div>
        <div className="control">
          <Switch on={tune} onChange={setTune} label={t('rtxPack.tune')} disabled={busy} />
        </div>
      </div>
      <MemoryPick spec={state.spec} auto={memoryFor(state.spec, picked)} value={memoryMb} onChange={setMemoryMb} disabled={busy} />
      <div className="setting">
        <div className="text">
          <span>{t('rtxLook.loaderHint', { loader: state.loader === 'oculus' ? 'Oculus' : state.loader === 'optifine' ? 'OptiFine' : 'Iris' })}</span>
        </div>
        <div className="control rtx-toggle">
          <button className="btn primary" disabled={busy || (pack === 'own' && !ownPack)} onClick={() => void install()}>
            {busy ? <Loader2 className="spin" /> : <Sparkles />} {t(state.installed ? 'rtxLook.apply' : 'rtxLook.get')}
          </button>
        </div>
      </div>
    </>
  )

  return (
    <div className="settings-section rtx-remix">
      <h3>{t('rtxLook.title')}</h3>
      <div className="card">
        {!state.supported ? (
          <div className="setting">
            <div className="text">
              <b>
                <Sparkles /> {t('rtxLook.title')}
              </b>
              <span>{t(state.why === 'loader' ? 'rtxLook.whyLoader' : 'rtxLook.whyVersion', { version: inst.mcVersion })}</span>
            </div>
          </div>
        ) : state.installed ? (
          <>
            <div className="setting">
              <div className="text">
                <b>
                  <Sparkles /> {t('rtxLook.switch', { name: shaderTitle(state.shader ?? '') })}
                </b>
                <span>{t(state.on ? 'rtxLook.onHint' : 'rtxLook.offHint')}</span>
              </div>
              <div className="control rtx-toggle">
                {busy && <Loader2 className="spin" />}
                <button className="btn sm ghost" disabled={busy} onClick={() => setSetup((v) => !v)}>
                  {t('rtxLook.change')}
                </button>
                <Switch on={state.on} onChange={(v) => void set(v)} label={t('rtxLook.title')} disabled={busy} />
              </div>
            </div>
            {setup && form}
          </>
        ) : (
          <>
            <div className="setting">
              <div className="text">
                <b>
                  <Sparkles /> {t('rtxLook.title')}
                </b>
                <span>{t('rtxLook.intro')}</span>
              </div>
            </div>
            {form}
          </>
        )}
      </div>
    </div>
  )
}
