import { useEffect, useState } from 'react'
import { AlertTriangle, Check, FileText, Loader2, Play, Wrench } from 'lucide-react'
import type { CrashFix, GameCrashEvent } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { t, tx } from '../i18n'
import { Modal } from './ui'

/** What a Fix button will do, in words. */
function fixLabel(f: CrashFix): string {
  switch (f.kind) {
    case 'installMod':
      return t('crash.fix.install', { name: f.name })
    case 'disableMod':
      return t('crash.fix.disable', { file: f.file })
    case 'javaAuto':
      return t('crash.fix.javaAuto')
    case 'memory':
      return t('crash.fix.memory', { mb: f.mb })
  }
}

/**
 * The "Minecraft crashed" window: what the crash fixer found after a crash, a Fix button for each problem it can
 * fix, and Fix all and play again. Nothing changes until the player presses a button.
 */
export default function CrashFixer() {
  const { instances, go, toast, games } = useStore()
  const [crash, setCrash] = useState<GameCrashEvent>()
  const [fixed, setFixed] = useState<Set<number>>(new Set())
  const [busy, setBusy] = useState<number | 'all'>()

  useEffect(() => api.on.gameCrash((e) => (setCrash(e), setFixed(new Set()))), [])
  if (!crash) return null

  const inst = instances.find((i) => i.id === crash.instanceId)
  const fixable = crash.problems.map((p, i) => (p.fix && !fixed.has(i) ? i : -1)).filter((i) => i >= 0)
  const close = () => setCrash(undefined)

  // Fabric's own error window keeps the game open (and its mod files locked): close it before fixing.
  const closeGame = async () => {
    const state = games[crash.instanceId]
    if (state !== 'running' && state !== 'preparing') return
    await new Promise<void>((resolve) => {
      const done = () => (off(), clearTimeout(giveUp), resolve())
      const off = api.on.game((e) => e.runId === crash.runId && (e.state === 'stopped' || e.state === 'crashed') && done())
      const giveUp = setTimeout(done, 10_000)
      void api.instances.kill(crash.instanceId, crash.runId).catch(done)
    })
  }
  const fix = async (i: number) => {
    await closeGame()
    const f = crash.problems[i].fix!
    await api.instances.crashFix(crash.instanceId, f)
    setFixed((s) => new Set(s).add(i))
  }
  const fixOne = async (i: number) => {
    setBusy(i)
    try {
      await fix(i)
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(undefined)
    }
  }
  const fixAllAndPlay = async () => {
    setBusy('all')
    try {
      for (const i of fixable) await fix(i)
      close()
      await api.instances.launch(crash.instanceId)
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(undefined)
    }
  }

  return (
    <Modal
      title={t('crash.title', { name: inst?.name ?? '' })}
      subtitle={crash.problems.length ? t('crash.found') : t('crash.unknown')}
      onClose={close}
      footer={
        <>
          <button
            className="btn ghost"
            onClick={() => {
              close()
              go({ page: 'instance', id: crash.instanceId, tab: 'logs' })
            }}
          >
            <FileText /> {t('crash.logs')}
          </button>
          <span className="grow" />
          <button className="btn" onClick={close}>
            {t('common.close')}
          </button>
          {fixable.length > 0 ? (
            <button className="btn primary" disabled={busy !== undefined} onClick={() => void fixAllAndPlay()}>
              {busy === 'all' ? <Loader2 className="spin" /> : <Wrench />} {t('crash.fixAll')}
            </button>
          ) : (
            crash.problems.some((p) => p.fix) && (
              <button className="btn primary" onClick={() => (close(), void api.instances.launch(crash.instanceId))}>
                <Play /> {t('crash.playAgain')}
              </button>
            )
          )}
        </>
      }
    >
      {crash.problems.length === 0 ? (
        <p className="muted">{t('crash.unknownBody')}</p>
      ) : (
        <ul className="crash-list">
          {crash.problems.map((p, i) => (
            <li key={i} className={fixed.has(i) ? 'fixed' : ''}>
              <AlertTriangle className="icon" />
              <div className="grow">
                <b>{tx(p.title)}</b>
                {p.detail && <p className="hint">{tx(p.detail)}</p>}
                {p.fix && <p className="hint fix-what">{fixLabel(p.fix)}</p>}
              </div>
              {p.fix &&
                (fixed.has(i) ? (
                  <span className="chip success">
                    <Check /> {t('crash.fixed')}
                  </span>
                ) : (
                  <button className="btn sm" disabled={busy !== undefined} onClick={() => void fixOne(i)}>
                    {busy === i ? <Loader2 className="spin" /> : <Wrench />} {t('crash.fix')}
                  </button>
                ))}
            </li>
          ))}
        </ul>
      )}
    </Modal>
  )
}
