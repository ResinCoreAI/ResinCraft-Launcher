import { t, type Key } from '../i18n'
import { toHex } from '../customTheme'

/** The colour variables most themes change, with what each one paints. */
export const GUIDE_VARS: [string, Key][] = [
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
.hw-web-wrap, .hw-monster, .hw-pumpkin, .xm-sleigh, .xm-chest {
  display: none;
}`]
]

const isColour = (name: string) => !name.includes('radius') && !name.includes('font')

/**
 * How to customize: the variables (each colour with a picker), the parts of the app, and examples to start from.
 * Settings → Custom CSS writes what it picks into the CSS box; the theme editor's Fine-tune writes it into the theme.
 */
export function CssGuide({
  add,
  color,
  setColor,
  open
}: {
  /** Adds a snippet of CSS. */
  add: (snippet: string) => void
  /** The colour a variable has now. */
  color: (name: string) => string
  /** A colour picked for a variable. */
  setColor: (name: string, hex: string) => void
  open?: boolean
}) {
  const css = getComputedStyle(document.documentElement)
  return (
    <details className="css-guide" open={open}>
      <summary>{t('settings.cssGuide.title')}</summary>
      <p className="hint">{t('settings.cssGuide.intro')}</p>
      <h4>{t('settings.cssGuide.vars')}</h4>
      <div className="css-guide-grid">
        {GUIDE_VARS.map(([name, label]) => {
          const value = isColour(name) ? color(name) : css.getPropertyValue(name).trim()
          return (
            <div key={name} className="css-guide-row">
              {isColour(name) ? (
                <label className="css-guide-dot pick" style={{ background: value }} title={t('settings.cssGuide.pick')}>
                  <input type="color" value={toHex(value)} onChange={(e) => setColor(name, e.target.value)} aria-label={`${t('settings.cssGuide.pick')} ${name}`} />
                </label>
              ) : (
                <span className="css-guide-dot none" />
              )}
              <button className="css-guide-insert" title={t('settings.cssGuide.insert')} onClick={() => add(`:root {\n  ${name}: ${value};\n}`)}>
                <code>{name}</code>
                <span className="grow">{t(label)}</span>
              </button>
            </div>
          )
        })}
      </div>
      <h4>{t('settings.cssGuide.parts')}</h4>
      <div className="css-guide-grid">
        {GUIDE_PARTS.map(([sel, label]) => (
          <div key={sel} className="css-guide-row">
            <button className="css-guide-insert" title={t('settings.cssGuide.insert')} onClick={() => add(`${sel} {\n  \n}`)}>
              <code>{sel}</code>
              <span className="grow">{t(label)}</span>
            </button>
          </div>
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
