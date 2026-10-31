import { useEffect, useState, type ReactNode } from 'react'
import { Cpu, Loader2, TriangleAlert, Undo2 } from 'lucide-react'
import type { Instance, MulticoreItem, MulticorePlan, ServerInfo, ServerMulticorePlan } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { t, type Key } from '../i18n'
import { Modal, Switch } from './ui'

const STATE: Record<MulticoreItem['state'], Key> = {
  install: 'servers.multicore.state.install',
  have: 'servers.multicore.state.have',
  off: 'servers.multicore.state.off',
  conflict: 'servers.multicore.state.conflict',
  oldLoader: 'servers.multicore.state.oldLoader',
  oldJava: 'servers.multicore.state.oldJava',
  none: 'servers.multicore.state.none'
}

const SLOT: Record<string, Key> = {
  own: 'servers.multicore.slot.own',
  render: 'servers.multicore.slot.render',
  chunks: 'servers.multicore.slot.chunks',
  logic: 'servers.multicore.slot.logic'
}

const REASON: Record<NonNullable<MulticorePlan['reason']>, Key> = {
  vanilla: 'servers.multicore.reason.vanilla',
  noLoader: 'servers.multicore.reason.noLoader',
  noMods: 'servers.multicore.reason.noMods',
  plugins: 'servers.multicore.reason.plugins',
  hybrid: 'servers.multicore.reason.hybrid',
  bedrock: 'servers.multicore.reason.bedrock',
  folia: 'servers.multicore.reason.folia'
}

const SERVER_NAMES: Record<string, string> = { paper: 'Paper', purpur: 'Purpur', leaf: 'Leaf', spigot: 'Spigot' }

function Items({ items }: { items: MulticoreItem[] }) {
  return (
    <div className="card">
      {items.map((i) => (
        <div className="setting" key={i.slot}>
          <div className="text">
            <b>
              {i.title ?? t(SLOT[i.slot] ?? 'servers.multicore.slot.own')}
              {i.version ? ` ${i.version}` : ''}
            </b>
            <span>
              {t(SLOT[i.slot] ?? 'servers.multicore.slot.own')} · {t(STATE[i.state], { with: i.with })}
              {i.state === 'install' && i.test ? ` ${t('servers.multicore.testBuild')}` : ''}
            </span>
          </div>
        </div>
      ))}
    </div>
  )
}

/** The dialog both setups share: what would happen, then one button to do it and one to take it back. */
function SetupModal<P extends MulticorePlan>({
  title,
  load,
  apply,
  undo,
  notice,
  extra,
  onClose
}: {
  title: string
  load: () => Promise<P>
  apply: () => Promise<P>
  undo: () => Promise<P>
  notice?: string
  extra?: (plan: P, set: (p: P) => void, busy: boolean, run: (work: () => Promise<P>, done: string) => Promise<void>) => ReactNode
  onClose: () => void
}) {
  const { toast } = useStore()
  const [plan, setPlan] = useState<P>()
  const [busy, setBusy] = useState(false)
  useEffect(() => {
    let live = true
    void load().then(
      (p) => live && setPlan(p),
      (e) => {
        toast((e as Error).message, 'error')
        onClose()
      }
    )
    return () => void (live = false)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])
  const run = async (work: () => Promise<P>, done: string) => {
    setBusy(true)
    try {
      setPlan(await work())
      toast(done, 'success')
    } catch (e) {
      toast((e as Error).message, 'error')
      await load().then(setPlan, () => {})
    } finally {
      setBusy(false)
    }
  }
  const count = plan?.items.filter((i) => i.state === 'install').length ?? 0
  return (
    <Modal
      wide
      title={title}
      onClose={onClose}
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            {t('common.close')}
          </button>
          {!!plan?.undo.length && (
            <button className="btn" disabled={busy} onClick={() => void run(undo, t('servers.multicore.undone'))}>
              <Undo2 /> {t('servers.multicore.undo', { count: plan.undo.length })}
            </button>
          )}
          {count > 0 && (
            <button className="btn primary" disabled={busy} onClick={() => void run(apply, t('servers.multicore.done'))}>
              {busy ? <Loader2 className="spin" /> : <Cpu />} {t('servers.multicore.install', { count })}
            </button>
          )}
        </>
      }
    >
      {!plan ? (
        <Loader2 className="spin" />
      ) : (
        <>
          {plan.reason ? <p>{t(REASON[plan.reason])}</p> : <p>{t('servers.multicore.intro')}</p>}
          {plan.items.length > 0 && !plan.reason && <Items items={plan.items} />}
          {notice && count > 0 && <p>{notice}</p>}
          {!!plan.undo.length && <p className="muted">{t('servers.multicore.undoHint', { list: plan.undo.join(', ') })}</p>}
          {extra?.(plan, setPlan, busy, run)}
        </>
      )}
    </Modal>
  )
}

function SetupRow({ hint, onOpen }: { hint: string; onOpen: () => void }) {
  return (
    <div className="setting">
      <div className="text">
        <b>{t('servers.multicore.title')}</b>
        <span>{hint}</span>
      </div>
      <button className="btn" onClick={onOpen}>
        <Cpu /> {t('servers.multicore.open')}
      </button>
    </div>
  )
}

/** Instance → Settings: installs the multi-core mods that fit this instance. */
export function InstanceMulticore({ inst }: { inst: Instance }) {
  const [open, setOpen] = useState(false)
  return (
    <div className="settings-section">
      <h3>{t('servers.multicore.title')}</h3>
      <div className="card">
        <SetupRow hint={t('servers.multicore.hint')} onOpen={() => setOpen(true)} />
      </div>
      {open && (
        <SetupModal
          title={t('servers.multicore.titleFor', { name: inst.name })}
          load={() => api.content.multicorePlan(inst.id)}
          apply={() => api.content.multicoreApply(inst.id)}
          undo={() => api.content.multicoreUndo(inst.id)}
          onClose={() => setOpen(false)}
        />
      )}
    </div>
  )
}

/** Server → Settings: the mods on a mod server; on a plugin server, the honest answer and the choice of moving to Folia. */
export function ServerMulticore({ s, reload }: { s: ServerInfo; reload: () => Promise<void> }) {
  const [open, setOpen] = useState(false)
  const [understood, setUnderstood] = useState(false)
  const off = s.state === 'stopped' || s.state === 'crashed'
  const label = (kind: string) => SERVER_NAMES[kind] ?? kind
  const changed = async (p: ServerMulticorePlan) => {
    setUnderstood(false)
    await reload().catch(() => {})
    return p
  }
  const folia = (plan: ServerMulticorePlan, _set: unknown, busy: boolean, run: (w: () => Promise<ServerMulticorePlan>, done: string) => Promise<void>) => (
    <>
      {plan.moved && (
        <div className="card">
          <div className="setting">
            <div className="text">
              <b>{t('servers.multicore.back.title', { kind: label(plan.moved.kind) })}</b>
              <span>{t(off ? 'servers.multicore.back.hint' : 'servers.multicore.folia.stopFirst')}</span>
            </div>
            <button
              className="btn"
              disabled={busy || !off}
              onClick={() => void run(() => api.server.undoFolia(s.id).then(changed), t('servers.multicore.back.done', { kind: label(plan.moved?.kind ?? '') }))}
            >
              <Undo2 /> {t('servers.multicore.back.button')}
            </button>
          </div>
        </div>
      )}
      {plan.folia && !plan.folia.available && <p>{t('servers.multicore.folia.noBuild', { mc: s.mcVersion })}</p>}
      {plan.folia?.available && (
        <div className="card">
          <div className="setting">
            <div className="text">
              <b>
                <TriangleAlert /> {t('servers.multicore.folia.title')}
              </b>
              <span>{t('servers.multicore.folia.warning')}</span>
            </div>
          </div>
          <div className="setting">
            <div className="text">
              <b>{t('servers.multicore.folia.plugins')}</b>
              <span>
                {plan.folia.blocked.length
                  ? t('servers.multicore.folia.blocked', { list: plan.folia.blocked.join(', ') })
                  : t('servers.multicore.folia.noneBlocked')}
                {plan.folia.ready.length ? ` ${t('servers.multicore.folia.ready', { list: plan.folia.ready.join(', ') })}` : ''}
              </span>
            </div>
          </div>
          <div className="setting">
            <div className="text">
              <b>{t('servers.multicore.folia.understand')}</b>
              <span>{t(off ? 'servers.multicore.folia.backupHint' : 'servers.multicore.folia.stopFirst')}</span>
            </div>
            <Switch on={understood} onChange={setUnderstood} label={t('servers.multicore.folia.understand')} />
          </div>
          <div className="setting">
            <div className="text" />
            <button
              className="btn danger"
              disabled={busy || !understood || !off}
              onClick={() => void run(() => api.server.moveToFolia(s.id, true).then(changed), t('servers.multicore.folia.done'))}
            >
              {busy ? <Loader2 className="spin" /> : <TriangleAlert />} {t('servers.multicore.folia.button')}
            </button>
          </div>
        </div>
      )}
    </>
  )
  return (
    <div className="settings-section">
      <h3>{t('servers.multicore.title')}</h3>
      <div className="card">
        <SetupRow hint={t('servers.multicore.hint')} onOpen={() => setOpen(true)} />
      </div>
      {open && (
        <SetupModal<ServerMulticorePlan>
          title={t('servers.multicore.titleFor', { name: s.name })}
          load={() => api.server.multicorePlan(s.id)}
          apply={() => api.server.multicoreApply(s.id)}
          undo={() => api.server.multicoreUndo(s.id)}
          notice={off ? undefined : t('servers.multicore.nextStart')}
          extra={folia}
          onClose={() => {
            setOpen(false)
            setUnderstood(false)
          }}
        />
      )}
    </div>
  )
}
