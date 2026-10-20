// Accounts → Manage → Skin / Find skins: the launcher's skin library (shared by every account) and the online search.
import { useEffect, useRef, useState, type ReactNode } from 'react'
import { BookmarkPlus, Check, ExternalLink, History, Loader2, Pencil, Search, Shirt, Trash2, Upload, UserRound } from 'lucide-react'
import type { OnlineSkin, SavedSkin, SkinVariant } from '@shared/types'
import { api } from '../api'
import { t } from '../i18n'
import { timeAgo } from '../util'
import { guessVariant, readSkinFile, SkinView } from './SkinPreview'
import { MoreButton, Segmented } from './ui'

/** What the big preview on the left shows while the user looks around. */
export interface Peek {
  key: string
  url: string
  variant: SkinVariant
}

const SOURCE_ICON: Record<SavedSkin['source'], ReactNode> = {
  upload: <Upload size={11} />,
  player: <UserRound size={11} />,
  online: <Search size={11} />,
  history: <History size={11} />
}

function SkinArt({ url, variant, label, onClick, on }: { url: string; variant: SkinVariant; label: string; onClick: () => void; on: boolean }) {
  return (
    <button className={`pm-skin-art ${on ? 'on' : ''}`} onClick={onClick} title={t('profile.lib.preview')} aria-pressed={on}>
      <SkinView url={url} variant={variant} side="front" scale={3} label={label} />
    </button>
  )
}

/** The library grid with Upload and "From a player" above it. */
export function MySkins({
  library,
  error,
  reload,
  busy,
  wearing,
  peek,
  setPeek,
  use,
  toast
}: {
  library?: SavedSkin[]
  error?: string
  reload: () => Promise<void>
  busy?: string
  wearing?: string
  peek?: Peek
  setPeek: (p?: Peek) => void
  use: (s: SavedSkin) => Promise<void>
  toast: (text: string, kind?: 'error') => void
}) {
  const file = useRef<HTMLInputElement>(null)
  const [fileError, setFileError] = useState<string>()
  const [player, setPlayer] = useState<string>()
  const [renaming, setRenaming] = useState<{ id: string; name: string }>()
  const [deleting, setDeleting] = useState<string>()
  const [working, setWorking] = useState<string>()

  const pick = async (f: File) => {
    setFileError(undefined)
    const r = await readSkinFile(f)
    if (!('dataUrl' in r)) {
      setFileError(r.error === 'size' ? t('profile.skin.badSize', { width: r.width ?? 0, height: r.height ?? 0 }) : t('profile.skin.badType'))
      return
    }
    setWorking('upload')
    try {
      const variant = await guessVariant(r.dataUrl)
      const saved = await api.skins.importPng(r.dataUrl, f.name.replace(/\.png$/i, ''), variant)
      await reload()
      setPeek({ key: saved.id, url: saved.preview, variant: saved.variant })
      toast(t('profile.lib.saved', { name: saved.name }))
    } catch (e) {
      setFileError((e as Error).message)
    } finally {
      setWorking(undefined)
    }
  }

  const savePlayer = async () => {
    const name = player?.trim()
    if (!name) return
    setWorking('player')
    setFileError(undefined)
    try {
      const saved = await api.skins.saveOnline(await api.skins.lookupPlayer(name))
      await reload()
      setPeek({ key: saved.id, url: saved.preview, variant: saved.variant })
      setPlayer(undefined)
      toast(t('profile.lib.saved', { name: saved.name }))
    } catch (e) {
      setFileError((e as Error).message)
    } finally {
      setWorking(undefined)
    }
  }

  const change = async (id: string, name?: string, variant?: SkinVariant) => {
    try {
      const s = await api.skins.update(id, name, variant)
      await reload()
      if (peek?.key === id) setPeek({ key: id, url: s.preview, variant: s.variant })
    } catch (e) {
      toast((e as Error).message, 'error')
    }
  }

  const remove = async (s: SavedSkin) => {
    try {
      await api.skins.remove(s.id)
      if (peek?.key === s.id) setPeek(undefined)
      await reload()
      toast(t('profile.lib.removed', { name: s.name }))
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setDeleting(undefined)
    }
  }

  const locked = !!busy || !!working

  return (
    <div className="field">
      <div className="row" style={{ flexWrap: 'wrap' }}>
        <button className="btn" onClick={() => file.current?.click()} disabled={locked}>
          {working === 'upload' ? <Loader2 className="spin" /> : <Upload />} {t('profile.skin.upload')}
        </button>
        <button className="btn" onClick={() => setPlayer(player === undefined ? '' : undefined)} disabled={locked} aria-expanded={player !== undefined}>
          <UserRound /> {t('profile.lib.fromPlayer')}
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
      {player !== undefined && (
        <div className="row">
          <input
            className="input grow"
            value={player}
            autoFocus
            maxLength={16}
            onChange={(e) => setPlayer(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && void savePlayer()}
            placeholder={t('profile.lib.playerPlaceholder')}
            aria-label={t('profile.lib.playerName')}
            spellCheck={false}
            autoComplete="off"
          />
          <button className="btn primary" onClick={() => void savePlayer()} disabled={!player.trim() || locked}>
            {working === 'player' ? <Loader2 className="spin" /> : <BookmarkPlus />} {t('profile.lib.savePlayer')}
          </button>
        </div>
      )}
      {fileError && <span className="pm-error">{fileError}</span>}

      <div className="pm-lib-head">
        <span className="label">{library ? t('profile.lib.title', { count: library.length }) : t('profile.lib.titlePlain')}</span>
        <span className="hint">{t('profile.lib.hint')}</span>
      </div>
      {error ? (
        <span className="pm-error">{error}</span>
      ) : !library ? (
        <div className="row hint">
          <Loader2 className="spin" size={14} /> {t('common.loading')}
        </div>
      ) : !library.length ? (
        <div className="pm-lib-empty hint">{t('profile.lib.empty')}</div>
      ) : (
        <div className="pm-skins">
          {library.map((s) => (
            <div key={s.id} className={`pm-skin ${wearing === s.id ? 'wearing' : ''}`}>
              <SkinArt
                url={s.preview}
                variant={s.variant}
                label={s.name}
                on={peek?.key === s.id}
                onClick={() => setPeek(peek?.key === s.id ? undefined : { key: s.id, url: s.preview, variant: s.variant })}
              />
              {renaming?.id === s.id ? (
                <input
                  className="input sm"
                  value={renaming.name}
                  autoFocus
                  maxLength={40}
                  aria-label={t('common.rename')}
                  onChange={(e) => setRenaming({ id: s.id, name: e.target.value })}
                  onBlur={() => {
                    if (renaming.name.trim() && renaming.name !== s.name) void change(s.id, renaming.name)
                    setRenaming(undefined)
                  }}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') e.currentTarget.blur()
                    if (e.key === 'Escape') setRenaming(undefined)
                  }}
                />
              ) : (
                <span className="pm-skin-name ellipsis" title={s.name}>
                  {s.name}
                </span>
              )}
              <span className="pm-skin-tag" title={new Date(s.addedAt).toLocaleString()}>
                {SOURCE_ICON[s.source]} {t(`profile.lib.source.${s.source}`)} · {timeAgo(s.addedAt)}
              </span>
              {deleting === s.id ? (
                <div className="pm-skin-actions">
                  <button className="btn sm danger" onClick={() => void remove(s)}>
                    {t('common.delete')}
                  </button>
                  <button className="btn sm ghost" onClick={() => setDeleting(undefined)}>
                    {t('common.cancel')}
                  </button>
                </div>
              ) : (
                <div className="pm-skin-actions">
                  {wearing === s.id ? (
                    <span className="pm-ok">
                      <Check size={14} /> {t('profile.lib.wearing')}
                    </span>
                  ) : (
                    <button className="btn sm primary" disabled={locked} onClick={() => void use(s)}>
                      {busy === `use:${s.id}` ? <Loader2 className="spin" /> : <Shirt />} {t('profile.lib.use')}
                    </button>
                  )}
                  <MoreButton
                    className="sm"
                    title={s.name}
                    items={() => [
                      { label: t('common.rename'), icon: <Pencil />, onSelect: () => setRenaming({ id: s.id, name: s.name }) },
                      {
                        label: t('profile.skin.arms'),
                        items: (['classic', 'slim'] as const).map((v) => ({
                          label: t(v === 'slim' ? 'profile.skin.slim' : 'profile.skin.classic'),
                          checked: s.variant === v,
                          onSelect: () => void change(s.id, undefined, v)
                        }))
                      },
                      '-',
                      { label: t('common.delete'), icon: <Trash2 />, danger: true, onSelect: () => setDeleting(s.id) }
                    ]}
                  />
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  )
}

type Mode = 'gallery' | 'player'

/** Find skins: MineSkin's public gallery, or any Java player's current skin. */
export function FindSkins({
  busy,
  peek,
  setPeek,
  save,
  use
}: {
  busy?: string
  peek?: Peek
  setPeek: (p?: Peek) => void
  /** Saves to the library; resolves to the saved skin. */
  save: (s: OnlineSkin) => Promise<SavedSkin | undefined>
  use: (s: OnlineSkin) => Promise<void>
}) {
  const [mode, setMode] = useState<Mode>('gallery')
  const [query, setQuery] = useState('')
  const [results, setResults] = useState<{ query: string; skins: OnlineSkin[]; next?: string }>()
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string>()
  const [player, setPlayer] = useState('')
  const [found, setFound] = useState<OnlineSkin>()
  const [saved, setSaved] = useState<Record<string, true>>({})
  const [working, setWorking] = useState<string>()
  const seq = useRef(0)

  // Gallery: search once the user stops typing (an empty box lists the newest skins).
  useEffect(() => {
    if (mode !== 'gallery') return
    const q = query.trim()
    const n = ++seq.current
    const timer = setTimeout(
      () => {
        setLoading(true)
        setError(undefined)
        api.skins
          .search(q)
          .then(
            (page) => n === seq.current && setResults({ query: q, ...page }),
            (e) => n === seq.current && setError((e as Error).message)
          )
          .finally(() => n === seq.current && setLoading(false))
      },
      results ? 450 : 0
    )
    return () => clearTimeout(timer)
  }, [query, mode])

  const more = async () => {
    if (!results?.next || loading) return
    const n = seq.current
    setLoading(true)
    try {
      const page = await api.skins.search(results.query, results.next)
      if (n !== seq.current) return
      const have = new Set(results.skins.map((s) => s.url))
      setResults({ query: results.query, skins: [...results.skins, ...page.skins.filter((s) => !have.has(s.url))], next: page.next })
    } catch (e) {
      setError((e as Error).message)
    } finally {
      setLoading(false)
    }
  }

  const lookup = async () => {
    const name = player.trim()
    if (!name) return
    setWorking('lookup')
    setError(undefined)
    setFound(undefined)
    try {
      const s = await api.skins.lookupPlayer(name)
      setFound(s)
      setPeek({ key: `online:${s.url}`, url: s.url, variant: s.variant ?? 'classic' })
    } catch (e) {
      setError((e as Error).message)
    } finally {
      setWorking(undefined)
    }
  }

  const doSave = async (s: OnlineSkin) => {
    setWorking(`save:${s.url}`)
    try {
      if (await save(s)) setSaved((m) => ({ ...m, [s.url]: true }))
    } finally {
      setWorking(undefined)
    }
  }

  const locked = !!busy || !!working

  const card = (s: OnlineSkin) => {
    const key = `online:${s.url}`
    const name = s.name || t('profile.find.untitled')
    return (
      <div key={s.url} className="pm-skin">
        <SkinArt
          url={s.url}
          variant={s.variant ?? 'classic'}
          label={name}
          on={peek?.key === key}
          onClick={() => setPeek(peek?.key === key ? undefined : { key, url: s.url, variant: s.variant ?? 'classic' })}
        />
        <span className={`pm-skin-name ellipsis ${s.name ? '' : 'muted'}`} title={name}>
          {name}
        </span>
        <div className="pm-skin-actions">
          <button className="btn sm primary" disabled={locked} onClick={() => void use(s)}>
            {busy === `use:${s.url}` ? <Loader2 className="spin" /> : <Shirt />} {t('profile.lib.use')}
          </button>
          <button
            className="btn sm icon"
            disabled={locked || saved[s.url]}
            onClick={() => void doSave(s)}
            title={saved[s.url] ? t('profile.find.saved') : t('profile.find.save')}
            aria-label={saved[s.url] ? t('profile.find.saved') : t('profile.find.save')}
          >
            {working === `save:${s.url}` ? <Loader2 className="spin" /> : saved[s.url] ? <Check /> : <BookmarkPlus />}
          </button>
        </div>
      </div>
    )
  }

  return (
    <div className="pm-section">
      <div>
        <Segmented
          value={mode}
          onChange={(m) => {
            setMode(m)
            setError(undefined)
          }}
          options={[
            { value: 'gallery', label: t('profile.find.gallery') },
            { value: 'player', label: t('profile.find.player') }
          ]}
        />
      </div>

      {mode === 'gallery' ? (
        <>
          <div className="field">
            <div className="pm-search">
              <Search size={15} />
              <input
                className="input grow"
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder={t('profile.find.placeholder')}
                aria-label={t('profile.find.gallery')}
                spellCheck={false}
              />
            </div>
            <span className="hint">
              {t('profile.find.credit')}{' '}
              <button className="link" onClick={() => void api.app.openExternal('https://mineskin.org/skins')}>
                mineskin.org <ExternalLink size={11} />
              </button>
            </span>
          </div>
          {error && <span className="pm-error">{error}</span>}
          {results && !results.skins.length && !loading && !error && (
            <div className="pm-lib-empty hint">{results.query ? t('profile.find.none', { query: results.query }) : t('profile.find.noneAtAll')}</div>
          )}
          {results && results.skins.length > 0 && <div className="pm-skins tall">{results.skins.map(card)}</div>}
          {!results && loading && (
            <div className="row hint">
              <Loader2 className="spin" size={14} /> {t('common.loading')}
            </div>
          )}
          {/* A plain button, not infinite scroll: MineSkin limits how often apps may ask. */}
          {results?.next && (
            <button className="btn ghost pm-more" onClick={() => void more()} disabled={loading}>
              {loading && <Loader2 className="spin" />} {t('profile.find.more')}
            </button>
          )}
        </>
      ) : (
        <>
          <div className="field">
            <label htmlFor="pm-player">{t('profile.lib.playerName')}</label>
            <div className="row">
              <input
                id="pm-player"
                className="input grow"
                value={player}
                maxLength={16}
                onChange={(e) => setPlayer(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && void lookup()}
                placeholder={t('profile.lib.playerPlaceholder')}
                spellCheck={false}
                autoComplete="off"
              />
              <button className="btn" onClick={() => void lookup()} disabled={!player.trim() || locked}>
                {working === 'lookup' ? <Loader2 className="spin" /> : <Search />} {t('profile.find.lookUp')}
              </button>
            </div>
            <span className="hint">{t('profile.find.playerHint')}</span>
          </div>
          {error && <span className="pm-error">{error}</span>}
          {found && <div className="pm-skins">{card(found)}</div>}
        </>
      )}
    </div>
  )
}
