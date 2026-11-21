import { useEffect, useRef, useState, type CSSProperties } from 'react'
import { Loader2, MemoryStick } from 'lucide-react'
import type { Instance } from '@shared/types'
import { api } from '../api'
import { useInstanceTask, useStore } from '../store'
import { t, tx, type Key } from '../i18n'
import { describe } from '../util'
import { useLaunchScreen } from '../launchScreenState'
import { InstanceTile, TaskBar } from './ui'
import { usePackArt } from './packArt'
import { RtxBanner, useRtxRemix } from './RtxBanner'
import { readLine, shortLine, STAGE_COUNT, stageFraction, startProgress, type LaunchProgress } from '../launchProgress'

/**
 * Off Windows, where the engine can't see the game's window (game/window.rs): what the game prints once its window
 * exists. The rendering library reports itself as the window is made, and resources load right after.
 */
const WINDOW_UP = /LWJGL|Backend library|Reloading ResourceManager/i
/**
 * What the game prints when it has finished loading, just before its title screen: the sound engine starts last,
 * on every version from 1.7 on (or says it couldn't). The screen folds then, not when the window first appears:
 * Forge's and the game's own loading screens can take minutes in a big modpack.
 */
const READY = /Sound engine started|Sound engine startup failed|Error starting SoundSystem|Sound system started/i
/** If the game never says it is ready (no sound device, an unusual version): fold anyway after this long. */
const READY_WAIT = 300_000
/** How often what the game prints is shown while it loads. */
const SHOW_MS = 250
/** The fold's length: keep in step with `launch-fold` in styles.css. */
const FOLD_MS = 560

/**
 * The launch screen: from pressing Play until the game has finished loading, the launcher's window shows the
 * instance (its picture behind its icon and name), what is being prepared and how far along it is, then what the
 * game is printing while it starts and loads. When the game says it is ready (`READY`) the screen folds away. Close
 * puts it away for this launch; Settings → Game turns it off for good.
 */
export default function LaunchScreen() {
  const { games, instances, info } = useStore()
  const on = useLaunchScreen()
  const [closed, setClosed] = useState<string>()
  /** Folding away because the game's window has just come up: shown a moment longer, as it folds. */
  const [folding, setFolding] = useState<string>()
  /** The instance whose process has started but that hasn't finished loading yet, and the last line it printed. */
  const [starting, setStarting] = useState<string>()
  const [said, setSaid] = useState('')
  /** Its window is up and it is loading: the screen says so while it waits for the game to be ready. */
  const [windowUp, setWindowUp] = useState(false)
  /** What the game's log says it is doing: its stage and the mods it is going through. */
  const [progress, setProgress] = useState<LaunchProgress>(startProgress())
  const preparing = instances.find((i) => games[i.id] === 'preparing')
  const was = useRef<string | undefined>(undefined)

  // Preparing has just ended in a running game: stay until its window is up.
  useEffect(() => {
    const before = was.current
    was.current = preparing?.id
    if (before && before !== preparing?.id && games[before] === 'running') {
      setStarting(before)
      setSaid('')
      setWindowUp(false)
      setProgress(startProgress())
      // Forge counts mod files as it finds them: against the instance's own enabled mods.
      void api.content
        .list(before, 'mod')
        .then((mods) => {
          const total = mods.filter((m) => m.enabled && !m.native).length
          if (total) setProgress((p) => (p.modsTotal ? p : { ...p, modsTotal: total }))
        })
        .catch(() => {})
    }
  }, [preparing?.id, games])

  useEffect(() => {
    if (!starting) return
    // It stopped or crashed before a window came up: no fold, there is no game to open into.
    if (games[starting] !== 'running') return setStarting(undefined)
    const fold = () => {
      setFolding(starting)
      setStarting(undefined)
    }
    const watched = info?.platform === 'win32'
    // A big modpack prints thousands of lines while it loads: they're read as they come but shown a few times a
    // second, so the launcher redraws little while the game needs the PC.
    let queued: string[] = []
    const flush = () => {
      if (!queued.length) return
      const lines = queued
      queued = []
      setSaid(shortLine(lines[lines.length - 1]))
      setProgress((p) => lines.reduce((acc, l) => readLine(acc, l), p))
    }
    const shown = window.setInterval(flush, SHOW_MS)
    const offLog = api.on.log((e) => {
      if (e.instanceId !== starting) return
      for (const l of e.lines) if (l.text) queued.push(l.text)
      if (!watched && e.lines.some((l) => WINDOW_UP.test(l.text))) setWindowUp(true)
      if (e.lines.some((l) => READY.test(l.text))) {
        flush()
        fold()
      }
    })
    // The engine saw the game's own window appear: it is loading now.
    const offWindow = api.on.gameWindow((e) => e.instanceId === starting && setWindowUp(true))
    const timer = window.setTimeout(fold, READY_WAIT)
    return () => {
      offLog()
      offWindow()
      window.clearTimeout(timer)
      window.clearInterval(shown)
    }
    // `games` only matters for the stop/crash check above; re-subscribing on every change would restart the wait.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [starting, games[starting ?? ''], info?.platform])

  useEffect(() => {
    if (!folding) return
    const timer = window.setTimeout(() => setFolding(undefined), FOLD_MS)
    return () => window.clearTimeout(timer)
  }, [folding])

  // The render where preparing has just ended comes before the effect above marks it as starting: it counts as
  // starting already, so the screen doesn't vanish for a frame and fade back in.
  const handing = !preparing && !starting && was.current && games[was.current] === 'running' ? was.current : undefined
  const inst = on ? (preparing ?? instances.find((i) => i.id === (starting ?? handing ?? folding))) : undefined
  const id = inst?.id
  // The next launch shows it again.
  useEffect(() => {
    if (!id) setClosed(undefined)
  }, [id])
  if (!inst || closed === inst.id) return null
  return (
    <Screen
      inst={inst}
      said={preparing ? undefined : handing ? '' : said}
      progress={progress}
      loading={!preparing && windowUp}
      folding={!preparing && folding === inst.id}
      onClose={() => setClosed(inst.id)}
    />
  )
}

/**
 * Behind the name: the instance's banner, the RTX scene, or its modpack's art; else its own icon, blown up and
 * blurred into a glow of its colours, with the icon's colour as the floor.
 */
function Backdrop({ inst, rtx }: { inst: Instance; rtx: boolean }) {
  const art = usePackArt(inst, !!inst.bannerUrl || rtx)
  const picture = inst.bannerUrl ?? art
  if (picture) return <img loading="lazy" decoding="async" className="launch-photo" src={picture} alt="" draggable={false} />
  if (rtx) return <RtxBanner />
  return (
    <div className="launch-glow" style={{ '--glow': inst.color ?? '#7c3aed' } as CSSProperties}>
      {inst.iconUrl && <img loading="lazy" decoding="async" src={inst.iconUrl} alt="" draggable={false} />}
    </div>
  )
}

/** `said`: set once the game's process is running; the last line the game printed while its window comes up. */
function Screen({
  inst,
  said,
  progress,
  loading,
  folding,
  onClose
}: {
  inst: Instance
  said?: string
  progress: LaunchProgress
  loading: boolean
  folding: boolean
  onClose: () => void
}) {
  const { settings } = useStore()
  const task = useInstanceTask(inst.id)
  const rtx = useRtxRemix(inst) && !inst.bannerUrl
  const memory = inst.memoryMb ?? settings?.memoryMb
  const [version, loader] = describe(inst).split(' · ')
  const started = said !== undefined
  const ram = useGameRam(inst.id, started)

  useEffect(() => {
    // Escape closes it, unless a dialog is open over it: that one takes the key.
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && !document.querySelector('.backdrop') && onClose()
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [onClose])
  // Keyboard focus moves onto it, so Tab and Enter act here and not on the page underneath.
  const close = useRef<HTMLButtonElement>(null)
  useEffect(() => close.current?.focus({ preventScroll: true }), [])

  return (
    <div
      className={`launch-screen ${folding ? 'folding' : ''} ${started && !folding ? 'loading' : ''}`}
      role="dialog"
      aria-label={`${t(started ? 'servers.state.starting' : 'common.preparing')}: ${inst.name}`}
    >
      <div className="launch-drag" data-tauri-drag-region />
      <div className="launch-art" aria-hidden>
        <Backdrop inst={inst} rtx={rtx} />
      </div>
      <button ref={close} className="btn ghost launch-close" onClick={onClose}>
        {t('common.close')}
      </button>
      <div className="launch-body">
        <InstanceTile inst={inst} className="launch-icon" />
        <h1>{inst.name}</h1>
        <div className="launch-chips">
          <span className="chip">{version}</span>
          {loader && <span className="chip">{loader}</span>}
          {ram ? (
            // While it starts: how much memory the game uses now, of what it may use.
            <span className="chip launch-ram" title={t('settings.memory')}>
              <MemoryStick /> {gb(ram.bytes)} <span className="launch-ram-max">/ {gb(ram.maxBytes || (memory ?? 0) * 1024 * 1024)} GB</span>
              <i style={{ width: `${Math.min(100, (ram.bytes / (ram.maxBytes || (memory ?? 1) * 1024 * 1024)) * 100)}%` }} aria-hidden />
            </span>
          ) : (
            memory != null && (
              <span className="chip" title={t('settings.memory')}>
                <MemoryStick /> {memory} MB
              </span>
            )
          )}
        </div>
      </div>
      <div className="launch-progress" role="status">
        {started ? (
          <GameProgress progress={progress} said={said ?? ''} loading={loading} />
        ) : (
          <div className="launch-status">
            <Loader2 className="spin" />
            {
              <>
                <b>{task ? tx(task.titleText, task.title) : t('common.preparing')}</b>
                <span className="launch-detail ellipsis">{task ? tx(task.detailText, task.detail) : ''}</span>
                {task?.progress != null && <span className="launch-pct">{Math.round(task.progress * 100)}%</span>}
              </>
            }
          </div>
        )}
        {!started && <TaskBar progress={task?.progress} />}
      </div>
    </div>
  )
}

const GIB = 1024 * 1024 * 1024
/** Gigabytes with one decimal: "1.4". */
const gb = (bytes: number) => (bytes / GIB).toFixed(1)

/** While the game's process runs: what it uses now and may use at most, read every 1.5 s (the engine's measure). */
function useGameRam(id: string, on: boolean): { bytes: number; maxBytes: number } | undefined {
  const [ram, setRam] = useState<{ bytes: number; maxBytes: number }>()
  useEffect(() => {
    if (!on) return setRam(undefined)
    let live = true
    const read = () =>
      void api.app
        .memory()
        .then((m) => {
          const runs = m.games.filter((g) => g.instanceId === id)
          if (live && runs.length) setRam({ bytes: runs.reduce((a, g) => a + g.bytes, 0), maxBytes: Math.max(...runs.map((g) => g.maxBytes)) })
        })
        .catch(() => {})
    read()
    const timer = window.setInterval(read, 1500)
    return () => {
      live = false
      window.clearInterval(timer)
    }
  }, [id, on])
  return ram
}

const STAGE_NAMES: Key[] = ['launch.stage.java', 'launch.stage.mods', 'launch.stage.game', 'launch.stage.resources']

/**
 * Once the game's process runs, like the old Forge loading screen: a bar for the stage it is in (of four), and one
 * for the mods it is going through, with the one it is on. The last line it printed sits under them, small.
 */
function GameProgress({ progress: p, said, loading }: { progress: LaunchProgress; said: string; loading: boolean }) {
  // The window is up but the log hasn't said more than "mods": it is at least starting Minecraft.
  const stage = loading && p.stage < 2 ? 2 : p.stage
  const fraction = stageFraction({ ...p, stage })
  // Only once the game has said something about its mods (a Forge mod file found, Fabric's list): before that the row
  // would sit there empty while Java starts.
  const modsKnown = p.mods > 0
  // The second bar: the mods while it goes through them (complete once it has moved on), then the textures.
  const textures = stage === 3 && p.atlases > 0
  const modsDone = stage > 1
  const subLabel = textures ? t('launch.textures') : t('launch.mods')
  const subCurrent = textures || stage <= 2 ? p.current : ''
  const subCount = textures
    ? String(p.atlases)
    : p.modsTotal
      ? `${modsDone ? p.modsTotal : Math.min(p.mods, p.modsTotal)} / ${p.modsTotal}`
      : String(p.mods)
  const subFraction = textures ? Math.min(0.95, p.atlases / 8) : modsDone ? 1 : p.modsTotal ? Math.min(1, p.mods / p.modsTotal) : 0
  return (
    <div className="launch-stages">
      <div className="launch-stage">
        <div className="launch-stage-top">
          <Loader2 className="spin" />
          <b>{t(STAGE_NAMES[stage])}</b>
          <span className="launch-stage-count">{t('launch.stage.of', { n: stage + 1, total: STAGE_COUNT })}</span>
        </div>
        <div className="bar">
          <i style={{ width: `${fraction * 100}%` }} />
        </div>
      </div>
      {(modsKnown || textures) && (
        <div className="launch-stage sub">
          <div className="launch-stage-top">
            <span>{subLabel}</span>
            <span className="launch-detail ellipsis">{subCurrent}</span>
            <span className="launch-stage-count">{subCount}</span>
          </div>
          <div className="bar">
            <i style={{ width: `${subFraction * 100}%` }} />
          </div>
        </div>
      )}
      <span className="launch-said ellipsis">{said}</span>
    </div>
  )
}
