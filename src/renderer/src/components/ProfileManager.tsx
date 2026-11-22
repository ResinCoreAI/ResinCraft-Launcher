import { lazy, Suspense, useEffect, useRef, useState, type DragEvent, type ReactNode, type RefObject } from 'react'
import { AlertTriangle, BookmarkPlus, Check, CircleSlash, Download, Loader2, Pencil, RotateCcw, Search, Upload, X } from 'lucide-react'
import type { AccountView, NameStatus, OnlineSkin, PlayerProfile, SavedSkin, SkinVariant } from '@shared/types'
import { api } from '../api'
import { currentLanguage, t } from '../i18n'
import { useStore } from '../store'
import { FindSkins, SkinShelf, type Peek } from './SkinLibrary'
import { blankSkin, CapeView, guessVariant, readSkinFile, SkinView } from './SkinPreview'
import type { SkinEditorHandle } from './SkinEditor'

// three.js is big: the 3D figure loads only when the skin manager opens.
const Skin3D = lazy(() => import('./Skin3D'))
const SkinEditor = lazy(() => import('./SkinEditor'))
import { Modal } from './ui'
import '../profile.css'

type Tab = 'skin' | 'edit' | 'find' | 'cape' | 'name'
/** The skin open in the editor: where it came from, and its picture as it was loaded (`source`). */
interface Work {
  /** Changes when another skin is opened (the editor then loads `source` and clears undo). */
  key: string
  source: string
  label: string
  from: 'wearing' | 'blank' | 'library' | 'player' | 'link' | 'file'
  /** The name to save it under. */
  name: string
  libraryId?: string
}
let workSeq = 0
const NAME = /^[A-Za-z0-9_]{3,16}$/
const COOLDOWN_DAYS = 30

function day(iso: string, addDays = 0): string {
  const d = new Date(Date.parse(iso) + addDays * 864e5)
  return d.toLocaleDateString(currentLanguage(), { year: 'numeric', month: 'long', day: 'numeric' })
}

/** Accounts → Manage: a Microsoft account's skin, cape and name, changed through Minecraft services. */
export default function ProfileManager({ account, onClose }: { account: AccountView; onClose: () => void }) {
  const { refreshAccounts, toast } = useStore()
  const [profile, setProfile] = useState<PlayerProfile>()
  const [error, setError] = useState<string>()
  const [tab, setTab] = useState<Tab>('skin')
  const [busy, setBusy] = useState<string>()
  // The skin open in the editor, and its picture after the player's edits (none yet: undefined).
  const [work, setWork] = useState<Work>()
  const [edited, setEdited] = useState<string>()
  const [openError, setOpenError] = useState<string>()
  const editor = useRef<SkinEditorHandle>(null)
  const [variant, setVariant] = useState<SkinVariant>('classic')
  // The launcher's skin library, a skin being looked at, and the library skin this window last put on.
  const [library, setLibrary] = useState<SavedSkin[]>()
  const [libraryError, setLibraryError] = useState<string>()
  const [peek, setPeek] = useState<Peek>()
  const [wearing, setWearing] = useState<string>()

  const reloadLibrary = () =>
    api.skins.list().then(
      (l) => {
        setLibrary(l)
        setLibraryError(undefined)
      },
      (e) => setLibraryError((e as Error).message)
    )
  useEffect(() => void reloadLibrary(), [])

  /** Opens a picture in the editor. */
  const open = (w: Omit<Work, 'key'>, v?: SkinVariant) => {
    setWork({ ...w, key: `w${++workSeq}` })
    setEdited(undefined)
    setOpenError(undefined)
    if (v) setVariant(v)
  }
  /** Opens what the account wears now (a plain starting skin for Minecraft's default). */
  const openWearing = async (p: PlayerProfile) => {
    if (!p.skin) return open({ source: blankSkin(), label: t('skinTab.defaultSkin'), from: 'blank', name: t('skinTab.myName') }, 'classic')
    try {
      const png = p.skin.url.startsWith('data:') ? p.skin.url : await api.skins.fetchPng(p.skin.url)
      open({ source: png, label: t('skinTab.wearing'), from: 'wearing', name: p.name }, p.skin.variant)
    } catch (e) {
      open({ source: blankSkin(), label: t('skinTab.defaultSkin'), from: 'blank', name: t('skinTab.myName') }, p.skin.variant)
      setOpenError((e as Error).message)
    }
  }

  const load = () => {
    setError(undefined)
    api.accounts.profile(account.id).then(
      (p) => {
        setProfile(p)
        setVariant(p.skin?.variant ?? 'classic')
        void openWearing(p)
        void refreshAccounts()
      },
      (e) => setError((e as Error).message)
    )
  }
  useEffect(load, [account.id])

  /** Runs one change; the engine returns the updated profile. */
  const run = async (what: string, call: () => Promise<PlayerProfile>, done: (p: PlayerProfile) => string) => {
    setBusy(what)
    try {
      const next = await call()
      setProfile((old) => ({ ...next, nameChange: next.nameChange ?? old?.nameChange }))
      // Keep an arm style the user is still choosing unless this change was to the skin itself.
      if (what === 'skin' || what === 'reset' || what.startsWith('use:')) setVariant(next.skin?.variant ?? 'classic')
      // The skin that was replaced is now in the library as a previous skin.
      if (what !== 'cape' && what !== 'name') void reloadLibrary()
      await refreshAccounts()
      toast(done(next))
      return true
    } catch (e) {
      toast((e as Error).message, 'error')
      return false
    } finally {
      setBusy(undefined)
    }
  }

  /** Wears a library skin. */
  const useSaved = async (s: SavedSkin) => {
    if (await run(`use:${s.id}`, () => api.skins.apply(account.id, s.id), () => t('profile.lib.nowWearing', { name: s.name }))) {
      setWearing(s.id)
      setPeek(undefined)
      open({ source: s.preview, label: t('skinTab.wearing'), from: 'wearing', name: s.name, libraryId: s.id }, s.variant)
    }
  }
  /** Saves an online skin to the library; undefined if that failed (the error is shown as a toast). */
  const saveOnline = async (s: OnlineSkin) => {
    try {
      const saved = await api.skins.saveOnline(s)
      await reloadLibrary()
      toast(t('profile.lib.saved', { name: saved.name }))
      return saved
    } catch (e) {
      toast((e as Error).message, 'error')
      return undefined
    }
  }
  const useOnline = async (s: OnlineSkin) => {
    setBusy(`use:${s.url}`)
    let saved: SavedSkin
    try {
      saved = await api.skins.saveOnline(s)
    } catch (e) {
      toast((e as Error).message, 'error')
      setBusy(undefined)
      return
    }
    if (await run(`use:${s.url}`, () => api.skins.apply(account.id, saved.id), () => t('profile.lib.nowWearing', { name: saved.name }))) {
      setWearing(saved.id)
      setPeek(undefined)
    }
  }

  const cape = profile?.capes.find((c) => c.active)
  const skinUrl = peek ? peek.url : (edited ?? profile?.skin?.url)
  const showVariant = peek ? peek.variant : variant
  const editing = tab === 'edit' && !!profile

  return (
    <Modal title={t('profile.title', { name: profile?.name ?? account.username })} subtitle={t('profile.subtitle')} onClose={onClose} wide>
      {error ? (
        <div className="callout danger">
          <AlertTriangle />
          <div className="grow">{error}</div>
          <button className="btn sm" onClick={load}>
            {t('common.retry')}
          </button>
        </div>
      ) : !profile ? (
        <div className="empty" style={{ padding: 30 }}>
          <Loader2 className="spin" />
          <p>{t('profile.loading')}</p>
        </div>
      ) : (
        <div className={`pm ${editing ? 'pm-edit' : ''}`}>
          {!editing && (
          <div className="pm-figure">
            <Suspense fallback={<SkinView url={skinUrl} variant={showVariant} side="front" scale={8} label={t('profile.skinFront')} />}>
              <Skin3D url={skinUrl} variant={showVariant} capeUrl={cape?.url} label={t('profile.skin3d', { name: profile.name })} />
            </Suspense>
            <b className="pm-name">{profile.name}</b>
            <span className="hint">
              {peek || edited
                ? t('profile.preview')
                : profile.skin
                  ? t(profile.skin.variant === 'slim' ? 'profile.skin.slim' : 'profile.skin.classic')
                  : t('profile.defaultSkin')}
              {cape && !edited && !peek && ` · ${cape.alias}`}
            </span>
          </div>
          )}

          <div className="pm-panel">
            <div className="tabs pm-tabs">
              {(['skin', 'edit', 'find', 'cape', 'name'] as const).map((id) => (
                <button
                  key={id}
                  className={tab === id ? 'on' : ''}
                  onClick={() => {
                    setTab(id)
                    setPeek(undefined)
                  }}
                >
                  {t(`profile.tab.${id}`)}
                </button>
              ))}
            </div>

            {tab === 'skin' && (
              <div className="pm-skin-home">
                <SkinShelf
                  library={library}
                  error={libraryError}
                  reload={reloadLibrary}
                  busy={busy}
                  wearing={wearing}
                  opened={work?.libraryId}
                  open={(sk) => {
                    open({ source: sk.preview, label: sk.name, from: 'library', name: sk.name, libraryId: sk.id }, sk.variant)
                    setTab('edit')
                  }}
                  use={useSaved}
                  toast={toast}
                />
                <div className="pm-actions">
                  <button
                    className="btn ghost"
                    onClick={() =>
                      void run('reset', () => api.accounts.resetSkin(account.id), () => t('profile.skin.wasReset')).then(
                        (ok) => ok && profile && void openWearing({ ...profile, skin: undefined })
                      )
                    }
                    disabled={!!busy || !profile.skin}
                  >
                    {busy === 'reset' ? <Loader2 className="spin" /> : <RotateCcw />} {t('profile.skin.reset')}
                  </button>
                  <div className="grow" />
                  <button className="btn primary" onClick={() => setTab('edit')}>
                    <Pencil /> {t('profile.tab.edit')}
                  </button>
                </div>
              </div>
            )}

            {tab === 'edit' && (
              <SkinTab
                profile={profile}
                work={work}
                edited={edited}
                setEdited={setEdited}
                openError={openError}
                editor={editor}
                variant={variant}
                setVariant={setVariant}
                open={open}
                busy={busy}
                save={async () => {
                  const png = editor.current?.png()
                  if (!png || !work) return
                  try {
                    const saved = await api.skins.importPng(png, edited ? t('skinTab.editedName', { name: work.name }) : work.name, variant)
                    await reloadLibrary()
                    setWork({ ...work, libraryId: saved.id, from: work.from === 'wearing' ? 'wearing' : 'library', name: saved.name })
                    toast(t('profile.lib.saved', { name: saved.name }))
                  } catch (e) {
                    toast((e as Error).message, 'error')
                  }
                }}
                download={() => {
                  const png = editor.current?.png()
                  if (png)
                    void api.app.savePng(`${profile.name}-skin`, png).then(
                      (path) => path && toast(t('skinTab.downloaded')),
                      (e) => toast((e as Error).message, 'error')
                    )
                }}
                apply={async () => {
                  const png = editor.current?.png()
                  if (!png || !work) return
                  if (await run('skin', () => api.accounts.setSkin(account.id, { variant, png }), () => t('profile.skin.changed'))) {
                    setWork({ ...work, from: 'wearing', label: t('skinTab.wearing') })
                    setEdited(undefined)
                    setWearing(work.libraryId)
                  }
                }}
                reset={() =>
                  void run(
                    'reset',
                    () => api.accounts.resetSkin(account.id),
                    () => t('profile.skin.wasReset')
                  ).then((ok) => ok && profile && void openWearing({ ...profile, skin: undefined }))
                }
              />
            )}

            {tab === 'find' && <FindSkins busy={busy} peek={peek} setPeek={setPeek} save={saveOnline} use={useOnline} />}

            {tab === 'cape' && (
              <div className="pm-section">
                <p className="muted">{profile.capes.length ? t('profile.cape.count', { count: profile.capes.length }) : t('profile.cape.empty')}</p>
                <div className="pm-capes">
                  <button
                    className={`pm-cape ${!cape ? 'on' : ''}`}
                    disabled={!!busy || !cape}
                    onClick={() => void run('cape', () => api.accounts.hideCape(account.id), () => t('profile.cape.hidden'))}
                  >
                    <span className="pm-cape-art none">
                      <CircleSlash />
                    </span>
                    <span className="ellipsis">{t('profile.cape.none')}</span>
                    {!cape && <Check className="pm-check" />}
                  </button>
                  {profile.capes.map((c) => (
                    <button
                      key={c.id}
                      className={`pm-cape ${c.active ? 'on' : ''}`}
                      disabled={!!busy || c.active}
                      title={c.alias}
                      onClick={() =>
                        void run(
                          'cape',
                          () => api.accounts.setCape(account.id, c.id),
                          () => t('profile.cape.equipped', { name: c.alias })
                        )
                      }
                    >
                      <span className="pm-cape-art">
                        <CapeView url={c.url} label={c.alias} />
                      </span>
                      <span className="ellipsis">{c.alias}</span>
                      {c.active && <Check className="pm-check" />}
                    </button>
                  ))}
                </div>
                {busy === 'cape' && (
                  <div className="row hint">
                    <Loader2 className="spin" size={14} /> {t('common.working')}
                  </div>
                )}
              </div>
            )}

            {tab === 'name' && (
              <NameTab
                profile={profile}
                busy={busy}
                change={(name) =>
                  run(
                    'name',
                    () => api.accounts.changeName(account.id, name),
                    (p) => t('profile.name.changed', { name: p.name })
                  )
                }
                check={(name) => api.accounts.nameAvailable(account.id, name)}
              />
            )}
          </div>
        </div>
      )}
    </Modal>
  )
}

function SkinTab({
  profile,
  work,
  edited,
  setEdited,
  openError,
  editor,
  variant,
  setVariant,
  open,
  busy,
  save,
  download,
  apply,
  reset
}: {
  profile: PlayerProfile
  work?: Work
  edited?: string
  setEdited: (png?: string) => void
  openError?: string
  editor: RefObject<SkinEditorHandle | null>
  variant: SkinVariant
  setVariant: (v: SkinVariant) => void
  open: (w: Omit<Work, 'key'>, v?: SkinVariant) => void
  busy?: string
  save: () => Promise<void>
  download: () => void
  apply: () => Promise<void>
  reset: () => void
}) {
  const file = useRef<HTMLInputElement>(null)
  const [query, setQuery] = useState('')
  const [finding, setFinding] = useState(false)
  const [findError, setFindError] = useState<string>()
  const [dragging, setDragging] = useState(false)
  const worn = profile.skin?.variant ?? 'classic'
  // Something to apply: another skin, an edit, or other arms for the skin worn now.
  const changed = !!work && (work.from === 'blank' ? !!edited : work.from !== 'wearing' || !!edited || variant !== worn)

  /** The one box: a link opens that picture, anything else is a player's name. */
  const find = async () => {
    const q = query.trim()
    if (!q) return
    setFinding(true)
    setFindError(undefined)
    try {
      if (/^https?:\/\//i.test(q)) {
        const png = await api.skins.fetchPng(q)
        open({ source: png, label: t('skinTab.fromLink'), from: 'link', name: t('skinTab.linkName') }, await guessVariant(png))
      } else {
        const found = await api.skins.lookupPlayer(q)
        const png = await api.skins.fetchPng(found.url)
        open({ source: png, label: t('skinTab.fromPlayer', { name: found.name }), from: 'player', name: found.name }, found.variant ?? (await guessVariant(png)))
      }
      setQuery('')
    } catch (e) {
      setFindError((e as Error).message)
    } finally {
      setFinding(false)
    }
  }
  const pickFile = async (f: File) => {
    setFindError(undefined)
    const r = await readSkinFile(f)
    if (!('dataUrl' in r)) {
      setFindError(r.error === 'size' ? t('profile.skin.badSize', { width: r.width ?? 0, height: r.height ?? 0 }) : t('profile.skin.badType'))
      return
    }
    open({ source: r.dataUrl, label: t('skinTab.fromFile', { file: f.name }), from: 'file', name: f.name.replace(/\.png$/i, '') }, await guessVariant(r.dataUrl))
  }
  const onDrop = (e: DragEvent) => {
    e.preventDefault()
    setDragging(false)
    const f = e.dataTransfer.files?.[0]
    if (f) void pickFile(f)
  }

  return (
    <div
      className={`pm-skin-edit ${dragging ? 'dropping' : ''}`}
      onDragOver={(e) => {
        if (![...e.dataTransfer.types].includes('Files')) return
        e.preventDefault()
        setDragging(true)
      }}
      onDragLeave={(e) => e.currentTarget === e.target && setDragging(false)}
      onDrop={onDrop}
    >
      <div className="se-omni">
        <label className="se-omni-box">
          <Search />
          <input
            className="input"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && void find()}
            placeholder={t('skinTab.omniPlaceholder')}
            aria-label={t('skinTab.omniLabel')}
            spellCheck={false}
            autoComplete="off"
          />
          <button className="btn sm" onClick={() => void find()} disabled={!query.trim() || finding}>
            {finding ? <Loader2 className="spin" /> : null} {t('skinTab.open')}
          </button>
        </label>
        <button className="btn" onClick={() => file.current?.click()} disabled={!!busy}>
          <Upload /> {t('profile.skin.upload')}
        </button>
        <input
          ref={file}
          type="file"
          accept="image/png"
          hidden
          onChange={(e) => {
            const f = e.target.files?.[0]
            e.target.value = ''
            if (f) void pickFile(f)
          }}
        />
      </div>
      <div className="se-open-line">
        {work && (
          <span className="se-open-what">
            <b>{work.label}</b>
            {edited && <span className="chip compact accent">{t('skinTab.edited')}</span>}
          </span>
        )}
        <span className="hint">{findError ? '' : t('skinTab.omniHint')}</span>
        {(findError || openError) && <span className="pm-error">{findError ?? openError}</span>}
      </div>

      <div className="pm-edit-body">
        {work ? (
          <Suspense
            fallback={
              <div className="empty" style={{ padding: 40 }}>
                <Loader2 className="spin" />
              </div>
            }
          >
            <SkinEditor handle={editor} source={work.source} sourceKey={work.key} variant={variant} setVariant={setVariant} onEdit={setEdited} />
          </Suspense>
        ) : (
          <div className="empty" style={{ padding: 40 }}>
            <Loader2 className="spin" />
          </div>
        )}
      </div>
      {dragging && <div className="se-drop">{t('skinTab.drop')}</div>}

      <div className="pm-actions">
        <button className="btn ghost" onClick={reset} disabled={!!busy || !profile.skin}>
          {busy === 'reset' ? <Loader2 className="spin" /> : <RotateCcw />} {t('profile.skin.reset')}
        </button>
        <button className="btn ghost" onClick={download} disabled={!work}>
          <Download /> {t('skinTab.download')}
        </button>
        <button className="btn ghost" onClick={() => void save()} disabled={!work || (!edited && !!work.libraryId)}>
          <BookmarkPlus /> {t('skinTab.save')}
        </button>
        <div className="grow" />
        {work && (edited || work.from !== 'wearing') && (
          <button
            className="btn ghost"
            onClick={() => (work.from === 'wearing' || work.from === 'blank' ? open({ ...work }, variant) : void openWearingAgain(profile, open, setEdited))}
            disabled={!!busy}
          >
            <X /> {t('profile.skin.discard')}
          </button>
        )}
        <button className="btn primary" onClick={() => void apply()} disabled={!changed || !!busy}>
          {busy === 'skin' ? <Loader2 className="spin" /> : <Check />} {t('profile.skin.apply')}
        </button>
      </div>
    </div>
  )
}

/** Discard: back to what the account wears. */
async function openWearingAgain(profile: PlayerProfile, open: (w: Omit<Work, 'key'>, v?: SkinVariant) => void, setEdited: (png?: string) => void) {
  setEdited(undefined)
  if (!profile.skin) return open({ source: blankSkin(), label: t('skinTab.defaultSkin'), from: 'blank', name: t('skinTab.myName') }, 'classic')
  try {
    const png = profile.skin.url.startsWith('data:') ? profile.skin.url : await api.skins.fetchPng(profile.skin.url)
    open({ source: png, label: t('skinTab.wearing'), from: 'wearing', name: profile.name }, profile.skin.variant)
  } catch {
    open({ source: blankSkin(), label: t('skinTab.defaultSkin'), from: 'blank', name: t('skinTab.myName') }, profile.skin.variant)
  }
}

function NameTab({
  profile,
  busy,
  change,
  check
}: {
  profile: PlayerProfile
  busy?: string
  change: (name: string) => Promise<boolean>
  check: (name: string) => Promise<NameStatus>
}) {
  const [name, setName] = useState('')
  const [status, setStatus] = useState<{ name: string; status?: NameStatus; error?: string }>()
  const [confirming, setConfirming] = useState(false)
  const trimmed = name.trim()
  const same = trimmed.toLowerCase() === profile.name.toLowerCase()
  const nc = profile.nameChange
  const locked = nc?.allowed === false

  // Ask Mojang about the name once the user stops typing.
  useEffect(() => {
    setConfirming(false)
    if (!trimmed || same || !NAME.test(trimmed)) return setStatus(undefined)
    setStatus({ name: trimmed })
    const timer = setTimeout(() => {
      check(trimmed).then(
        (s) => setStatus((cur) => (cur?.name === trimmed ? { name: trimmed, status: s } : cur)),
        (e) => setStatus((cur) => (cur?.name === trimmed ? { name: trimmed, error: (e as Error).message } : cur))
      )
    }, 500)
    return () => clearTimeout(timer)
  }, [trimmed])

  const line = (() => {
    if (!trimmed) return null
    if (same) return <span className="hint">{t('profile.name.same')}</span>
    if (!NAME.test(trimmed)) return <span className="pm-error">{t('profile.name.invalid')}</span>
    if (!status || status.name !== trimmed || (!status.status && !status.error))
      return (
        <span className="hint row">
          <Loader2 className="spin" size={13} /> {t('profile.name.checking')}
        </span>
      )
    if (status.error) return <span className="pm-error">{t('profile.name.checkFailed', { error: status.error })}</span>
    if (status.status === 'available')
      return (
        <span className="pm-ok">
          <Check size={14} /> {t('profile.name.available', { name: trimmed })}
        </span>
      )
    return (
      <span className="pm-error">
        {t(status.status === 'taken' ? 'profile.name.taken' : status.status === 'not_allowed' ? 'profile.name.notAllowed' : 'profile.name.invalid', {
          name: trimmed
        })}
      </span>
    )
  })()
  const ready = !locked && !same && status?.name === trimmed && status.status === 'available'

  return (
    <div className="pm-section">
      <div className="field">
        <span className="label">{t('profile.name.current')}</span>
        <b className="pm-current">{profile.name}</b>
        {nc?.changedAt && !locked && <span className="hint">{t('profile.name.lastChanged', { date: day(nc.changedAt) })}</span>}
      </div>
      {locked && (
        <div className="callout">
          <AlertTriangle />
          <div>{nc?.changedAt ? t('profile.name.cooldown', { date: day(nc.changedAt, COOLDOWN_DAYS) }) : t('profile.name.rules')}</div>
        </div>
      )}
      <div className="field">
        <label htmlFor="pm-name">{t('profile.name.new')}</label>
        <input
          id="pm-name"
          className="input"
          value={name}
          maxLength={16}
          disabled={locked || !!busy}
          onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && ready && setConfirming(true)}
          placeholder={profile.name}
          spellCheck={false}
          autoComplete="off"
        />
        <div className="pm-status">{line}</div>
        <span className="hint">{t('profile.name.rules')}</span>
      </div>
      {confirming ? (
        <div className="callout pm-confirm">
          <AlertTriangle />
          <div className="grow">{t('profile.name.confirm', { name: trimmed })}</div>
          <div className="row">
            <button className="btn ghost sm" onClick={() => setConfirming(false)} disabled={!!busy}>
              {t('common.cancel')}
            </button>
            <button
              className="btn primary sm"
              disabled={!!busy}
              onClick={async () => {
                if (await change(trimmed)) {
                  setName('')
                  setConfirming(false)
                }
              }}
            >
              {busy === 'name' && <Loader2 className="spin" />} {t('profile.name.confirmButton')}
            </button>
          </div>
        </div>
      ) : (
        <div className="pm-actions">
          <div className="grow" />
          <button className="btn primary" disabled={!ready || !!busy} onClick={() => setConfirming(true)}>
            {t('profile.name.change')}
          </button>
        </div>
      )}
    </div>
  )
}
