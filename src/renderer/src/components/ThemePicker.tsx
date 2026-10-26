import { useEffect, useRef, useState } from 'react'
import { Download, Monitor, Pencil, Plus, Sparkles, Trash2, Upload } from 'lucide-react'
import type { CustomTheme, Settings } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { cssVarIn, draftFromScreen, forgetPicture, pictureFor, readThemeFile, setCssVar, themeFile, themeFromScreen } from '../customTheme'
import { CssGuide } from './CssGuide'
import { ThemeMaker } from './ThemeMaker'
import { t } from '../i18n'
import { activeSeason, effectiveTheme, THEMES, themesNow } from '../themes'
import { ConfirmModal, Switch } from './ui'

/** The colour variables a custom theme usually changes, filled in with the current theme's values. */
const TEMPLATE_VARS = ['--bg', '--bg-sidebar', '--surface', '--surface-2', '--surface-3', '--border', '--text', '--text-dim', '--accent', '--accent-hover', '--accent-fg', '--data', '--play']

function template(): string {
  const css = getComputedStyle(document.documentElement)
  const lines = TEMPLATE_VARS.map((v) => `  ${v}: ${css.getPropertyValue(v).trim()};`)
  return `/* ${t('settings.customCss.templateNote')} */\n:root {\n${lines.join('\n')}\n}\n`
}

/** A custom theme's card picture: its background (or picture) with its accent, panel and text colours. */
function CustomSwatch({ th }: { th: CustomTheme }) {
  const [picture, setPicture] = useState<string>()
  useEffect(() => {
    let alive = true
    void pictureFor(th).then((u) => alive && setPicture(u))
    return () => void (alive = false)
  }, [th.id, th.imageFile, th.imageUrl])
  return (
    <span
      className={`theme-swatch ${picture ? 'has-picture' : ''}`}
      style={{ background: picture ? `linear-gradient(rgba(0,0,0,${th.darken / 100}), rgba(0,0,0,${th.darken / 100})), url("${picture}")` : th.bg }}
    >
      {[th.accent, th.panel, th.text].map((c, i) => (
        <i key={i} style={{ background: c, outline: '1px solid rgba(127,127,127,0.35)' }} />
      ))}
    </span>
  )
}

/** Settings → Appearance: the theme cards, the holiday themes, and the player's own CSS. */
export function ThemePicker({ s, set }: { s: Settings; set: (patch: Partial<Settings>) => void }) {
  const prefersLight = window.matchMedia('(prefers-color-scheme: light)').matches
  const season = activeSeason()
  const seasonOn = !!season && s.seasonalThemes !== false && s.seasonOff !== season.key
  const shown = effectiveTheme(s, prefersLight)
  const seasonName = season ? t(THEMES.find((x) => x.id === season.theme)!.label) : ''

  // Picking a theme while a holiday's is on keeps the pick for this holiday; picking the holiday's own theme
  // brings the holiday back without changing the theme for the rest of the year.
  const pick = (id: string, extra: Partial<Settings> = {}) => {
    if (season && id === season.theme) return set({ seasonOff: '', ...extra })
    set({ theme: id, ...(season ? { seasonOff: season.key } : {}), ...extra })
  }

  // The player's own themes (ThemeMaker): made, edited and deleted here.
  const mine = s.customThemes ?? []
  const [making, setMaking] = useState<{ theme: CustomTheme; isNew: boolean }>()
  const [deleting, setDeleting] = useState<CustomTheme>()
  const current = shown.startsWith('custom:') ? mine.find((c) => `custom:${c.id}` === shown) : undefined
  const saveTheme = (th: CustomTheme) => {
    const list = mine.some((c) => c.id === th.id) ? mine.map((c) => (c.id === th.id ? th : c)) : [...mine, th]
    pick(`custom:${th.id}`, { customThemes: list })
  }
  const deleteTheme = async (th: CustomTheme) => {
    if (th.imageFile) await api.app.saveThemeImage(th.id, null).catch(() => {})
    forgetPicture(th.id)
    set({ customThemes: mine.filter((c) => c.id !== th.id), ...(s.theme === `custom:${th.id}` ? { theme: 'dark' } : {}) })
  }

  // Sharing: the theme on screen goes out as a .rctheme file (a built-in one with the player's Custom CSS too);
  // a file someone sent comes in as a new theme of your own.
  const { toast } = useStore()
  const importInput = useRef<HTMLInputElement>(null)
  const exportShown = async () => {
    const builtIn = THEMES.find((x) => x.id === shown)
    const th = current ?? themeFromScreen(builtIn ? t(builtIn.label) : t('settings.themeMaker.untitled'), s.customCss)
    try {
      const path = await api.app.exportTheme(th.name, await themeFile(th))
      if (path) toast(t('settings.themeMaker.exported', { path }), 'success')
    } catch (e) {
      toast((e as Error).message, 'error')
    }
  }
  const importFile = async (file: File | undefined, input: HTMLInputElement) => {
    input.value = ''
    if (!file) return
    try {
      const { theme, picture } = readThemeFile(await file.text())
      if (picture) {
        await api.app.saveThemeImage(theme.id, picture)
        theme.imageFile = true
      }
      saveTheme(theme)
      toast(t('settings.themeMaker.imported', { name: theme.name }), 'success')
    } catch (e) {
      toast(e instanceof SyntaxError || (e as Error).message === 'not a theme' ? t('settings.themeMaker.badFile') : (e as Error).message, 'error')
    }
  }

  return (
    <>
      <div className="setting stack">
        <div className="text">
          <b>{t('settings.theme')}</b>
          <span>{t('settings.theme.hint')}</span>
        </div>
        {season && (
          <div className="theme-season">
            <Sparkles size={15} />
            <span className="grow">
              {seasonOn
                ? t('settings.season.on', { name: seasonName, date: season.until.toLocaleDateString(undefined, { day: 'numeric', month: 'long' }) })
                : t('settings.season.off', { name: seasonName })}
            </span>
            <button className="btn sm" onClick={() => set({ seasonOff: seasonOn ? season.key : '' })}>
              {seasonOn ? t('settings.season.useMine') : t('settings.season.turnOn', { name: seasonName })}
            </button>
          </div>
        )}
        <div className="theme-grid" role="radiogroup" aria-label={t('settings.theme')}>
          <button
            role="radio"
            aria-checked={!seasonOn && s.theme === 'system'}
            className={`theme-card ${!seasonOn && s.theme === 'system' ? 'on' : ''}`}
            onClick={() => pick('system')}
          >
            <span className="theme-swatch system">
              <Monitor size={16} />
            </span>
            <span className="theme-name">{t('settings.theme.system')}</span>
          </button>
          {themesNow().map((th) => {
            const on = shown === th.id && !(s.theme === 'system' && !seasonOn)
            return (
              <button key={th.id} role="radio" aria-checked={on} className={`theme-card ${on ? 'on' : ''}`} onClick={() => pick(th.id)}>
                <span className="theme-swatch" style={{ background: th.swatch[0] }}>
                  {th.swatch.slice(1).map((c) => (
                    <i key={c} style={{ background: c }} />
                  ))}
                </span>
                <span className="theme-name">
                  {t(th.label)}
                  {th.seasonal && <span className="faint"> · {t('settings.theme.seasonal')}</span>}
                </span>
              </button>
            )
          })}
          {mine.map((th) => {
            const on = shown === `custom:${th.id}`
            return (
              <button
                key={th.id}
                role="radio"
                aria-checked={on}
                className={`theme-card ${on ? 'on' : ''}`}
                title={th.description || th.name}
                onClick={() => pick(`custom:${th.id}`)}
                onDoubleClick={() => setMaking({ theme: th, isNew: false })}
              >
                <CustomSwatch th={th} />
                <span className="theme-name">{th.name}</span>
              </button>
            )
          })}
          <button className="theme-card" onClick={() => setMaking({ theme: draftFromScreen(), isNew: true })} disabled={!!making}>
            <span className="theme-swatch add">
              <Plus size={18} />
            </span>
            <span className="theme-name">{t('settings.themeMaker.new')}</span>
          </button>
        </div>
        {!making && (
          <div className="row wrap" style={{ gap: 8 }}>
            <span className="grow muted ellipsis">
              {current && (
                <>
                  <b>{current.name}</b>
                  {current.description && ` · ${current.description}`}
                </>
              )}
            </span>
            {current && (
              <>
                <button className="btn sm" onClick={() => setMaking({ theme: current, isNew: false })}>
                  <Pencil /> {t('common.edit')}
                </button>
                <button className="btn sm ghost" onClick={() => setDeleting(current)}>
                  <Trash2 /> {t('common.delete')}
                </button>
              </>
            )}
            <button className="btn sm" title={t('settings.themeMaker.exportHint')} onClick={() => void exportShown()}>
              <Download /> {t('settings.themeMaker.export')}
            </button>
            <button className="btn sm" onClick={() => importInput.current?.click()}>
              <Upload /> {t('settings.themeMaker.import')}
            </button>
            <input ref={importInput} type="file" accept=".rctheme,application/json" hidden onChange={(e) => void importFile(e.target.files?.[0], e.target)} />
          </div>
        )}
        {making && <ThemeMaker key={making.theme.id} initial={making.theme} isNew={making.isNew} s={s} onSave={saveTheme} onClose={() => setMaking(undefined)} />}
        {deleting && (
          <ConfirmModal
            title={t('settings.themeMaker.deleteTitle', { name: deleting.name })}
            body={t('settings.themeMaker.deleteBody')}
            confirm={t('common.delete')}
            onConfirm={() => deleteTheme(deleting)}
            onClose={() => setDeleting(undefined)}
          />
        )}
      </div>
      {/* Only while a holiday lasts: the rest of the year its theme is a surprise. */}
      {season && (
        <div className="setting">
          <div className="text">
            <b>{t('settings.season')}</b>
            <span>{t('settings.season.hint')}</span>
          </div>
          <Switch on={s.seasonalThemes !== false} onChange={(v) => set({ seasonalThemes: v })} label={t('settings.season')} />
        </div>
      )}
      <CustomCss s={s} set={set} />
    </>
  )
}

function CustomCss({ s, set }: { s: Settings; set: (patch: Partial<Settings>) => void }) {
  const [css, setCss] = useState(s.customCss ?? '')
  const applyTimer = useRef<number | undefined>(undefined)
  useEffect(() => setCss(s.customCss ?? ''), [s.customCss])
  const save = (v: string) => v !== (s.customCss ?? '') && set({ customCss: v })
  return (
    <div className="setting stack">
      <div className="text">
        <b>{t('settings.customCss')}</b>
        <span>{t('settings.customCss.hint')}</span>
      </div>
      <textarea
        className="input mono custom-css"
        rows={6}
        value={css}
        spellCheck={false}
        placeholder={':root {\n  --accent: #ff8cc6;\n}'}
        onChange={(e) => setCss(e.target.value)}
        onBlur={() => save(css)}
        aria-label={t('settings.customCss')}
      />
      <div className="row">
        <button className="btn sm" onClick={() => setCss((c) => (c.trim() ? `${c.trimEnd()}\n\n${template()}` : template()))}>
          {t('settings.customCss.template')}
        </button>
        {css && (
          <button className="btn sm ghost" onClick={() => (setCss(''), save(''))}>
            {t('settings.customCss.clear')}
          </button>
        )}
        <span className="grow" />
        <button className="btn sm primary" disabled={css === (s.customCss ?? '')} onClick={() => save(css)}>
          {t('settings.customCss.apply')}
        </button>
      </div>
      <CssGuide
        add={(snippet) => setCss((c) => (c.trim() ? `${c.trimEnd()}\n\n${snippet}` : snippet))}
        color={(name) => cssVarIn(css, name) ?? getComputedStyle(document.documentElement).getPropertyValue(name).trim()}
        setColor={(name, hex) => {
          // A picked colour shows at once: its line goes in the box, and the CSS is applied a moment later.
          const next = setCssVar(css, name, hex)
          setCss(next)
          window.clearTimeout(applyTimer.current)
          applyTimer.current = window.setTimeout(() => save(next), 350)
        }}
      />
    </div>
  )
}
