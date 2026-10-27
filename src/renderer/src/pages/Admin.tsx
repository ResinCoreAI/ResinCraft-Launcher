// The Admin page: ResinCraft Relay's server-list tools, for accounts the relay lists as admins (relay protocol 8).
// Unlocked with the relay dashboard's password; the relay checks every request, this page is only its UI.
import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react'
import { Ban, Check, ExternalLink, EyeOff, Eye, Flag, Loader2, Lock, Pin, PinOff, Plus, RefreshCw, Search, Shield, Star, Trash2, X } from 'lucide-react'
import type { AdminStatus, ListingInfo } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { t, type Key } from '../i18n'
import { Empty, Modal, MoreButton, Segmented, Switch, type ContextEntry } from '../components/ui'
import { ListingFields, stateChip } from '../components/ServerList'
import '../serverlist.css'

type Promo = { kind: 'recommended' | 'sponsored'; until?: number | null; order?: number; since?: number; active?: boolean }

interface AdminItem {
  id: string
  title: string
  description: string
  tags: string[]
  address: string
  kind: 'resincraft' | 'submitted' | 'admin'
  edition: 'java' | 'bedrock'
  version: string
  modded: boolean
  modpack?: { name: string } | null
  pack?: { size: number } | null
  pinned: boolean
  promo?: Promo | null
  owner: string
  ownerName?: string
  ownerBanned?: boolean
  state: 'listed' | 'pending' | 'rejected' | 'hidden' | 'off' | 'unknown'
  message?: string | null
  likes: number
  reports: number
  online: boolean
  players: number
  maxPlayers: number
}

interface AdminSettings {
  enabled: boolean
  resincraft_approval: 'auto' | 'manual'
  submitted_approval: 'auto' | 'manual'
  max_submissions: number
  report_hide_threshold: number
  max_pack_mb: number
  pack_downloads_per_hour: number
}

interface AdminData {
  settings: AdminSettings
  settingsOverridden: boolean
  counts: Record<string, number>
  pending: AdminItem[]
  featured: AdminItem[]
  listings: AdminItem[]
  bans: { uuid: string; name?: string; reason: string; at: number; until?: number | null }[]
  reports: { id: string; listing: string; title: string; address: string; reporterName?: string; reason: string; at: number }[]
}

type Tab = 'relay' | 'pending' | 'listings' | 'reports' | 'bans' | 'settings' | 'add'

const when = (secs?: number | null) => (secs ? new Date(secs * 1000).toLocaleString() : '')

export default function Admin() {
  const { toast } = useStore()
  const [status, setStatus] = useState<AdminStatus>()
  const [opening, setOpening] = useState(false)
  const [password, setPassword] = useState('')
  const [unlocking, setUnlocking] = useState(false)
  const [error, setError] = useState<string>()
  const [data, setData] = useState<AdminData>()
  const [tab, setTab] = useState<Tab>('relay')
  const [code, setCode] = useState('')
  const [needCode, setNeedCode] = useState(false)
  const [remember, setRemember] = useState(true)
  const [q, setQ] = useState('')
  const [loading, setLoading] = useState(false)

  useEffect(() => {
    api.server.adminStatus().then(setStatus, () => setStatus({ admin: false, unlocked: false }))
  }, [])

  const load = useCallback(async (query = q) => {
    setLoading(true)
    try {
      setData((await api.server.adminCall('GET', `/api/listings${query.trim() ? `?q=${encodeURIComponent(query.trim())}` : ''}`)) as AdminData)
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setLoading(false)
    }
  }, [q, toast])
  useEffect(() => {
    if (status?.unlocked) void load('')
  }, [status?.unlocked]) // eslint-disable-line react-hooks/exhaustive-deps

  /** One action; reloads the lists after it. */
  const act = async (route: string, body: unknown, done?: Key) => {
    try {
      await api.server.adminCall('POST', `/api/listings/${route}`, body)
      if (done) toast(t(done), 'success')
      await load()
      return true
    } catch (e) {
      toast((e as Error).message, 'error')
      return false
    }
  }

  const unlock = async () => {
    setUnlocking(true)
    setError(undefined)
    try {
      const st = await api.server.adminUnlock({ password, code: needCode ? code : undefined, remember })
      if (st.needCode) {
        setNeedCode(true)
      } else {
        setStatus(st)
        setPassword('')
        setCode('')
        setNeedCode(false)
      }
    } catch (e) {
      setError((e as Error).message)
    } finally {
      setUnlocking(false)
    }
  }

  if (!status) return null
  if (!status.admin) {
    return (
      <div className="page">
        <Empty icon={<Shield />} title={t('servers.admin.notAdmin')}>
          {t('servers.admin.notAdminHint')}
        </Empty>
      </div>
    )
  }
  if (!status.unlocked) {
    return (
      <div className="page">
        <div className="page-head">
          <div>
            <h1>{t('servers.admin.title')}</h1>
            <p>{t('servers.admin.subtitle')}</p>
          </div>
        </div>
        <form
          className="card admin-unlock"
          onSubmit={(e) => {
            e.preventDefault()
            void unlock()
          }}
        >
          <Lock />
          <label className="field">
            <span>{t('servers.admin.password')}</span>
            <input className="input" type="password" autoFocus autoComplete="current-password" value={password} onChange={(e) => setPassword(e.target.value)} />
          </label>
          <span className="hint">{t('servers.admin.passwordHint', { name: status.username ?? '' })}</span>
          {needCode && (
            <label className="field">
              <span>{t('servers.admin.code')}</span>
              <input
                className="input"
                inputMode="numeric"
                autoComplete="one-time-code"
                autoFocus
                maxLength={12}
                value={code}
                placeholder="123456"
                onChange={(e) => setCode(e.target.value.replace(/[^0-9a-zA-Z-]/g, ''))}
              />
              <span className="hint">{t('servers.admin.codeHint')}</span>
            </label>
          )}
          <label className="inline-switch">
            <Switch on={remember} onChange={setRemember} label={t('servers.admin.remember')} />
            {t('servers.admin.remember')}
          </label>
          <span className="hint">{t('servers.admin.rememberHint')}</span>
          {error && (
            <span className="hint bad" role="alert">
              {error}
            </span>
          )}
          <button className="btn primary" disabled={(!password && !needCode) || (needCode && !code) || unlocking}>
            {unlocking && <Loader2 className="spin" />} {t('servers.admin.unlock')}
          </button>
        </form>
      </div>
    )
  }

  const tabs: { value: Tab; label: ReactNode }[] = [
    { value: 'relay', label: t('servers.admin.tab.relay') },
    { value: 'pending', label: <>{t('servers.admin.tab.pending')} {data?.pending.length ? <span className="count">{data.pending.length}</span> : null}</> },
    { value: 'listings', label: t('servers.admin.tab.listings') },
    { value: 'reports', label: <>{t('servers.admin.tab.reports')} {data?.reports.length ? <span className="count">{data.reports.length}</span> : null}</> },
    { value: 'bans', label: t('servers.admin.tab.bans') },
    { value: 'settings', label: t('servers.admin.tab.settings') },
    { value: 'add', label: t('servers.admin.tab.add') }
  ]

  return (
    <div className="page admin-page">
      <div className="page-head">
        <div>
          <h1>{t('servers.admin.title')}</h1>
          <p>
            {data
              ? t('servers.admin.counts', { listed: data.counts.listed ?? 0, pending: data.counts.pending ?? 0, hidden: data.counts.hidden ?? 0 })
              : t('servers.admin.subtitle')}
          </p>
        </div>
        <div className="row">
          {status.dashboardUser && <span className="hint">{t('servers.admin.signedInAs', { user: status.dashboardUser })}</span>}
          <button
            className="btn sm primary"
            disabled={opening}
            title={t('servers.admin.dashboardHint')}
            onClick={async () => {
              setOpening(true)
              try {
                await api.server.adminDashboard()
              } catch (e) {
                toast((e as Error).message, 'error')
              } finally {
                setOpening(false)
              }
            }}
          >
            {opening ? <Loader2 className="spin" /> : <ExternalLink />} {t('servers.admin.dashboard')}
          </button>
          <button className="btn sm" disabled={loading} onClick={() => void load()}>
            {loading ? <Loader2 className="spin" /> : <RefreshCw />} {t('common.refresh')}
          </button>
          <button
            className="btn sm"
            onClick={() =>
              void api.server.adminLock(false).then(() => {
                setStatus({ ...status, unlocked: false })
                setData(undefined)
              })
            }
          >
            <Lock /> {t('servers.admin.lock')}
          </button>
          {status.remembered && (
            <button
              className="btn sm ghost"
              title={t('servers.admin.forgetHint')}
              onClick={() =>
                void api.server.adminLock(true).then(() => {
                  setStatus({ ...status, unlocked: false, remembered: false })
                  setData(undefined)
                })
              }
            >
              {t('servers.admin.forget')}
            </button>
          )}
        </div>
      </div>
      <div className="toolbar">
        <Segmented<Tab> value={tab} onChange={setTab} options={tabs} />
      </div>
      {tab === 'relay' ? (
        <RelayFrame />
      ) : !data ? (
        <Loader2 className="spin" />
      ) : tab === 'pending' ? (
        <ItemList items={data.pending} empty={t('servers.admin.pendingEmpty')} act={act} pendingActions />
      ) : tab === 'listings' ? (
        <>
          {data.featured.length > 0 && (
            <section className="featured-servers">
              <h2>
                <Star /> {t('servers.admin.featuredNow')}
              </h2>
              <ItemList items={data.featured} empty="" act={act} />
            </section>
          )}
          <div className="toolbar">
            <form
              className="search"
              onSubmit={(e) => {
                e.preventDefault()
                void load(q)
              }}
            >
              <Search />
              <input className="input" placeholder={t('servers.admin.search')} value={q} onChange={(e) => setQ(e.target.value)} />
            </form>
          </div>
          <ItemList items={data.listings} empty={t('servers.list.empty')} act={act} />
        </>
      ) : tab === 'reports' ? (
        <Reports data={data} act={act} />
      ) : tab === 'bans' ? (
        <Bans data={data} act={act} />
      ) : tab === 'settings' ? (
        <AdminSettingsForm data={data} act={act} />
      ) : (
        <AddServer act={act} done={() => setTab('listings')} />
      )}
    </div>
  )
}

/** The relay's own dashboard inside the page, signed in by a one-time link; when its session runs out it asks the
 *  launcher (postMessage) for a fresh link instead of showing its sign-in page. */
function RelayFrame() {
  const frame = useRef<HTMLIFrameElement>(null)
  const [src, setSrc] = useState<string>()
  const [error, setError] = useState<string>()
  useEffect(() => {
    api.server.adminDashboardLink().then(
      (url) => setSrc(`${url}${url.includes('?') ? '&' : '?'}embed=1`),
      (e) => setError((e as Error).message)
    )
  }, [])
  useEffect(() => {
    const onMessage = async (e: MessageEvent) => {
      const data = e.data as { type?: string; need?: string } | null
      if (e.source !== frame.current?.contentWindow || data?.type !== 'resincraft-relay' || data.need !== 'ticket') return
      try {
        const url = await api.server.adminDashboardLink()
        frame.current?.contentWindow?.postMessage({ type: 'resincraft-relay', ticket: url }, new URL(url).origin)
      } catch (err) {
        setError((err as Error).message)
      }
    }
    window.addEventListener('message', onMessage)
    return () => window.removeEventListener('message', onMessage)
  }, [])
  if (error) return <p className="hint bad">{error}</p>
  if (!src) return <Loader2 className="spin" />
  return <iframe ref={frame} className="relay-frame" src={src} title={t('servers.admin.tab.relay')} referrerPolicy="no-referrer" />
}

type Act = (route: string, body: unknown, done?: Key) => Promise<boolean>

function ItemList({ items, empty, act, pendingActions }: { items: AdminItem[]; empty: string; act: Act; pendingActions?: boolean }) {
  const [asking, setAsking] = useState<{ item: AdminItem; what: 'reject' | 'hide' | 'ban' | 'promo' | 'remove' }>()
  if (items.length === 0) return empty ? <p className="hint">{empty}</p> : null
  return (
    <div className="card admin-items">
      {items.map((it) => (
        <div key={it.id} className="setting admin-item">
          <div className="text">
            <b>
              {it.title}{' '}
              <span className={`chip compact ${stateChip[it.state]}`}>{t(`servers.list.state.${it.state}` as Key)}</span>
              {it.pinned && (
                <span className="chip compact">
                  <Pin /> {t('servers.list.pinned')}
                </span>
              )}
              {it.promo?.active && (
                <span className={`chip compact ${it.promo.kind === 'sponsored' ? 'warn' : 'success'}`}>
                  {t(it.promo.kind === 'sponsored' ? 'servers.list.sponsored' : 'servers.list.recommended')}
                </span>
              )}
              {it.ownerBanned && <span className="chip compact danger">{t('servers.admin.ownerBanned')}</span>}
            </b>
            <span>
              {it.address} · {t(`servers.admin.kind.${it.kind}` as Key)}
              {it.ownerName ? ` · ${it.ownerName}` : ''} · {it.online ? t('servers.list.players', { count: it.players, max: it.maxPlayers }) : t('servers.list.offline')} ·{' '}
              ♥ {it.likes}
              {it.reports ? ` · ${t('servers.admin.reportCount', { count: it.reports })}` : ''}
              {it.modpack ? ` · ${it.modpack.name}` : ''}
            </span>
            {it.description && <span className="hint admin-desc">{it.description}</span>}
            {it.message && <span className="hint">{it.message}</span>}
          </div>
          <div className="row">
            {pendingActions && (
              <>
                <button className="btn sm primary" onClick={() => void act('approve', { id: it.id }, 'servers.admin.approved')}>
                  <Check /> {t('servers.admin.approve')}
                </button>
                <button className="btn sm" onClick={() => setAsking({ item: it, what: 'reject' })}>
                  <X /> {t('servers.admin.reject')}
                </button>
              </>
            )}
            <MoreButton
              items={(): ContextEntry[] => [
                it.pinned
                  ? { label: t('servers.admin.unpin'), icon: <PinOff />, onSelect: () => void act('unpin', { id: it.id }) }
                  : { label: t('servers.admin.pin'), icon: <Pin />, onSelect: () => void act('pin', { id: it.id }, 'servers.admin.pinned') },
                { label: t('servers.admin.promote'), icon: <Star />, onSelect: () => setAsking({ item: it, what: 'promo' }) },
                !!it.promo && { label: t('servers.admin.clearPromo'), icon: <X />, onSelect: () => void act('promo', { id: it.id, promo: null }) },
                '-',
                it.state === 'hidden'
                  ? { label: t('servers.admin.unhide'), icon: <Eye />, onSelect: () => void act('unhide', { id: it.id }) }
                  : { label: t('servers.admin.hide'), icon: <EyeOff />, onSelect: () => setAsking({ item: it, what: 'hide' }) },
                !!it.pack && { label: t('servers.admin.removePack'), icon: <Trash2 />, onSelect: () => void act('pack/remove', { id: it.id }) },
                !!it.owner && !it.ownerBanned && { label: t('servers.admin.banOwner'), icon: <Ban />, danger: true, onSelect: () => setAsking({ item: it, what: 'ban' }) },
                { label: t('servers.admin.remove'), icon: <Trash2 />, danger: true, onSelect: () => setAsking({ item: it, what: 'remove' }) }
              ]}
            />
          </div>
        </div>
      ))}
      {asking && <Ask item={asking.item} what={asking.what} act={act} onClose={() => setAsking(undefined)} />}
    </div>
  )
}

/** The dialogs behind an item's actions: a reason, a ban's length, a promotion's kind and end. */
function Ask({ item, what, act, onClose }: { item: AdminItem; what: 'reject' | 'hide' | 'ban' | 'promo' | 'remove'; act: Act; onClose: () => void }) {
  const [reason, setReason] = useState('')
  const [hours, setHours] = useState('')
  const [kind, setKind] = useState<Promo['kind']>(item.promo?.kind ?? 'recommended')
  const [days, setDays] = useState('')
  const [order, setOrder] = useState(String(item.promo?.order ?? 0))
  const [busy, setBusy] = useState(false)
  const go = async () => {
    setBusy(true)
    const days_ = Number(days)
    const ok =
      what === 'reject'
        ? await act('reject', { id: item.id, reason }, 'servers.admin.rejected')
        : what === 'hide'
        ? await act('hide', { id: item.id, reason }, 'servers.admin.hidden')
        : what === 'ban'
        ? await act('ban', { id: item.id, reason, hours: Number(hours) > 0 ? Number(hours) : undefined }, 'servers.admin.banned')
        : what === 'remove'
        ? await act('remove', { id: item.id }, 'servers.admin.removed')
        : await act(
            'promo',
            { id: item.id, promo: { kind, order: Number(order) || 0, until: days_ > 0 ? Math.floor(Date.now() / 1000 + days_ * 86400) : null } },
            'servers.admin.promoted'
          )
    setBusy(false)
    if (ok) onClose()
  }
  const title = t(`servers.admin.ask.${what}` as Key, { name: item.title })
  return (
    <Modal
      title={title}
      onClose={onClose}
      footer={
        <>
          <button className="btn" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className={`btn ${what === 'ban' || what === 'remove' ? 'danger' : 'primary'}`} disabled={busy} onClick={() => void go()}>
            {busy && <Loader2 className="spin" />} {t(`servers.admin.confirm.${what}` as Key)}
          </button>
        </>
      }
    >
      {(what === 'reject' || what === 'hide' || what === 'ban') && (
        <label className="field">
          <span>{t('servers.admin.reason')}</span>
          <textarea className="input" rows={3} maxLength={500} autoFocus value={reason} onChange={(e) => setReason(e.target.value)} />
        </label>
      )}
      {what === 'ban' && (
        <>
          <label className="field">
            <span>{t('servers.admin.banHours')}</span>
            <input className="input" type="number" min={0} value={hours} placeholder={t('servers.admin.forever')} onChange={(e) => setHours(e.target.value)} />
          </label>
          <p className="hint">{t('servers.admin.banHint', { name: item.ownerName ?? item.owner })}</p>
        </>
      )}
      {what === 'promo' && (
        <div className="listing-fields">
          <Segmented<Promo['kind']>
            value={kind}
            onChange={setKind}
            options={[
              { value: 'recommended', label: t('servers.list.recommended') },
              { value: 'sponsored', label: t('servers.list.sponsored') }
            ]}
          />
          <p className="hint">{t(kind === 'sponsored' ? 'servers.admin.sponsoredHint' : 'servers.admin.recommendedHint')}</p>
          <div className="field-row">
            <label className="field">
              <span>{t('servers.admin.promoDays')}</span>
              <input className="input" type="number" min={0} value={days} placeholder={t('servers.admin.forever')} onChange={(e) => setDays(e.target.value)} />
            </label>
            <label className="field">
              <span>{t('servers.admin.promoOrder')}</span>
              <input className="input" type="number" value={order} onChange={(e) => setOrder(e.target.value)} />
            </label>
          </div>
        </div>
      )}
      {what === 'remove' && <p>{t('servers.admin.removeBody', { name: item.title })}</p>}
    </Modal>
  )
}

function Reports({ data, act }: { data: AdminData; act: Act }) {
  if (data.reports.length === 0) return <p className="hint">{t('servers.admin.reportsEmpty')}</p>
  return (
    <div className="card admin-items">
      {data.reports.map((r) => (
        <div key={r.id} className="setting">
          <div className="text">
            <b>
              <Flag className="inline" /> {r.title} <span className="hint">{r.address}</span>
            </b>
            <span>{r.reason}</span>
            <span className="hint">
              {r.reporterName ?? ''} · {when(r.at)}
            </span>
          </div>
          <div className="row">
            <button className="btn sm" onClick={() => void act('reports/dismiss', { id: r.id }, 'servers.admin.dismissed')}>
              {t('servers.admin.dismiss')}
            </button>
            <button className="btn sm danger" onClick={() => void act('reports/hide', { id: r.id }, 'servers.admin.hidden')}>
              <EyeOff /> {t('servers.admin.hideListing')}
            </button>
          </div>
        </div>
      ))}
    </div>
  )
}

function Bans({ data, act }: { data: AdminData; act: Act }) {
  const [who, setWho] = useState('')
  const [reason, setReason] = useState('')
  const [hours, setHours] = useState('')
  const ban = async () => {
    const id = who.trim()
    const key = /^[0-9a-f-]{32,36}$/i.test(id) ? { uuid: id } : { name: id }
    if (await act('ban', { ...key, reason, hours: Number(hours) > 0 ? Number(hours) : undefined }, 'servers.admin.banned')) {
      setWho('')
      setReason('')
      setHours('')
    }
  }
  return (
    <>
      <div className="card">
        <div className="setting stack">
          <div className="text">
            <b>{t('servers.admin.banSomeone')}</b>
            <span>{t('servers.admin.banSomeoneHint')}</span>
          </div>
          <div className="listing-fields">
            <div className="field-row">
              <label className="field">
                <span>{t('servers.admin.banWho')}</span>
                <input className="input" value={who} spellCheck={false} onChange={(e) => setWho(e.target.value)} />
              </label>
              <label className="field">
                <span>{t('servers.admin.banHours')}</span>
                <input className="input" type="number" min={0} value={hours} placeholder={t('servers.admin.forever')} onChange={(e) => setHours(e.target.value)} />
              </label>
            </div>
            <label className="field">
              <span>{t('servers.admin.reason')}</span>
              <input className="input" value={reason} maxLength={500} onChange={(e) => setReason(e.target.value)} />
            </label>
            <div className="row end">
              <button className="btn sm danger" disabled={!who.trim()} onClick={() => void ban()}>
                <Ban /> {t('servers.admin.confirm.ban')}
              </button>
            </div>
          </div>
        </div>
      </div>
      {data.bans.length === 0 ? (
        <p className="hint">{t('servers.admin.bansEmpty')}</p>
      ) : (
        <div className="card admin-items">
          {data.bans.map((b) => (
            <div key={b.uuid} className="setting">
              <div className="text">
                <b>{b.name || b.uuid}</b>
                <span>{b.reason || '—'}</span>
                <span className="hint">{b.until ? t('servers.admin.banUntil', { when: when(b.until) }) : t('servers.admin.forever')}</span>
              </div>
              <button className="btn sm" onClick={() => void act('unban', { uuid: b.uuid }, 'servers.admin.unbanned')}>
                {t('servers.admin.unban')}
              </button>
            </div>
          ))}
        </div>
      )}
    </>
  )
}

function AdminSettingsForm({ data, act }: { data: AdminData; act: Act }) {
  const [s, setS] = useState<AdminSettings>(data.settings)
  useEffect(() => setS(data.settings), [data.settings])
  const dirty = JSON.stringify(s) !== JSON.stringify(data.settings)
  const num = (k: keyof AdminSettings) => (
    <input className="input" type="number" min={0} style={{ width: 120 }} value={String(s[k])} onChange={(e) => setS({ ...s, [k]: Math.max(0, Math.round(Number(e.target.value) || 0)) })} />
  )
  const approval = (k: 'resincraft_approval' | 'submitted_approval') => (
    <Segmented<'auto' | 'manual'>
      value={s[k]}
      onChange={(v) => setS({ ...s, [k]: v })}
      options={[
        { value: 'auto', label: t('servers.admin.approval.auto') },
        { value: 'manual', label: t('servers.admin.approval.manual') }
      ]}
    />
  )
  const row = (label: Key, hint: Key, control: ReactNode) => (
    <div className="setting">
      <div className="text">
        <b>{t(label)}</b>
        <span>{t(hint)}</span>
      </div>
      {control}
    </div>
  )
  return (
    <div className="card">
      {row('servers.admin.set.enabled', 'servers.admin.set.enabledHint', <Switch on={s.enabled} onChange={(v) => setS({ ...s, enabled: v })} />)}
      {row('servers.admin.set.resincraft', 'servers.admin.set.resincraftHint', approval('resincraft_approval'))}
      {row('servers.admin.set.submitted', 'servers.admin.set.submittedHint', approval('submitted_approval'))}
      {row('servers.admin.set.maxSubmissions', 'servers.admin.set.maxSubmissionsHint', num('max_submissions'))}
      {row('servers.admin.set.reportThreshold', 'servers.admin.set.reportThresholdHint', num('report_hide_threshold'))}
      {row('servers.admin.set.maxPack', 'servers.admin.set.maxPackHint', num('max_pack_mb'))}
      {row('servers.admin.set.packDownloads', 'servers.admin.set.packDownloadsHint', num('pack_downloads_per_hour'))}
      <div className="setting">
        <span className="hint grow">{data.settingsOverridden ? t('servers.admin.set.overridden') : t('servers.admin.set.fromFile')}</span>
        <div className="row">
          {data.settingsOverridden && (
            <button className="btn sm" onClick={() => void act('settings/reset', {}, 'servers.admin.set.saved')}>
              {t('servers.admin.set.reset')}
            </button>
          )}
          <button className="btn sm primary" disabled={!dirty} onClick={() => void act('settings', { settings: s }, 'servers.admin.set.saved')}>
            {t('servers.admin.set.save')}
          </button>
        </div>
      </div>
    </div>
  )
}

function AddServer({ act, done }: { act: Act; done: () => void }) {
  const [address, setAddress] = useState('')
  const [info, setInfo] = useState<ListingInfo>({ title: '', description: '', tags: [], region: '', language: '', version: '', edition: 'java', modded: false })
  const [pinned, setPinned] = useState(false)
  const [promo, setPromo] = useState<'' | Promo['kind']>('')
  const [busy, setBusy] = useState(false)
  const add = async () => {
    setBusy(true)
    const ok = await act('add', { address: address.trim(), listing: info, pinned, promo: promo ? { kind: promo } : undefined }, 'servers.admin.added')
    setBusy(false)
    if (ok) done()
  }
  return (
    <div className="card">
      <div className="setting stack">
        <div className="text">
          <b>{t('servers.admin.addTitle')}</b>
          <span>{t('servers.admin.addHint')}</span>
        </div>
        <label className="field">
          <span>{t('servers.list.field.address')}</span>
          <input className="input" value={address} placeholder="play.example.com" spellCheck={false} onChange={(e) => setAddress(e.target.value)} />
        </label>
        <ListingFields info={info} onChange={setInfo} showEdition />
        <div className="setting">
          <div className="text">
            <b>{t('servers.admin.pin')}</b>
          </div>
          <Switch on={pinned} onChange={setPinned} />
        </div>
        <div className="setting">
          <div className="text">
            <b>{t('servers.admin.promote')}</b>
            <span>{t(promo === 'sponsored' ? 'servers.admin.sponsoredHint' : 'servers.admin.recommendedHint')}</span>
          </div>
          <Segmented<'' | Promo['kind']>
            value={promo}
            onChange={setPromo}
            options={[
              { value: '', label: t('servers.admin.noPromo') },
              { value: 'recommended', label: t('servers.list.recommended') },
              { value: 'sponsored', label: t('servers.list.sponsored') }
            ]}
          />
        </div>
        <div className="row end">
          <button className="btn primary" disabled={busy || !address.trim() || !info.title.trim()} onClick={() => void add()}>
            {busy ? <Loader2 className="spin" /> : <Plus />} {t('servers.admin.addSend')}
          </button>
        </div>
      </div>
    </div>
  )
}
