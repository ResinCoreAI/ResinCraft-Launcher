import { lazy, Suspense, useEffect, useState, type ReactNode } from 'react'
import { AlertTriangle, Check, CircleSlash, Link2, Loader2, RotateCcw, X } from 'lucide-react'
import type { AccountView, NameStatus, OnlineSkin, PlayerProfile, SavedSkin, SkinUpload, SkinVariant } from '@shared/types'
import { api } from '../api'
import { currentLanguage, t } from '../i18n'
import { useStore } from '../store'
import { FindSkins, MySkins, type Peek } from './SkinLibrary'
import { CapeView, SkinView, useImage } from './SkinPreview'

// three.js is big: the 3D figure loads only when the skin manager opens.
const Skin3D = lazy(() => import('./Skin3D'))
import { Modal, Segmented } from './ui'
import '../profile.css'

type Tab = 'skin' | 'find' | 'cape' | 'name'
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
  // A skin the user picked but hasn't applied yet, shown in the preview.
  const [pending, setPending] = useState<{ upload: Omit<SkinUpload, 'variant'>; preview?: string; label: string }>()
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

  const load = () => {
    setError(undefined)
    api.accounts.profile(account.id).then(
      (p) => {
        setProfile(p)
        setVariant(p.skin?.variant ?? 'classic')
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
      setPending(undefined)
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
  const skinUrl = peek ? peek.url : pending ? pending.preview : profile?.skin?.url
  const showVariant = peek ? peek.variant : variant

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
        <div className="pm">
          <div className="pm-figure">
            <Suspense fallback={<SkinView url={skinUrl} variant={showVariant} side="front" scale={8} label={t('profile.skinFront')} />}>
              <Skin3D url={skinUrl} variant={showVariant} capeUrl={cape?.url} label={t('profile.skin3d', { name: profile.name })} />
            </Suspense>
            <b className="pm-name">{profile.name}</b>
            <span className="hint">
              {peek || pending
                ? t('profile.preview')
                : profile.skin
                  ? t(profile.skin.variant === 'slim' ? 'profile.skin.slim' : 'profile.skin.classic')
                  : t('profile.defaultSkin')}
              {cape && !pending && !peek && ` · ${cape.alias}`}
            </span>
          </div>

          <div className="pm-panel">
            <div className="tabs pm-tabs">
              {(['skin', 'find', 'cape', 'name'] as const).map((id) => (
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
              <SkinTab
                library={
                  <MySkins
                    library={library}
                    error={libraryError}
                    reload={reloadLibrary}
                    busy={busy}
                    wearing={wearing}
                    peek={peek}
                    setPeek={setPeek}
                    use={useSaved}
                    toast={toast}
                  />
                }
                profile={profile}
                variant={variant}
                setVariant={setVariant}
                pending={pending}
                setPending={setPending}
                busy={busy}
                apply={async () => {
                  const upload: SkinUpload = { variant, ...(pending?.upload ?? { url: profile.skin?.url }) }
                  if (await run('skin', () => api.accounts.setSkin(account.id, upload), () => t('profile.skin.changed'))) setPending(undefined)
                }}
                reset={() =>
                  void run(
                    'reset',
                    () => api.accounts.resetSkin(account.id),
                    () => t('profile.skin.wasReset')
                  ).then((ok) => ok && setPending(undefined))
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
  library,
  profile,
  variant,
  setVariant,
  pending,
  setPending,
  busy,
  apply,
  reset
}: {
  library: ReactNode
  profile: PlayerProfile
  variant: SkinVariant
  setVariant: (v: SkinVariant) => void
  pending?: { upload: Omit<SkinUpload, 'variant'>; preview?: string; label: string }
  setPending: (p?: { upload: Omit<SkinUpload, 'variant'>; preview?: string; label: string }) => void
  busy?: string
  apply: () => Promise<void>
  reset: () => void
}) {
  const [link, setLink] = useState('')
  const linkPreview = useImage(pending?.upload.url && pending.preview ? pending.preview : undefined)
  // Something to apply: a new file or link, or a different arm style for the current skin.
  const changed = !!pending || (!!profile.skin && variant !== profile.skin.variant)

  const useLink = () => {
    const url = link.trim()
    if (!url) return
    setPending({ upload: { url }, preview: url, label: url })
  }

  return (
    <div className="pm-section">
      {library}

      <div className="field">
        <label htmlFor="pm-link">{t('profile.skin.fromLink')}</label>
        <div className="row">
          <input
            id="pm-link"
            className="input grow"
            value={link}
            onChange={(e) => setLink(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && useLink()}
            placeholder={t('profile.skin.linkPlaceholder')}
            spellCheck={false}
          />
          <button className="btn" onClick={useLink} disabled={!link.trim() || !!busy}>
            <Link2 /> {t('common.apply')}
          </button>
        </div>
        {pending && (
          <span className="hint ellipsis" title={pending.label}>
            {t('profile.skin.picked', { file: pending.label })}
          </span>
        )}
        {pending?.upload.url && linkPreview.failed && <span className="pm-warn">{t('profile.skin.linkFailed')}</span>}
      </div>

      <div className="field">
        <span className="label">{t('profile.skin.arms')}</span>
        <div>
          <Segmented
            value={variant}
            onChange={setVariant}
            options={[
              { value: 'classic', label: t('profile.skin.classic') },
              { value: 'slim', label: t('profile.skin.slim') }
            ]}
          />
        </div>
        <span className="hint">{t('profile.skin.armsHint')}</span>
      </div>

      <div className="pm-actions">
        <button className="btn ghost" onClick={reset} disabled={!!busy || !profile.skin}>
          {busy === 'reset' ? <Loader2 className="spin" /> : <RotateCcw />} {t('profile.skin.reset')}
        </button>
        <div className="grow" />
        {pending && (
          <button className="btn ghost" onClick={() => setPending(undefined)} disabled={!!busy}>
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
