// Discover → My projects: what the player posts to ResinCraft Relay for Discover's ResinCraft source (relay
// community.rs). A project starts as a draft, gets its files (from this PC, or a modpack straight from one of the
// player's instances, packed as a .resinpack) and a picture, and goes to the admins for review; nothing shows in
// Discover before they approve it, and the answer comes in the Inbox (Team).
import { useEffect, useState } from 'react'
import { ArrowLeft, FileUp, ImagePlus, Loader2, Package, Plus, Send, Trash2, Upload } from 'lucide-react'
import type { CommunityKind, CommunityMine, CommunityProject, CommunityVersion } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { t, type Key } from '../i18n'
import { compact, timeAgo } from '../util'
import { ConfirmModal, Modal } from './ui'
import '../community.css'

const KINDS: { value: CommunityKind; key: Key }[] = [
  { value: 'modpack', key: 'community.kind.modpack' },
  { value: 'mod', key: 'community.kind.mod' },
  { value: 'resourcepack', key: 'community.kind.resourcepack' },
  { value: 'shader', key: 'community.kind.shader' }
]
const MOD_LOADERS: [string, string][] = [
  ['fabric', 'Fabric'],
  ['quilt', 'Quilt'],
  ['forge', 'Forge'],
  ['neoforge', 'NeoForge'],
  ['liteloader', 'LiteLoader']
]
const SHADER_LOADERS: [string, string][] = [
  ['iris', 'Iris'],
  ['optifine', 'OptiFine'],
  ['canvas', 'Canvas']
]
const MAX_CATEGORIES = 5

const kindLabel = (k: CommunityKind) => t(KINDS.find((x) => x.value === k)?.key ?? 'community.kind.mod')
const size = (b: number) => (b >= 1e9 ? `${(b / 1e9).toFixed(1)} GB` : b >= 1e6 ? `${(b / 1e6).toFixed(1)} MB` : `${Math.max(1, Math.round(b / 1e3))} KB`)

/** A project's or file's state, as a chip. */
function StateChip({ state }: { state?: string }) {
  const [cls, key]: [string, Key] =
    state === 'approved'
      ? ['success', 'community.state.approved']
      : state === 'pending'
        ? ['warn', 'community.state.pending']
        : state === 'rejected'
          ? ['danger', 'community.state.rejected']
          : state === 'removed'
            ? ['danger', 'community.state.removed']
            : ['', 'community.state.draft']
  return <span className={`chip ${cls}`}>{t(key)}</span>
}

export function CommunityProjects({
  onClose,
  categoryOptions,
  categoryLabel
}: {
  onClose: () => void
  /** Discover's Category filter choices by kind, and their names. */
  categoryOptions: Partial<Record<string, string[]>>
  categoryLabel: (c: string) => string
}) {
  const { toast } = useStore()
  const [data, setData] = useState<CommunityMine>()
  const [error, setError] = useState<string>()
  /** The project being edited: its id, or "new". */
  const [open, setOpen] = useState<string | null>(null)

  const load = async () => {
    try {
      setData(await api.community.mine())
      setError(undefined)
    } catch (e) {
      setError((e as Error).message)
    }
  }
  useEffect(() => {
    void load()
  }, [])

  const project = open && open !== 'new' ? data?.projects.find((p) => p.id === open) : undefined
  return (
    <Modal title={t('community.mine.title')} subtitle={t('community.mine.subtitle')} onClose={onClose} wide>
      {error && <div className="callout danger">{error}</div>}
      {!data && !error && <div className="skeleton" style={{ height: 120 }} />}
      {data && !data.on && <div className="callout">{t('community.mine.off')}</div>}
      {data && open ? (
        <ProjectEditor
          key={open}
          project={project}
          maxFile={data.maxFile}
          categoryOptions={categoryOptions}
          categoryLabel={categoryLabel}
          onBack={() => setOpen(null)}
          onChanged={async (id) => {
            await load()
            if (id) setOpen(id)
          }}
          onDeleted={async () => {
            setOpen(null)
            await load()
            toast(t('community.deleted'), 'success')
          }}
        />
      ) : (
        data && (
          <>
            <div className="row">
              <span className="hint grow">{t('community.mine.how')}</span>
              <button className="btn primary" disabled={!data.on} onClick={() => setOpen('new')}>
                <Plus /> {t('community.mine.new')}
              </button>
            </div>
            {data.projects.length === 0 ? (
              <div className="hint cp-empty">{t('community.mine.none')}</div>
            ) : (
              <div className="cp-list">
                {data.projects.map((p) => (
                  <button key={p.id} className="cp-row" onClick={() => setOpen(p.id)}>
                    {p.icon ? <img className="cp-icon" src={p.icon} alt="" /> : <span className="cp-icon cp-noicon"><Package /></span>}
                    <span className="grow cp-text">
                      <b className="ellipsis">{p.title}</b>
                      <span className="hint ellipsis">
                        {kindLabel(p.kind)} · {t('community.files', { count: p.versions.length })}
                        {p.downloads ? ` · ${t('community.downloads', { count: compact(p.downloads) })}` : ''}
                      </span>
                      {p.reason && (p.state === 'rejected' || p.state === 'removed') && <span className="hint bad ellipsis">{p.reason}</span>}
                    </span>
                    <StateChip state={p.state} />
                  </button>
                ))}
              </div>
            )}
          </>
        )
      )}
    </Modal>
  )
}

function ProjectEditor({
  project,
  maxFile,
  categoryOptions,
  categoryLabel,
  onBack,
  onChanged,
  onDeleted
}: {
  project?: CommunityProject
  maxFile: number
  categoryOptions: Partial<Record<string, string[]>>
  categoryLabel: (c: string) => string
  onBack: () => void
  onChanged: (id?: string) => Promise<void>
  onDeleted: () => Promise<void>
}) {
  const { toast } = useStore()
  const [kind, setKind] = useState<CommunityKind>(project?.kind ?? 'modpack')
  const [title, setTitle] = useState(project?.title ?? '')
  const [summary, setSummary] = useState(project?.summary ?? '')
  const [body, setBody] = useState(project?.body ?? '')
  const [categories, setCategories] = useState<string[]>(project?.categories ?? [])
  const [busy, setBusy] = useState<string | null>(null)
  const [deleting, setDeleting] = useState(false)
  const run = async (what: string, work: () => Promise<void>) => {
    setBusy(what)
    try {
      await work()
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(null)
    }
  }
  const save = () =>
    run('save', async () => {
      const p = await api.community.save({ id: project?.id, kind: project ? undefined : kind, title: title.trim(), summary: summary.trim(), body, categories })
      toast(t(project ? 'community.saved' : 'community.created'), 'success')
      await onChanged(p.id)
    })
  const icon = () =>
    run('icon', async () => {
      const path = await api.community.pickIcon()
      if (!path || !project) return
      await api.community.uploadIcon(project.id, path)
      await onChanged(project.id)
    })
  const submit = () =>
    run('submit', async () => {
      await api.community.submit(project!.id)
      toast(t('community.submitted'), 'success')
      await onChanged(project!.id)
    })
  const options = categoryOptions[kind] ?? []
  const canSubmit = !!project && (project.state === 'draft' || project.state === 'rejected') && project.versions.some((v) => v.state !== 'rejected')
  const changed = !project || title !== project.title || summary !== project.summary || body !== project.body || categories.join() !== project.categories.join()
  const valid = title.trim().length >= 3 && summary.trim().length >= 10

  return (
    <div className="cp-editor">
      <div className="row">
        <button className="btn ghost sm" onClick={onBack}>
          <ArrowLeft /> {t('community.mine.back')}
        </button>
        <span className="grow" />
        {project && <StateChip state={project.state} />}
      </div>
      {project?.state === 'pending' && <div className="callout">{t('community.state.pendingHint')}</div>}
      {project?.state === 'approved' && <div className="callout">{t(project.reviewsUpdates ? 'community.state.approvedReview' : 'community.state.approvedTrusted')}</div>}
      {project?.state === 'rejected' && <div className="callout danger">{t('community.state.rejectedHint', { reason: project.reason ?? '' })}</div>}
      {project?.state === 'removed' && <div className="callout danger">{t('community.state.removedHint', { reason: project.reason ?? '' })}</div>}

      <div className="cp-head">
        <button className="cp-icon big" onClick={() => void icon()} disabled={!project || !!busy} title={project ? t('community.icon') : t('community.iconLater')}>
          {busy === 'icon' ? <Loader2 className="spin" /> : project?.icon ? <img src={project.icon} alt="" /> : <ImagePlus />}
        </button>
        <div className="grow cp-fields">
          {!project && (
            <label className="field">
              <span>{t('community.field.kind')}</span>
              <select className="select" value={kind} onChange={(e) => (setKind(e.target.value as CommunityKind), setCategories([]))}>
                {KINDS.map((k) => (
                  <option key={k.value} value={k.value}>
                    {t(k.key)}
                  </option>
                ))}
              </select>
            </label>
          )}
          <label className="field">
            <span>{t('community.field.title')}</span>
            <input className="input" value={title} maxLength={60} onChange={(e) => setTitle(e.target.value)} placeholder={t('community.field.titlePlaceholder')} />
          </label>
          <label className="field">
            <span>{t('community.field.summary')}</span>
            <input className="input" value={summary} maxLength={200} onChange={(e) => setSummary(e.target.value)} placeholder={t('community.field.summaryPlaceholder')} />
          </label>
        </div>
      </div>
      <label className="field">
        <span>{t('community.field.body')}</span>
        <textarea className="input cp-body" value={body} maxLength={20000} rows={8} onChange={(e) => setBody(e.target.value)} placeholder={t('community.field.bodyPlaceholder')} />
      </label>
      {options.length > 0 && (
        <div className="field">
          <span>{t('community.field.categories', { max: MAX_CATEGORIES })}</span>
          <div className="cp-cats">
            {options.map((c) => {
              const on = categories.includes(c)
              return (
                <button
                  key={c}
                  type="button"
                  className={`chip-toggle ${on ? 'on' : ''}`}
                  aria-pressed={on}
                  disabled={!on && categories.length >= MAX_CATEGORIES}
                  onClick={() => setCategories((l) => (on ? l.filter((x) => x !== c) : [...l, c]))}
                >
                  {categoryLabel(c)}
                </button>
              )
            })}
          </div>
        </div>
      )}
      <div className="row">
        {project && (
          <button className="btn ghost danger" onClick={() => setDeleting(true)} disabled={!!busy}>
            <Trash2 /> {t('community.delete')}
          </button>
        )}
        <span className="grow" />
        <button className="btn" disabled={!valid || !changed || !!busy || project?.state === 'removed'} onClick={() => void save()}>
          {busy === 'save' && <Loader2 className="spin" />} {t(project ? 'community.save' : 'community.create')}
        </button>
        {project && (
          <button className="btn primary" disabled={!canSubmit || changed || !!busy} title={canSubmit ? undefined : t('community.submitHint')} onClick={() => void submit()}>
            {busy === 'submit' ? <Loader2 className="spin" /> : <Send />} {t('community.submit')}
          </button>
        )}
      </div>

      {project && (
        <>
          <h3 className="cp-section">{t('community.files.title')}</h3>
          {project.versions.length === 0 && <div className="hint">{t('community.files.none')}</div>}
          {project.versions.map((v) => (
            <VersionRow key={v.id} v={v} onDelete={() => run('v' + v.id, async () => (await api.community.deleteVersion(v.id), await onChanged(project.id)))} busy={busy === 'v' + v.id} />
          ))}
          {project.state !== 'removed' && <AddFile project={project} maxFile={maxFile} onAdded={() => onChanged(project.id)} />}
        </>
      )}
      {deleting && project && (
        <ConfirmModal
          title={t('community.deleteTitle', { name: project.title })}
          body={t('community.deleteBody')}
          confirm={t('community.delete')}
          onConfirm={() => void run('delete', async () => (await api.community.delete(project.id), await onDeleted()))}
          onClose={() => setDeleting(false)}
        />
      )}
    </div>
  )
}

function VersionRow({ v, onDelete, busy }: { v: CommunityVersion; onDelete: () => void; busy: boolean }) {
  return (
    <div className="cp-version">
      <div className="grow cp-text">
        <div className="row">
          <b>{v.name}</b>
          <StateChip state={v.state} />
        </div>
        <span className="hint ellipsis">
          {v.fileName} · {size(v.size)} · {v.mc.join(', ')}
          {v.loaders.length ? ` · ${v.loaders.join(', ')}` : ''} · {timeAgo(v.created * 1000)}
        </span>
        {v.reason && <span className="hint bad">{v.reason}</span>}
      </div>
      <button className="btn ghost icon sm" onClick={onDelete} disabled={busy} aria-label={t('community.files.delete')} title={t('community.files.delete')}>
        {busy ? <Loader2 className="spin" /> : <Trash2 />}
      </button>
    </div>
  )
}

/** A new file for a project: from this PC, or (a modpack) one of the player's instances packed as a .resinpack. */
function AddFile({ project, maxFile, onAdded }: { project: CommunityProject; maxFile: number; onAdded: () => Promise<void> }) {
  const { toast, instances } = useStore()
  const modpack = project.kind === 'modpack'
  const [from, setFrom] = useState<'instance' | 'file'>(modpack ? 'instance' : 'file')
  const [path, setPath] = useState<string | null>(null)
  const [instanceId, setInstanceId] = useState(instances[0]?.id ?? '')
  const [version, setVersion] = useState('1.0.0')
  const [mc, setMc] = useState('')
  const [loaders, setLoaders] = useState<string[]>([])
  const [changelog, setChangelog] = useState('')
  const [busy, setBusy] = useState(false)
  const loaderChoices = project.kind === 'mod' ? MOD_LOADERS : project.kind === 'shader' ? SHADER_LOADERS : project.kind === 'modpack' ? MOD_LOADERS : []
  const fromInstance = modpack && from === 'instance'
  const ready = !!version.trim() && (fromInstance ? !!instanceId : !!path && !!mc.trim()) && !(project.kind === 'mod' && loaders.length === 0)
  const pick = async () => {
    const p = await api.community.pickFile(project.kind).catch((e) => (toast((e as Error).message, 'error'), null))
    if (p) setPath(p)
  }
  const send = async () => {
    setBusy(true)
    try {
      if (fromInstance) await api.community.uploadInstance(project.id, instanceId, version.trim(), changelog.trim())
      else await api.community.upload(project.id, path!, version.trim(), mc, loaders.join(','), changelog.trim())
      toast(t('community.files.added'), 'success')
      setPath(null)
      setChangelog('')
      await onAdded()
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(false)
    }
  }
  return (
    <div className="cp-add">
      <b>{t('community.files.add')}</b>
      {modpack && (
        <div className="row">
          <label className="row">
            <input type="radio" checked={from === 'instance'} onChange={() => setFrom('instance')} /> {t('community.files.fromInstance')}
          </label>
          <label className="row">
            <input type="radio" checked={from === 'file'} onChange={() => setFrom('file')} /> {t('community.files.fromFile')}
          </label>
        </div>
      )}
      {fromInstance ? (
        <label className="field">
          <span>{t('community.files.instance')}</span>
          <select className="select" value={instanceId} onChange={(e) => setInstanceId(e.target.value)}>
            {instances.map((i) => (
              <option key={i.id} value={i.id}>
                {i.name} ({i.mcVersion} {i.loader})
              </option>
            ))}
          </select>
          <span className="hint">{t('community.files.instanceHint')}</span>
        </label>
      ) : (
        <div className="row">
          <button className="btn" onClick={() => void pick()}>
            <FileUp /> {t('community.files.pick')}
          </button>
          <span className="hint ellipsis grow">{path ?? t('community.files.pickHint', { max: size(maxFile) })}</span>
        </div>
      )}
      <div className="row wrap cp-meta">
        <label className="field">
          <span>{t('community.files.version')}</span>
          <input className="input" value={version} maxLength={40} onChange={(e) => setVersion(e.target.value)} />
        </label>
        {!fromInstance && (
          <label className="field grow">
            <span>{t('community.files.mc')}</span>
            <input className="input" value={mc} onChange={(e) => setMc(e.target.value)} placeholder="1.21.1, 1.21" />
          </label>
        )}
      </div>
      {!fromInstance && loaderChoices.length > 0 && (
        <div className="field">
          <span>{t(project.kind === 'shader' ? 'community.files.shaderLoaders' : 'community.files.loaders')}</span>
          <div className="cp-cats">
            {loaderChoices.map(([v, l]) => {
              const on = loaders.includes(v)
              return (
                <button key={v} type="button" className={`chip-toggle ${on ? 'on' : ''}`} aria-pressed={on} onClick={() => setLoaders((x) => (on ? x.filter((y) => y !== v) : [...x, v]))}>
                  {l}
                </button>
              )
            })}
          </div>
        </div>
      )}
      <label className="field">
        <span>{t('community.files.changelog')}</span>
        <textarea className="input" rows={3} maxLength={5000} value={changelog} onChange={(e) => setChangelog(e.target.value)} placeholder={t('community.files.changelogPlaceholder')} />
      </label>
      <div className="row">
        <span className="hint grow">{t(project.state === 'approved' && !project.reviewsUpdates ? 'community.files.goesUp' : 'community.files.waits')}</span>
        <button className="btn primary" disabled={!ready || busy} onClick={() => void send()}>
          {busy ? <Loader2 className="spin" /> : <Upload />} {t('community.files.send')}
        </button>
      </div>
    </div>
  )
}
