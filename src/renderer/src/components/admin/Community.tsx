// Admin → Community: what players post for Discover's ResinCraft source (relay community.rs, `/api/community-admin/…`;
// the relay dashboard's Community page has the same). New projects and new files of approved ones wait here until an
// admin approves or rejects them (with a reason the owner reads in their Inbox); approved ones can be taken down,
// trusted (their new files skip review) or deleted.
import { useCallback, useEffect, useState } from 'react'
import { Check, Download, Package, RotateCcw, Search, ShieldCheck, Trash2, X } from 'lucide-react'
import type { CommunityProject, CommunityQueue, CommunityVersion } from '@shared/types'
import { api } from '../../api'
import { useStore } from '../../store'
import { t, type Key } from '../../i18n'
import { Switch, ConfirmModal } from '../ui'
import { ago, AskModal, Loading, post, size, useAct, useAdmin } from './common'
import '../../community.css'

const KIND_KEYS: Record<string, Key> = {
  modpack: 'community.kind.modpack',
  mod: 'community.kind.mod',
  resourcepack: 'community.kind.resourcepack',
  shader: 'community.kind.shader'
}
const STATES: [string, Key][] = [
  ['', 'servers.admin.community.filter.all'],
  ['pending', 'community.state.pending'],
  ['approved', 'community.state.approved'],
  ['rejected', 'community.state.rejected'],
  ['draft', 'community.state.draft'],
  ['removed', 'community.state.removed']
]

type Ask = { title: string; label: string; confirm: string; run: (reason: string) => Promise<boolean> }

export function CommunityAdmin() {
  const { toast } = useStore()
  const queue = useAdmin<CommunityQueue>('/api/community-admin/queue', 60_000)
  const [filter, setFilter] = useState('')
  const [q, setQ] = useState('')
  const [search, setSearch] = useState('')
  const [list, setList] = useState<CommunityProject[]>()
  const [base, setBase] = useState('')
  const [ask, setAsk] = useState<Ask>()
  const [deleting, setDeleting] = useState<CommunityProject>()

  const loadList = useCallback(async () => {
    try {
      setList((await post<{ projects: CommunityProject[] }>('/api/community-admin/list', { state: filter, q: search })).projects)
    } catch (e) {
      toast((e as Error).message, 'error')
    }
  }, [filter, search, toast])
  useEffect(() => {
    void loadList()
  }, [loadList])
  useEffect(() => {
    void api.community.base().then(setBase, () => setBase(''))
  }, [])
  const reload = useCallback(async () => {
    await Promise.all([queue.reload(), loadList()])
  }, [queue, loadList])
  const act = useAct(reload)

  /** A file under review: a one-time link, opened in the browser (it downloads there). */
  const download = async (v: CommunityVersion) => {
    try {
      const r = await post<{ path: string; url: string | null }>('/api/community-admin/ticket', { version: v.id })
      await api.app.openExternal(r.url ?? `${base}${r.path}`)
    } catch (e) {
      toast((e as Error).message, 'error')
    }
  }
  const reject = (body: { project?: string; version?: string }, name: string) =>
    setAsk({
      title: t('servers.admin.community.rejectTitle', { name }),
      label: t('servers.admin.community.reason'),
      confirm: t('servers.admin.community.reject'),
      run: (reason) => act('/api/community-admin/review', { ...body, approve: false, reason }, 'servers.admin.community.rejected')
    })

  if (!queue.data) return <Loading error={queue.error} />
  const d = queue.data
  const icon = (p: { icon: string | null }) => (p.icon ? <img className="cp-icon" src={`${base}${p.icon}`} alt="" /> : <span className="cp-icon cp-noicon"><Package /></span>)
  const kind = (k: string) => t(KIND_KEYS[k] ?? 'community.kind.mod')

  const versionLine = (v: CommunityVersion, review?: string) => (
    <div className="cp-version" key={v.id}>
      <div className="grow cp-text">
        <div className="row">
          <b>{v.name}</b>
          <span className="hint ellipsis">
            {v.fileName} · {size(v.size)} · {v.mc.join(', ')}
            {v.loaders.length ? ` · ${v.loaders.join(', ')}` : ''} · {ago(v.created)}
          </span>
        </div>
        {v.changelog && <p className="cm-body hint">{v.changelog}</p>}
        <span className="hint mono ellipsis" title={v.sha256}>
          SHA-256 {v.sha256}
        </span>
      </div>
      <button className="btn sm" onClick={() => void download(v)} title={t('servers.admin.community.downloadHint')}>
        <Download /> {t('servers.admin.community.download')}
      </button>
      {review && (
        <>
          <button className="btn sm primary" onClick={() => void act('/api/community-admin/review', { version: v.id, approve: true }, 'servers.admin.community.approved')}>
            <Check /> {t('servers.admin.community.approve')}
          </button>
          <button className="btn sm danger" onClick={() => reject({ version: v.id }, `${review} ${v.name}`)}>
            <X /> {t('servers.admin.community.reject')}
          </button>
        </>
      )}
    </div>
  )

  return (
    <div className="admin-community">
      <p className="hint">{t('servers.admin.community.hint')}</p>
      {!d.on && <div className="callout">{t('servers.admin.community.off')}</div>}
      <label className="row">
        <Switch on={d.reviewUpdates} onChange={(on) => void act('/api/community-admin/settings', { reviewUpdates: on }, 'servers.admin.community.saved')} label={t('servers.admin.community.reviewUpdates')} />
        <span>{t('servers.admin.community.reviewUpdates')}</span>
      </label>

      <h3>{t('servers.admin.community.waiting', { count: d.waiting })}</h3>
      {d.waiting === 0 && <p className="hint">{t('servers.admin.community.nothingWaiting')}</p>}
      {d.projects.map((p) => (
        <div key={p.id} className="cm-card">
          <div className="row">
            {icon(p)}
            <div className="grow cp-text">
              <div className="row">
                <b>{p.title}</b>
                <span className="chip">{kind(p.kind)}</span>
              </div>
              <span className="hint">
                {t('servers.admin.community.by', { name: p.author })} · {ago(p.updated)}
              </span>
              <span>{p.summary}</span>
              {p.categories.length > 0 && <span className="hint">{p.categories.join(', ')}</span>}
            </div>
          </div>
          {p.body && (
            <details>
              <summary className="hint">{t('servers.admin.community.description')}</summary>
              <pre className="cm-body">{p.body}</pre>
            </details>
          )}
          {p.versions.map((v) => versionLine(v))}
          <div className="row">
            <span className="grow" />
            <button className="btn danger" onClick={() => reject({ project: p.id }, p.title)}>
              <X /> {t('servers.admin.community.reject')}
            </button>
            <button className="btn primary" onClick={() => void act('/api/community-admin/review', { project: p.id, approve: true }, 'servers.admin.community.approved')}>
              <Check /> {t('servers.admin.community.approveProject')}
            </button>
          </div>
        </div>
      ))}
      {d.updates.map((u) => (
        <div key={u.version.id} className="cm-card">
          <div className="row">
            <b>{u.project.title}</b>
            <span className="hint">
              {t('servers.admin.community.newVersion')} · {t('servers.admin.community.by', { name: u.project.author })}
            </span>
          </div>
          {versionLine(u.version, u.project.title)}
        </div>
      ))}

      <h3>{t('servers.admin.community.all')}</h3>
      <p className="hint">{t('servers.admin.community.counts', { approved: d.approved, rejected: d.rejected, removed: d.removed })}</p>
      <div className="toolbar">
        <select className="select" value={filter} onChange={(e) => setFilter(e.target.value)} aria-label={t('servers.admin.community.filter.label')}>
          {STATES.map(([v, k]) => (
            <option key={v} value={v}>
              {t(k)}
            </option>
          ))}
        </select>
        <form
          className="search grow"
          onSubmit={(e) => {
            e.preventDefault()
            setSearch(q)
          }}
        >
          <Search />
          <input className="input" value={q} onChange={(e) => setQ(e.target.value)} placeholder={t('servers.admin.community.search')} />
        </form>
      </div>
      {!list ? (
        <Loading />
      ) : list.length === 0 ? (
        <p className="hint">{t('servers.admin.community.none')}</p>
      ) : (
        <div className="cp-list">
          {list.map((p) => (
            <div key={p.id} className="cm-card">
              <div className="row wrap">
                {icon(p)}
                <div className="grow cp-text">
                  <div className="row">
                    <b className="ellipsis">{p.title}</b>
                    <span className={`chip ${p.state === 'approved' ? 'success' : p.state === 'rejected' || p.state === 'removed' ? 'danger' : p.state === 'pending' ? 'warn' : ''}`}>
                      {t(STATES.find(([s]) => s === p.state)?.[1] ?? 'community.state.draft')}
                    </span>
                  </div>
                  <span className="hint ellipsis">
                    {kind(p.kind)} · {t('servers.admin.community.by', { name: p.author })} · {t('community.files', { count: p.versions.length })} · {t('community.downloads', { count: p.downloads })} · {ago(p.updated)}
                  </span>
                  {p.reason && <span className="hint bad">{p.reason}</span>}
                </div>
                <select
                  className="select"
                  value={p.reviewUpdates == null ? '' : p.reviewUpdates ? 'review' : 'trust'}
                  onChange={(e) => void act('/api/community-admin/trust', { id: p.id, reviewUpdates: e.target.value === '' ? null : e.target.value === 'review' }, 'servers.admin.community.saved')}
                  aria-label={t('servers.admin.community.updates')}
                  title={t('servers.admin.community.updates')}
                >
                  <option value="">{t(d.reviewUpdates ? 'servers.admin.community.updatesDefaultReview' : 'servers.admin.community.updatesDefaultTrust')}</option>
                  <option value="review">{t('servers.admin.community.updatesReview')}</option>
                  <option value="trust">{t('servers.admin.community.updatesTrust')}</option>
                </select>
                {p.state === 'removed' ? (
                  <button className="btn sm" onClick={() => void act('/api/community-admin/restore', { id: p.id }, 'servers.admin.community.restored')}>
                    <RotateCcw /> {t('servers.admin.community.restore')}
                  </button>
                ) : (
                  p.state === 'approved' && (
                    <button
                      className="btn sm"
                      onClick={() =>
                        setAsk({
                          title: t('servers.admin.community.removeTitle', { name: p.title }),
                          label: t('servers.admin.community.reason'),
                          confirm: t('servers.admin.community.remove'),
                          run: (reason) => act('/api/community-admin/remove', { id: p.id, reason }, 'servers.admin.community.removed')
                        })
                      }
                    >
                      <ShieldCheck /> {t('servers.admin.community.remove')}
                    </button>
                  )
                )}
                <button className="btn sm ghost danger" onClick={() => setDeleting(p)} aria-label={t('servers.admin.community.delete')} title={t('servers.admin.community.delete')}>
                  <Trash2 />
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
      {ask && (
        <AskModal
          title={ask.title}
          fields={[{ key: 'reason', label: ask.label, type: 'textarea', hint: t('servers.admin.community.reasonHint') }]}
          confirm={ask.confirm}
          danger
          onSubmit={(v) => ask.run(v.reason.trim())}
          onClose={() => setAsk(undefined)}
        />
      )}
      {deleting && (
        <ConfirmModal
          title={t('servers.admin.community.deleteTitle', { name: deleting.title })}
          body={t('servers.admin.community.deleteBody')}
          confirm={t('servers.admin.community.delete')}
          onConfirm={() => void act('/api/community-admin/delete', { id: deleting.id }, 'servers.admin.community.deleted')}
          onClose={() => setDeleting(undefined)}
        />
      )}
    </div>
  )
}
