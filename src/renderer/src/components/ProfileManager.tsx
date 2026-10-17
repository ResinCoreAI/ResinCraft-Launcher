import { useEffect, useRef, useState } from 'react'
import { AlertTriangle, Check, CircleSlash, Link2, Loader2, RotateCcw, Upload, X } from 'lucide-react'
import type { AccountView, NameStatus, PlayerProfile, SkinUpload, SkinVariant } from '@shared/types'
import { api } from '../api'
import { currentLanguage, t } from '../i18n'
import { useStore } from '../store'
import { CapeView, readSkinFile, SkinView, useImage } from './SkinPreview'
import { Modal, Segmented } from './ui'
import '../profile.css'

type Tab = 'skin' | 'cape' | 'name'
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
      if (what === 'skin' || what === 'reset') setVariant(next.skin?.variant ?? 'classic')
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

  const cape = profile?.capes.find((c) => c.active)
  const skinUrl = pending ? pending.preview : profile?.skin?.url
  const showVariant = variant

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
            <div className="pm-views">
              <figure>
                <SkinView url={skinUrl} variant={showVariant} side="front" label={t('profile.skinFront')} />
                <figcaption>{t('profile.front')}</figcaption>
              </figure>
              <figure>
                <SkinView url={skinUrl} variant={showVariant} side="back" capeUrl={cape?.url} label={t('profile.skinBack')} />
                <figcaption>{t('profile.back')}</figcaption>
              </figure>
            </div>
            <b className="pm-name">{profile.name}</b>
            <span className="hint">
              {pending
                ? t('profile.preview')
                : profile.skin
                  ? t(profile.skin.variant === 'slim' ? 'profile.skin.slim' : 'profile.skin.classic')
                  : t('profile.defaultSkin')}
              {cape && !pending && ` · ${cape.alias}`}
            </span>
          </div>

          <div className="pm-panel">
            <div className="tabs pm-tabs">
              {(['skin', 'cape', 'name'] as const).map((id) => (
                <button key={id} className={tab === id ? 'on' : ''} onClick={() => setTab(id)}>
                  {t(`profile.tab.${id}`)}
                </button>
              ))}
            </div>

            {tab === 'skin' && (
              <SkinTab
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
  variant,
  setVariant,
  pending,
  setPending,
  busy,
  apply,
  reset
}: {
  profile: PlayerProfile
  variant: SkinVariant
  setVariant: (v: SkinVariant) => void
  pending?: { upload: Omit<SkinUpload, 'variant'>; preview?: string; label: string }
  setPending: (p?: { upload: Omit<SkinUpload, 'variant'>; preview?: string; label: string }) => void
  busy?: string
  apply: () => Promise<void>
  reset: () => void
}) {
  const file = useRef<HTMLInputElement>(null)
  const [link, setLink] = useState('')
  const [fileError, setFileError] = useState<string>()
  const linkPreview = useImage(pending?.upload.url && pending.preview ? pending.preview : undefined)
  // Something to apply: a new file or link, or a different arm style for the current skin.
  const changed = !!pending || (!!profile.skin && variant !== profile.skin.variant)

  const pick = async (f: File) => {
    setFileError(undefined)
    const r = await readSkinFile(f)
    if ('dataUrl' in r) setPending({ upload: { png: r.dataUrl }, preview: r.dataUrl, label: f.name })
    else setFileError(r.error === 'size' ? t('profile.skin.badSize', { width: r.width ?? 0, height: r.height ?? 0 }) : t('profile.skin.badType'))
  }

  const useLink = () => {
    const url = link.trim()
    if (!url) return
    setFileError(undefined)
    setPending({ upload: { url }, preview: url, label: url })
  }

  return (
    <div className="pm-section">
      <div className="field">
        <div className="row" style={{ flexWrap: 'wrap' }}>
          <button className="btn" onClick={() => file.current?.click()} disabled={!!busy}>
            <Upload /> {t('profile.skin.upload')}
          </button>
          <span className="hint grow">{t('profile.skin.uploadHint')}</span>
        </div>
        <input
          ref={file}
          type="file"
          accept="image/png"
          hidden
          onChange={(e) => {
            const f = e.target.files?.[0]
            e.target.value = ''
            if (f) void pick(f)
          }}
        />
        {fileError && <span className="pm-error">{fileError}</span>}
        {pending && (
          <span className="hint ellipsis" title={pending.label}>
            {t('profile.skin.picked', { file: pending.label })}
          </span>
        )}
        {pending?.upload.url && linkPreview.failed && <span className="pm-warn">{t('profile.skin.linkFailed')}</span>}
      </div>

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
