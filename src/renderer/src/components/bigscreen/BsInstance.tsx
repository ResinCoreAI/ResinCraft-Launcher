// Big screen mode: one instance (mock "Instance" in .impeccable/mocks/big-screen-full.html). Its picture, Play, the
// controller mod switch and its numbers on top; its worlds, mods, screenshots and servers as tabs (LT/RT) below.
import { useEffect, useRef, useState, type MutableRefObject } from 'react'
import { Gamepad2, Globe, Loader2, Play, Settings2, Square } from 'lucide-react'
import type { ContentItem, Instance, ScreenshotInfo, ServerEntry, WorldInfo } from '@shared/types'
import { api } from '../../api'
import { useStore, type Route } from '../../store'
import { t, type Key } from '../../i18n'
import { describe, playtime, timeAgo } from '../../util'
import { useControllerMod } from '../ControllerSupport'
import { Art, BsSheet, useTabSteps } from './parts'

const TABS = ['worlds', 'mods', 'screenshots', 'servers'] as const
type Tab = (typeof TABS)[number]
const TAB_LABEL: Record<Tab, Key> = {
  worlds: 'bigscreen.inst.worlds',
  mods: 'bigscreen.inst.mods',
  screenshots: 'bigscreen.inst.screenshots',
  servers: 'bigscreen.inst.servers'
}
const MODE: Record<string, Key> = {
  survival: 'bigscreen.mode.survival',
  creative: 'bigscreen.mode.creative',
  adventure: 'bigscreen.mode.adventure',
  spectator: 'bigscreen.mode.spectator'
}
const gb = (bytes: number) => (bytes >= 1e9 ? `${(bytes / 1e9).toFixed(1)} GB` : `${Math.max(1, Math.round(bytes / 1e6))} MB`)

export default function BsInstance({
  inst,
  startTab = 'worlds',
  openPage,
  backRef
}: {
  inst: Instance
  startTab?: Tab
  openPage: (r: Route) => void
  /** Big screen asks this first when B is pressed: true when it closed something of its own. */
  backRef: MutableRefObject<(() => boolean) | undefined>
}) {
  const { games, play, stop, playWorld, join, toast } = useStore()
  const [tab, setTab] = useState<Tab>(startTab)
  const [worlds, setWorlds] = useState<WorldInfo[]>()
  const [mods, setMods] = useState<ContentItem[]>()
  const [shots, setShots] = useState<ScreenshotInfo[]>()
  const [thumbs, setThumbs] = useState<Record<string, string>>({})
  const [servers, setServers] = useState<ServerEntry[]>()
  const [viewing, setViewing] = useState<{ name: string; url?: string }>()
  const controller = useControllerMod(inst)
  const running = games[inst.id] === 'running' || games[inst.id] === 'preparing'
  useTabSteps(TABS, tab, setTab, !viewing)

  useEffect(() => {
    void api.worlds.list(inst.id).then(setWorlds, () => setWorlds([]))
    void api.content.list(inst.id, 'mod').then(setMods, () => setMods([]))
    void api.screenshots.list(inst.id).then(setShots, () => setShots([]))
    void api.servers.list(inst.id).then((s) => setServers(s.filter((x) => !x.hidden)), () => setServers([]))
  }, [inst.id])

  // Screenshot thumbnails, a few at a time, only once their tab is open.
  useEffect(() => {
    if (tab !== 'screenshots' || !shots) return
    let live = true
    void (async () => {
      for (const s of shots.slice(0, 24)) {
        if (!live) return
        if (thumbs[s.name]) continue
        const url = await api.screenshots.thumbnail(inst.id, s.name).catch(() => '')
        if (live && url) setThumbs((m) => ({ ...m, [s.name]: url }))
      }
    })()
    return () => {
      live = false
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tab, shots, inst.id])

  useEffect(() => {
    backRef.current = () => {
      if (!viewing) return false
      setViewing(undefined)
      return true
    }
    return () => {
      backRef.current = undefined
    }
  }, [viewing, backRef])

  // Turning a mod off renames its file (".disabled" on the end), so the list is read again afterwards: the old name
  // is gone, and toggling it a second time by that name would fail. One at a time, so a held A can't race it.
  const toggling = useRef(false)
  const toggleMod = async (m: ContentItem) => {
    if (toggling.current) return
    toggling.current = true
    setMods((list) => list?.map((x) => (x.filename === m.filename ? { ...x, enabled: !x.enabled } : x)))
    try {
      await api.content.toggle(inst.id, 'mod', m.filename)
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      await api.content.list(inst.id, 'mod').then(setMods, () => undefined)
      toggling.current = false
    }
  }
  const view = async (s: ScreenshotInfo) => {
    setViewing({ name: s.name, url: thumbs[s.name] })
    const url = await api.screenshots.image(inst.id, s.name).catch(() => undefined)
    if (url) setViewing((v) => (v?.name === s.name ? { name: s.name, url } : v))
  }

  const enabledMods = mods?.filter((m) => m.enabled).length

  return (
    <div className="bsi">
      <div className="bsi-hero" aria-hidden>
        <Art inst={inst} className="bs-hero-art" />
      </div>
      <section className="bsi-head">
        <div className="bsi-title">
          <span className="bs-chip">{describe(inst)}</span>
          <h1>{inst.name}</h1>
          <div className="bs-actions">
            {running ? (
              <button className="bs-big stop" data-first onClick={() => void stop(inst.id)}>
                <Square /> {games[inst.id] === 'preparing' ? t('bigscreen.starting') : t('bigscreen.stop')}
              </button>
            ) : (
              <button className="bs-big play" data-first onClick={() => void play(inst.id)}>
                <Play /> {t('bigscreen.play')}
              </button>
            )}
            {controller.modded && (
              <button className={`bs-big ${controller.on ? 'on' : ''}`} disabled={!controller.usable} onClick={() => void controller.set(!controller.on)}>
                {controller.busy ? <Loader2 className="spin" /> : <Gamepad2 />} {controller.on ? t('bigscreen.inst.controllerOn') : t('bigscreen.inst.controllerOff')}
              </button>
            )}
            <button className="bs-big" onClick={() => openPage({ page: 'instance', id: inst.id, tab: 'settings' })}>
              <Settings2 /> {t('bigscreen.inst.settings')}
            </button>
          </div>
        </div>
        <dl className="bsi-stats">
          <div>
            <dt>{t('bigscreen.inst.played')}</dt>
            <dd>{inst.playtime > 0 ? playtime(inst.playtime) : '—'}</dd>
          </div>
          <div>
            <dt>{t('bigscreen.inst.mods')}</dt>
            <dd>{enabledMods ?? '…'}</dd>
          </div>
          {inst.memoryMb ? (
            <div>
              <dt>{t('bigscreen.inst.memory')}</dt>
              <dd>{gb(inst.memoryMb * 1e6)}</dd>
            </div>
          ) : null}
          <div>
            <dt>{t('bigscreen.inst.worlds')}</dt>
            <dd>{worlds?.length ?? '…'}</dd>
          </div>
        </dl>
      </section>

      <nav className="bsi-tabs" aria-label={inst.name}>
        <span className="bs-bumper" aria-hidden>
          LT
        </span>
        {TABS.map((x) => (
          <button key={x} className={`bsi-tab ${tab === x ? 'on' : ''}`} aria-current={tab === x ? 'page' : undefined} onClick={() => setTab(x)}>
            {t(TAB_LABEL[x])}
          </button>
        ))}
        <span className="bs-bumper" aria-hidden>
          RT
        </span>
      </nav>

      <section className="bsi-body">
        {tab === 'worlds' &&
          (!worlds ? (
            <Loader2 className="spin" />
          ) : worlds.length ? (
            <div className="bsi-grid">
              {worlds.map((w) => (
                <button key={w.folder} className="bsi-world" onClick={() => void playWorld(inst.id, w.folder)} aria-label={t('bigscreen.inst.playWorld', { name: w.name })}>
                  <span className="bsi-world-pic">{w.icon ? <img loading="lazy" decoding="async" src={w.icon} alt="" /> : <Globe />}</span>
                  <span className="bsi-world-text">
                    <b>{w.name}</b>
                    <span>
                      {[w.gameMode && MODE[w.gameMode] ? t(MODE[w.gameMode]) : '', w.lastPlayed ? timeAgo(w.lastPlayed) : '', gb(w.size)].filter(Boolean).join(' · ')}
                    </span>
                  </span>
                </button>
              ))}
            </div>
          ) : (
            <p className="bs-note">{t('bigscreen.inst.noWorlds')}</p>
          ))}

        {tab === 'mods' &&
          (!mods ? (
            <Loader2 className="spin" />
          ) : mods.length ? (
            <div className="bsi-list">
              {mods.map((m) => (
                <button key={m.filename.replace(/\.disabled$/, '')} className={`bsi-mod ${m.enabled ? '' : 'off'}`} onClick={() => void toggleMod(m)} aria-pressed={m.enabled}>
                  <span className="bsi-mod-ic">{m.iconUrl ? <img loading="lazy" decoding="async" src={m.iconUrl} alt="" /> : null}</span>
                  <span className="bsi-mod-text">
                    <b>{m.title ?? m.filename}</b>
                    <span>{m.versionNumber ?? m.filename}</span>
                  </span>
                  <span className={`bs-chip ${m.enabled ? 'ok' : ''}`}>{m.enabled ? t('bigscreen.inst.modOn') : t('bigscreen.inst.modOff')}</span>
                </button>
              ))}
            </div>
          ) : (
            <p className="bs-note">{t('bigscreen.inst.noMods')}</p>
          ))}

        {tab === 'screenshots' &&
          (!shots ? (
            <Loader2 className="spin" />
          ) : shots.length ? (
            <div className="bsi-grid shots">
              {shots.slice(0, 24).map((s) => (
                <button key={s.name} className="bsi-shot" onClick={() => void view(s)} aria-label={s.name}>
                  {thumbs[s.name] ? <img loading="lazy" decoding="async" src={thumbs[s.name]} alt="" /> : <Loader2 className="spin" />}
                </button>
              ))}
            </div>
          ) : (
            <p className="bs-note">{t('bigscreen.inst.noShots')}</p>
          ))}

        {tab === 'servers' &&
          (!servers ? (
            <Loader2 className="spin" />
          ) : servers.length ? (
            <div className="bsi-list">
              {servers.map((s) => (
                <button key={s.ip} className="bsi-mod" onClick={() => void join({ address: s.ip, name: s.name, edition: 'java', instanceId: inst.id })}>
                  <span className="bsi-mod-ic">{s.icon ? <img loading="lazy" decoding="async" src={`data:image/png;base64,${s.icon}`} alt="" /> : <Globe />}</span>
                  <span className="bsi-mod-text">
                    <b>{s.name || s.ip}</b>
                    <span className="mono">{s.ip}</span>
                  </span>
                  <span className="bs-chip">{t('bigscreen.hint.join')}</span>
                </button>
              ))}
            </div>
          ) : (
            <p className="bs-note">{t('bigscreen.inst.noServers')}</p>
          ))}
      </section>

      {viewing && (
        <BsSheet title={viewing.name} onClose={() => setViewing(undefined)} wide>
          <div className="bsi-viewer">{viewing.url ? <img loading="lazy" decoding="async" src={viewing.url} alt="" /> : <Loader2 className="spin" />}</div>
          <button className="bs-big" data-autofocus onClick={() => setViewing(undefined)}>
            {t('bigscreen.hint.close')}
          </button>
        </BsSheet>
      )}
    </div>
  )
}
