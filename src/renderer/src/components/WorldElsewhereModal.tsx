import { useMemo, useState, type ReactNode } from 'react'
import { AlertTriangle, ArrowRightLeft, Copy, Loader2, Play } from 'lucide-react'
import type { Instance, WorldInfo } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { describe, LOADER_LABEL } from '../util'
import { t, type Key } from '../i18n'
import { Modal } from './ui'
import { hasQuickPlay } from './WorldsTab'

export type ElsewhereMode = 'play' | 'move' | 'copy'

const MODES: { mode: ElsewhereMode; icon: ReactNode; label: Key; hint: Key }[] = [
  { mode: 'play', icon: <Play size={15} />, label: 'worlds.elsewhere.play', hint: 'worlds.elsewhere.playHint' },
  { mode: 'move', icon: <ArrowRightLeft size={15} />, label: 'worlds.elsewhere.move', hint: 'worlds.elsewhere.moveHint' },
  { mode: 'copy', icon: <Copy size={15} />, label: 'worlds.elsewhere.copy', hint: 'worlds.elsewhere.copyHint' }
]

/** Compares release numbers ("1.20.1" < "1.21.4" < "26.1"); 0 when either isn't one (a snapshot). */
function compareMc(a: string, b: string): number {
  const parse = (v: string) => (/^\d+(\.\d+)*$/.test(v) ? v.split('.').map(Number) : null)
  const [x, y] = [parse(a), parse(b)]
  if (!x || !y) return 0
  for (let i = 0; i < Math.max(x.length, y.length); i++) {
    const d = (x[i] ?? 0) - (y[i] ?? 0)
    if (d) return d
  }
  return 0
}

/** Plays a world in another instance (it comes back when the game closes), or moves or copies it there. */
export default function WorldElsewhereModal({
  inst,
  world,
  initial,
  onClose,
  onDone
}: {
  inst: Instance
  world: WorldInfo
  initial: ElsewhereMode
  onClose: () => void
  onDone: () => void
}) {
  const { instances, games, toast, playWorldIn } = useStore()
  const others = useMemo(
    () => instances.filter((i) => i.id !== inst.id).sort((a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0) || a.name.localeCompare(b.name)),
    [instances, inst.id]
  )
  const [mode, setMode] = useState(initial)
  const [toId, setToId] = useState(others[0]?.id ?? '')
  const [busy, setBusy] = useState(false)
  const to = others.find((i) => i.id === toId)
  const toRunning = !!to && (games[to.id] === 'running' || games[to.id] === 'preparing')
  const tooOld = mode === 'play' && !!to && !hasQuickPlay(to.mcVersion)
  const otherVersion = !!to && !!world.version && world.version !== to.mcVersion
  // An older game can't read what a newer one saved, so that warning is stronger.
  const older = otherVersion && compareMc(to.mcVersion, world.version!) < 0
  const otherLoader = !!to && to.loader !== inst.loader

  const run = async () => {
    if (!to) return
    setBusy(true)
    try {
      if (mode === 'play') {
        onClose()
        await playWorldIn(inst.id, world.folder, to.id)
        onDone()
        return
      }
      await api.worlds.transfer(inst.id, world.folder, to.id, mode === 'copy')
      toast(t(mode === 'copy' ? 'worlds.elsewhere.copied' : 'worlds.elsewhere.moved', { name: world.name, instance: to.name }), 'success')
      onDone()
      onClose()
    } catch (e) {
      toast((e as Error).message, 'error')
      setBusy(false)
    }
  }

  const action = MODES.find((m) => m.mode === mode)!
  return (
    <Modal
      title={t('worlds.elsewhere.title', { name: world.name })}
      onClose={onClose}
      footer={
        <>
          <button className="btn" onClick={onClose} disabled={busy}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={!to || toRunning || tooOld || busy} onClick={() => void run()}>
            {busy ? <Loader2 className="spin" /> : action.icon} {t(action.label)}
          </button>
        </>
      }
    >
      {others.length === 0 ? (
        <p className="faint">{t('worlds.elsewhere.none')}</p>
      ) : (
        <>
          <div className="field">
            <label htmlFor="world-to">{t('worlds.elsewhere.instance')}</label>
            <select id="world-to" className="select" value={toId} disabled={busy} onChange={(e) => setToId(e.target.value)}>
              {others.map((i) => (
                <option key={i.id} value={i.id}>
                  {i.name} · {describe(i)}
                </option>
              ))}
            </select>
            {to && toRunning && (
              <span className="hint warn">
                <AlertTriangle size={13} /> {t('worlds.elsewhere.running', { name: to.name })}
              </span>
            )}
            {to && tooOld && (
              <span className="hint warn">
                <AlertTriangle size={13} /> {t('worlds.elsewhere.tooOld', { name: to.name, version: to.mcVersion })}
              </span>
            )}
            {to && otherVersion && (
              <span className="hint warn">
                <AlertTriangle size={13} />{' '}
                {t(older ? 'worlds.elsewhere.olderVersion' : 'worlds.elsewhere.otherVersion', { world: world.version!, name: to.name, version: to.mcVersion })}
                {mode === 'play' && ` ${t('worlds.elsewhere.backupFirst')}`}
              </span>
            )}
            {to && otherLoader && (
              <span className="hint warn">
                <AlertTriangle size={13} /> {t('worlds.elsewhere.otherMods', { name: to.name, loader: LOADER_LABEL[to.loader] })}
              </span>
            )}
          </div>

          <div className="field">
            <div className="include-list">
              {MODES.map((m) => (
                <label key={m.mode} className={`include-row ${mode === m.mode ? 'on' : ''}`}>
                  <input type="radio" name="world-mode" disabled={busy} checked={mode === m.mode} onChange={() => setMode(m.mode)} />
                  {m.icon}
                  <span className="grow elsewhere-mode">
                    <span>{t(m.label)}</span>
                    <span className="hint">{t(m.hint)}</span>
                  </span>
                </label>
              ))}
            </div>
          </div>
        </>
      )}
    </Modal>
  )
}
