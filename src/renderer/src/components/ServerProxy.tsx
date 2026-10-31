// A proxy's Network tab: the servers behind it, try order, forced hosts, forwarding, and what each backend needs
// (server/proxyconfig.rs). On this PC only: the web console and other PCs never reach these calls, so Servers.tsx
// doesn't show the tab there.
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { ArrowDown, ArrowUp, CircleCheck, CircleDashed, CircleX, Copy, Loader2, Plus, Trash2, TriangleAlert, Wrench } from 'lucide-react'
import type { ProxyBackendSetup, ProxyConfigView, ProxyEdit, ProxyForwarding, ServerInfo, ServerKind } from '@shared/types'
import { api } from '../api'
import { ConfirmModal, Segmented, Switch } from './ui'
import { useStore } from '../store'
import { t, type Key } from '../i18n'
import '../server-proxy.css'

export const hasProxyConfig = (type: ServerKind) => ['velocity', 'bungeecord', 'waterfall', 'travertine', 'waterdogpe'].includes(type)

const editOf = (v: ProxyConfigView): ProxyEdit => ({ backends: v.backends, forcedHosts: v.forcedHosts, forwarding: v.forwarding })

/** A name for the file, from the server's name. */
function nameFor(label: string, taken: string[]): string {
  const base = label.toLowerCase().replace(/[^a-z0-9_-]+/g, '-').replace(/^-+|-+$/g, '') || 'server'
  let name = base
  for (let i = 2; taken.includes(name); i++) name = `${base}-${i}`
  return name
}

function Check({ ok, label }: { ok?: boolean | null; label: string }) {
  const icon = ok == null ? <CircleDashed /> : ok ? <CircleCheck /> : <CircleX />
  return (
    <span className={`sv-proxy-check ${ok == null ? '' : ok ? 'ok' : 'bad'}`}>
      {icon} {label}
    </span>
  )
}

function BackendCard({ item, view, onApply, onCopy }: { item: ProxyBackendSetup; view: ProxyConfigView; onApply: () => void; onCopy: () => void }) {
  const modern = view.forwarding === 'modern'
  const secretNeeded = modern && item.advice !== 'bedrock'
  const forwardingLabel = t(modern ? 'servers.proxy.setup.modern' : view.forwarding === 'legacy' ? 'servers.proxy.setup.legacy' : 'servers.proxy.setup.forwarding')
  return (
    <div className="setting stack sv-proxy-setup">
      <div className="sv-proxy-setup-head">
        <b>{item.name}</b>
        <span className="hint">{item.localName ? t('servers.proxy.setup.local', { name: item.localName }) : t('servers.proxy.setup.notLocal')}</span>
        {item.pending && <span className="chip">{t('servers.proxy.setup.nextStart')}</span>}
      </div>
      {item.advice !== 'bedrock' && (
        <div className="sv-proxy-checks">
          <Check ok={item.onlineModeOff} label={t('servers.proxy.setup.onlineMode')} />
          <Check ok={item.forwardingOk} label={forwardingLabel} />
        </div>
      )}
      <span className="hint">{t(`servers.proxy.advice.${item.advice}` as Key)}</span>
      {item.tooOld && <span className="hint warn">{t('servers.proxy.setup.tooOld')}</span>}
      {view.forwarding === 'bungeeguard' && <span className="hint">{t('servers.proxy.setup.bungeeguard')}</span>}
      <div className="row wrap">
        {item.canApply && item.localId && (
          <button className="btn sm" onClick={onApply}>
            <Wrench /> {t('servers.proxy.setup.apply')}
          </button>
        )}
        {secretNeeded && view.hasSecret && !item.canApply && (
          <button className="btn sm" onClick={onCopy}>
            <Copy /> {t('servers.proxy.setup.copySecret')}
          </button>
        )}
      </div>
    </div>
  )
}

export default function ServerProxy({ s }: { s: ServerInfo }) {
  const { toast } = useStore()
  const [view, setView] = useState<ProxyConfigView>()
  const [draft, setDraft] = useState<ProxyEdit>()
  const [failed, setFailed] = useState<string>()
  const [saving, setSaving] = useState(false)
  const [applying, setApplying] = useState<ProxyBackendSetup>()

  const show = useCallback((v: ProxyConfigView) => {
    setView(v)
    setDraft(editOf(v))
    setFailed(undefined)
  }, [])
  const dirty = useMemo(() => !!view && !!draft && JSON.stringify(editOf(view)) !== JSON.stringify(draft), [view, draft])
  const unsaved = useRef(false)
  unsaved.current = dirty
  // The checks follow the server starting and stopping; what's being typed stays.
  useEffect(() => {
    api.server.proxyConfig(s.id).then((v) => (unsaved.current ? setView(v) : show(v)), (e: Error) => setFailed(e.message))
  }, [s.id, s.state, show])

  if (failed) return <div className="hint warn">{failed}</div>
  if (!view || !draft) return <Loader2 className="spin" />

  const running = s.state !== 'stopped' && s.state !== 'crashed'
  const velocity = s.type === 'velocity'
  const waterdog = s.type === 'waterdogpe'
  const backends = draft.backends
  const firstJoin = backends.find((b) => b.join)
  const used = backends.map((b) => b.address)
  const free = view.choices.filter((c) => !used.includes(c.address))
  const patch = (p: Partial<ProxyEdit>) => setDraft({ ...draft, ...p })
  const setBackend = (i: number, p: Partial<ProxyEdit['backends'][number]>) => {
    const old = backends[i].name
    const renamed = p.name !== undefined && p.name !== old
    patch({
      backends: backends.map((b, n) => (n === i ? { ...b, ...p } : b)),
      forcedHosts: renamed ? draft.forcedHosts.map((h) => ({ ...h, servers: h.servers.map((x) => (x === old ? p.name! : x)) })) : draft.forcedHosts
    })
  }
  const move = (i: number, by: number) => {
    const next = [...backends]
    next.splice(i + by, 0, next.splice(i, 1)[0])
    patch({ backends: next })
  }
  const remove = (i: number) => {
    const gone = backends[i].name
    patch({
      backends: backends.filter((_, n) => n !== i),
      forcedHosts: draft.forcedHosts.map((h) => ({ ...h, servers: h.servers.filter((x) => x !== gone) })).filter((h) => h.servers.length > 0)
    })
  }
  const add = (label: string, address: string) =>
    patch({ backends: [...backends, { name: nameFor(label, backends.map((b) => b.name)), address, join: !backends.some((b) => b.join) }] })
  const setHost = (i: number, p: Partial<ProxyEdit['forcedHosts'][number]>) => patch({ forcedHosts: draft.forcedHosts.map((h, n) => (n === i ? { ...h, ...p } : h)) })

  const act = async (call: Promise<ProxyConfigView>, done: string) => {
    try {
      show(await call)
      toast(done, 'success')
    } catch (e) {
      toast((e as Error).message, 'error')
    }
  }
  const save = async () => {
    setSaving(true)
    await act(api.server.proxySave(s.id, draft), t('servers.proxy.saved'))
    setSaving(false)
  }
  const copySecret = async () => {
    try {
      await navigator.clipboard.writeText(await api.server.proxySecret(s.id))
      toast(t('servers.proxy.setup.secretCopied'), 'info')
    } catch (e) {
      toast((e as Error).message, 'error')
    }
  }

  const modes: { value: ProxyForwarding; label: string }[] = velocity
    ? [
        { value: 'none', label: t('servers.proxy.forwarding.none') },
        { value: 'legacy', label: t('servers.proxy.forwarding.legacy') },
        { value: 'modern', label: t('servers.proxy.forwarding.modern') }
      ]
    : [
        { value: 'none', label: t('servers.proxy.forwarding.none') },
        { value: 'legacy', label: t('servers.proxy.forwarding.on') }
      ]
  if (draft.forwarding === 'bungeeguard') modes.push({ value: 'bungeeguard', label: t('servers.proxy.forwarding.bungeeguard') })

  return (
    <div className="sv-proxy">
      {view.missing && <div className="hint warn">{t('servers.proxy.firstStart', { file: view.file })}</div>}
      {view.pending && running && <div className="hint warn">{t('servers.proxy.pendingRunning')}</div>}
      {!view.onlineMode && (
        <div className="hint warn">
          <TriangleAlert /> {t('servers.proxy.offlineProxy')}
        </div>
      )}

      <div className="settings-section">
        <h3>{t('servers.proxy.backends')}</h3>
        <div className="card">
          <div className="setting stack">
            <span className="hint">{t('servers.proxy.backendsHint', { file: view.file })}</span>
          </div>
          {backends.length === 0 && <div className="setting sv-proxy-empty">{t('servers.proxy.empty')}</div>}
          {backends.map((b, i) => (
            <div key={i} className="setting sv-proxy-row">
              <div className="sv-proxy-move">
                <button className="btn icon sm" disabled={i === 0} title={t('servers.proxy.up')} aria-label={t('servers.proxy.up')} onClick={() => move(i, -1)}>
                  <ArrowUp />
                </button>
                <button className="btn icon sm" disabled={i === backends.length - 1} title={t('servers.proxy.down')} aria-label={t('servers.proxy.down')} onClick={() => move(i, 1)}>
                  <ArrowDown />
                </button>
              </div>
              <input className="input" value={b.name} spellCheck={false} aria-label={t('servers.proxy.name')} placeholder={t('servers.proxy.name')} onChange={(e) => setBackend(i, { name: e.target.value })} />
              <input className="input mono" value={b.address} spellCheck={false} aria-label={t('servers.proxy.address')} placeholder="127.0.0.1:25566" onChange={(e) => setBackend(i, { address: e.target.value })} />
              <div className="sv-proxy-join">
                <Switch on={b.join} label={t('servers.proxy.join')} onChange={(join) => setBackend(i, { join })} />
                <span className="hint">{b === firstJoin ? t('servers.proxy.default') : b.join ? t('servers.proxy.fallback') : t('servers.proxy.joinOff')}</span>
              </div>
              <button className="btn icon sm" title={t('common.delete')} aria-label={t('common.delete')} onClick={() => remove(i)}>
                <Trash2 />
              </button>
            </div>
          ))}
          <div className="setting sv-proxy-add">
            <select
              className="select"
              value=""
              disabled={free.length === 0}
              aria-label={t('servers.proxy.fromThisPc')}
              onChange={(e) => {
                const c = free.find((x) => x.id === e.target.value)
                if (c) add(c.name, c.address)
              }}
            >
              <option value="">{t(free.length ? 'servers.proxy.fromThisPc' : 'servers.proxy.noOthers')}</option>
              {free.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name} ({c.address})
                </option>
              ))}
            </select>
            <button className="btn sm" onClick={() => add('server', '')}>
              <Plus /> {t('servers.proxy.typeAddress')}
            </button>
          </div>
        </div>
      </div>

      <div className="settings-section">
        <h3>{t('servers.proxy.forcedHosts')}</h3>
        <div className="card">
          <div className="setting stack">
            <span className="hint">{t('servers.proxy.forcedHint')}</span>
          </div>
          {draft.forcedHosts.map((h, i) => (
            <div key={i} className="setting sv-proxy-host">
              <input className="input mono" value={h.host} spellCheck={false} aria-label={t('servers.proxy.host')} placeholder="play.example.com" onChange={(e) => setHost(i, { host: e.target.value })} />
              <select className="select" value={h.servers[0] ?? ''} aria-label={t('servers.proxy.hostServer')} onChange={(e) => setHost(i, { servers: [e.target.value, ...h.servers.slice(1)] })}>
                {backends.map((b, n) => (
                  <option key={n} value={b.name}>
                    {b.name}
                  </option>
                ))}
              </select>
              {h.servers.length > 1 && <span className="hint">{t('servers.proxy.hostMore', { count: String(h.servers.length - 1) })}</span>}
              <button className="btn icon sm" title={t('common.delete')} aria-label={t('common.delete')} onClick={() => patch({ forcedHosts: draft.forcedHosts.filter((_, n) => n !== i) })}>
                <Trash2 />
              </button>
            </div>
          ))}
          <div className="setting sv-proxy-add">
            <button className="btn sm" disabled={backends.length === 0} onClick={() => patch({ forcedHosts: [...draft.forcedHosts, { host: '', servers: [backends[0].name] }] })}>
              <Plus /> {t('servers.proxy.addHost')}
            </button>
          </div>
        </div>
      </div>

      {!waterdog && (
        <div className="settings-section">
          <h3>{t('servers.proxy.forwarding')}</h3>
          <div className="card">
            <div className="setting stack">
              <Segmented value={draft.forwarding} options={modes} onChange={(forwarding) => patch({ forwarding })} />
              <span className="hint">{t(`servers.proxy.forwardingHint.${draft.forwarding}` as Key)}</span>
            </div>
          </div>
        </div>
      )}

      <div className="sv-proxy-save">
        <span className="hint">{t('servers.proxy.nextStart')}</span>
        <button className="btn" disabled={!dirty || saving} onClick={() => setDraft(editOf(view))}>
          {t('servers.proxy.discard')}
        </button>
        <button className="btn primary" disabled={!dirty || saving || view.missing} onClick={() => void save()}>
          {saving && <Loader2 className="spin" />} {t('common.save')}
        </button>
      </div>

      {view.setup.length > 0 && (
        <div className="settings-section">
          <h3>{t('servers.proxy.setup')}</h3>
          <div className="card">
            <div className="setting stack">
              <span className="hint warn">
                <TriangleAlert /> {t('servers.proxy.setup.warning')}
              </span>
              {dirty && <span className="hint">{t('servers.proxy.setup.saveFirst')}</span>}
            </div>
            {view.setup.map((item) => (
              <BackendCard key={item.name} item={item} view={view} onApply={() => (dirty ? toast(t('servers.proxy.setup.saveFirst'), 'info') : setApplying(item))} onCopy={() => void copySecret()} />
            ))}
          </div>
        </div>
      )}

      {applying?.localId && (
        <ConfirmModal
          title={t('servers.proxy.confirm.title', { name: applying.localName ?? applying.name })}
          body={
            <>
              <p>{t('servers.proxy.confirm.changes')}</p>
              <p>
                <b>{t('servers.proxy.confirm.warning')}</b>
              </p>
              {applying.exposed && <p>{t('servers.proxy.confirm.sharing')}</p>}
            </>
          }
          confirm={t('servers.proxy.confirm.apply')}
          onClose={() => setApplying(undefined)}
          onConfirm={() => act(api.server.proxyApply(s.id, applying.localId!), t('servers.proxy.setup.applied'))}
        />
      )}
    </div>
  )
}
