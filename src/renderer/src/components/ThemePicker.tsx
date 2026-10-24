import { useEffect, useState } from 'react'
import { Monitor, Sparkles } from 'lucide-react'
import type { Settings } from '@shared/types'
import { t } from '../i18n'
import { activeSeason, effectiveTheme, THEMES } from '../themes'
import { Switch } from './ui'

/** The colour variables a custom theme usually changes, filled in with the current theme's values. */
const TEMPLATE_VARS = ['--bg', '--bg-sidebar', '--surface', '--surface-2', '--surface-3', '--border', '--text', '--text-dim', '--accent', '--accent-hover', '--accent-fg', '--data', '--play']

function template(): string {
  const css = getComputedStyle(document.documentElement)
  const lines = TEMPLATE_VARS.map((v) => `  ${v}: ${css.getPropertyValue(v).trim()};`)
  return `/* ${t('settings.customCss.templateNote')} */\n:root {\n${lines.join('\n')}\n}\n`
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
  const pick = (id: string) => {
    if (season && id === season.theme) return set({ seasonOff: '' })
    set({ theme: id, ...(season ? { seasonOff: season.key } : {}) })
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
          {THEMES.map((th) => {
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
        </div>
      </div>
      <div className="setting">
        <div className="text">
          <b>{t('settings.season')}</b>
          <span>{t('settings.season.hint')}</span>
        </div>
        <Switch on={s.seasonalThemes !== false} onChange={(v) => set({ seasonalThemes: v })} label={t('settings.season')} />
      </div>
      <CustomCss s={s} set={set} />
    </>
  )
}

function CustomCss({ s, set }: { s: Settings; set: (patch: Partial<Settings>) => void }) {
  const [css, setCss] = useState(s.customCss ?? '')
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
    </div>
  )
}
