import { useEffect, useState } from 'react'
import { Monitor, Sparkles } from 'lucide-react'
import type { Settings } from '@shared/types'
import { t, type Key } from '../i18n'
import { activeSeason, effectiveTheme, THEMES, themesNow } from '../themes'
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
        </div>
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
      <CssGuide add={(snippet) => setCss((c) => (c.trim() ? `${c.trimEnd()}

${snippet}` : snippet))} />
    </div>
  )
}

/** The colour variables most themes change, with what each one paints. */
const GUIDE_VARS: [string, Key][] = [
  ['--bg', 'settings.cssGuide.var.bg'],
  ['--bg-sidebar', 'settings.cssGuide.var.sidebar'],
  ['--surface', 'settings.cssGuide.var.surface'],
  ['--surface-2', 'settings.cssGuide.var.surface2'],
  ['--border', 'settings.cssGuide.var.border'],
  ['--text', 'settings.cssGuide.var.text'],
  ['--text-dim', 'settings.cssGuide.var.textDim'],
  ['--accent', 'settings.cssGuide.var.accent'],
  ['--accent-fg', 'settings.cssGuide.var.accentFg'],
  ['--data', 'settings.cssGuide.var.data'],
  ['--play', 'settings.cssGuide.var.play'],
  ['--danger', 'settings.cssGuide.var.danger'],
  ['--radius', 'settings.cssGuide.var.radius'],
  ['--font', 'settings.cssGuide.var.font']
]

/** Parts of the app a selector can reach. */
const GUIDE_PARTS: [string, Key][] = [
  ['body', 'settings.cssGuide.part.body'],
  ['.sidebar', 'settings.cssGuide.part.sidebar'],
  ['.topbar', 'settings.cssGuide.part.topbar'],
  ['.content', 'settings.cssGuide.part.content'],
  ['.card', 'settings.cssGuide.part.card'],
  ['.icard', 'settings.cssGuide.part.icard'],
  ['.btn.primary', 'settings.cssGuide.part.button'],
  ['.chip', 'settings.cssGuide.part.chip'],
  ['.modal', 'settings.cssGuide.part.modal']
]

/** Ready-made changes, added to the CSS box with one click. */
const GUIDE_EXAMPLES: [Key, string][] = [
  ['settings.cssGuide.ex.background', `/* A picture behind everything (any image link) */
body {
  background: url("https://example.com/my-picture.jpg") center / cover fixed;
}
.content, .sidebar {
  background: rgba(10, 9, 18, 0.82);
}`],
  ['settings.cssGuide.ex.round', `/* Rounder corners */
:root {
  --radius: 14px;
  --radius-sm: 10px;
}`],
  ['settings.cssGuide.ex.bigger', `/* Bigger text */
body {
  font-size: 15.5px;
}`],
  ['settings.cssGuide.ex.neon', `/* A neon glow on main buttons */
.btn.primary {
  box-shadow: 0 0 14px var(--accent);
}`],
  ['settings.cssGuide.ex.noDecor', `/* No holiday decorations */
.hw-web-wrap, .hw-monster, .hw-pumpkin, .xm-sleigh, .xm-tree {
  display: none;
}`]
]

/** Settings → Custom CSS → How to customize: the variables, the parts of the app, and examples to start from. */
function CssGuide({ add }: { add: (snippet: string) => void }) {
  const css = getComputedStyle(document.documentElement)
  return (
    <details className="css-guide">
      <summary>{t('settings.cssGuide.title')}</summary>
      <p className="hint">{t('settings.cssGuide.intro')}</p>
      <h4>{t('settings.cssGuide.vars')}</h4>
      <div className="css-guide-grid">
        {GUIDE_VARS.map(([name, label]) => {
          const value = css.getPropertyValue(name).trim()
          return (
            <button key={name} className="css-guide-row" title={t('settings.cssGuide.insert')} onClick={() => add(`:root {
  ${name}: ${value};
}`)}>
              {name.includes('radius') || name.includes('font') ? <span className="css-guide-dot none" /> : <span className="css-guide-dot" style={{ background: value }} />}
              <code>{name}</code>
              <span className="grow">{t(label)}</span>
            </button>
          )
        })}
      </div>
      <h4>{t('settings.cssGuide.parts')}</h4>
      <div className="css-guide-grid">
        {GUIDE_PARTS.map(([sel, label]) => (
          <button key={sel} className="css-guide-row" title={t('settings.cssGuide.insert')} onClick={() => add(`${sel} {
  
}`)}>
            <code>{sel}</code>
            <span className="grow">{t(label)}</span>
          </button>
        ))}
      </div>
      <h4>{t('settings.cssGuide.examples')}</h4>
      <div className="row wrap" style={{ gap: 6 }}>
        {GUIDE_EXAMPLES.map(([label, snippet]) => (
          <button key={label} className="btn sm" onClick={() => add(snippet)}>
            + {t(label)}
          </button>
        ))}
      </div>
      <p className="hint">{t('settings.cssGuide.tips')}</p>
    </details>
  )
}
