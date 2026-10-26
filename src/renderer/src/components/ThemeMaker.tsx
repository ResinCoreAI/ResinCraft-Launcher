import { useEffect, useRef, useState, type ReactNode } from 'react'
import { AlertTriangle, ImagePlus, Loader2, Palette, X } from 'lucide-react'
import type { CustomTheme, Settings } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { t } from '../i18n'
import { contrast, FONTS, forgetPicture, pictureFor, showCustom, showTheme, toHex } from '../customTheme'
import { CssGuide } from './CssGuide'

const MAX_PICTURE = 8 * 1024 * 1024

/**
 * Settings → Appearance → Create theme: four colours, a few dials, a background picture and an effect. The whole
 * launcher shows the theme while it's being made; Cancel puts the old one back.
 */
export function ThemeMaker({
  initial,
  isNew,
  s,
  onSave,
  onClose
}: {
  initial: CustomTheme
  isNew: boolean
  s: Settings
  /** Saves the theme (and puts it on). */
  onSave: (theme: CustomTheme) => void
  onClose: () => void
}) {
  const { toast } = useStore()
  const [d, setD] = useState(initial)
  /** A picture picked in this form (a data URL, to save), not saved yet; `preview` shows it (a blob: URL). */
  const [upload, setUpload] = useState<string>()
  const [preview, setPreview] = useState<string>()
  const [dropPicture, setDropPicture] = useState(false)
  const [thumb, setThumb] = useState<string>()
  const [busy, setBusy] = useState(false)
  const file = useRef<HTMLInputElement>(null)
  const saved = useRef(false)
  const latest = useRef(s)
  latest.current = s

  const edit = (patch: Partial<CustomTheme>) => setD((x) => ({ ...x, ...patch }))

  // Fine-tune: the guide's colours map onto the four main ones where they match, the rest become overrides.
  const MAIN: Record<string, 'bg' | 'panel' | 'text' | 'accent'> = { '--bg': 'bg', '--surface': 'panel', '--text': 'text', '--accent': 'accent' }
  const colorOf = (name: string) =>
    MAIN[name] ? d[MAIN[name]] : (d.vars?.[name] ?? toHex(getComputedStyle(document.documentElement).getPropertyValue(name), '#000000'))
  const setColor = (name: string, hex: string) => (MAIN[name] ? edit({ [MAIN[name]]: hex }) : setD((x) => ({ ...x, vars: { ...x.vars, [name]: hex } })))
  const addCss = (snippet: string) => setD((x) => ({ ...x, css: x.css?.trim() ? `${x.css.trimEnd()}\n\n${snippet}` : snippet }))
  const shown: CustomTheme = { ...d, imageFile: !dropPicture && (!!upload || !!d.imageFile) }

  // Live preview, and the old theme back when the form closes without saving.
  useEffect(() => showCustom(shown, preview), [JSON.stringify(shown), preview])
  useEffect(() => () => void (preview && setTimeout(() => URL.revokeObjectURL(preview), 5000)), [preview])
  useEffect(
    () => () => {
      if (!saved.current) showTheme(latest.current, window.matchMedia('(prefers-color-scheme: light)').matches)
    },
    []
  )
  useEffect(() => {
    let alive = true
    void pictureFor(shown, preview).then((u) => alive && setThumb(u))
    return () => void (alive = false)
  }, [preview, shown.imageFile, d.imageUrl])

  const pick = (f: File | undefined) => {
    if (!f) return
    if (f.size > MAX_PICTURE) return toast(t('settings.themeMaker.tooBig'), 'error')
    const reader = new FileReader()
    reader.onload = () => {
      setUpload(String(reader.result))
      setPreview(URL.createObjectURL(f))
      setDropPicture(false)
    }
    reader.readAsDataURL(f)
  }
  const removePicture = () => {
    setUpload(undefined)
    setPreview(undefined)
    setDropPicture(true)
    edit({ imageUrl: '' })
    if (file.current) file.current.value = ''
  }

  const save = async () => {
    setBusy(true)
    try {
      let imageFile = !!d.imageFile
      if (upload) {
        await api.app.saveThemeImage(d.id, upload)
        imageFile = true
      } else if (dropPicture && d.imageFile) {
        await api.app.saveThemeImage(d.id, null)
        imageFile = false
      }
      if (upload || dropPicture) forgetPicture(d.id)
      saved.current = true
      onSave({ ...d, name: d.name.trim() || t('settings.themeMaker.untitled'), description: d.description?.trim(), imageFile })
      onClose()
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(false)
    }
  }

  const readable = contrast(d.text, d.panel) >= 4.5 && contrast(d.text, d.bg) >= 3
  const range = (key: 'radius' | 'blur' | 'panelOpacity' | 'darken', min: number, max: number, unit: string) => (
    <div className="range-row">
      <input type="range" min={min} max={max} value={d[key]} onChange={(e) => edit({ [key]: Number(e.target.value) })} aria-label={t(`settings.themeMaker.${key}`)} />
      <output>
        {d[key]}
        {unit}
      </output>
    </div>
  )
  const field = (label: string, control: ReactNode, wide = false, hint?: string) => (
    <label className={`field ${wide ? 'wide' : ''}`}>
      {label}
      {control}
      {hint && <span className="hint">{hint}</span>}
    </label>
  )

  return (
    <div className="theme-maker" role="group" aria-label={t(isNew ? 'settings.themeMaker.new' : 'settings.themeMaker.edit')}>
      <div className="theme-maker-head">
        <Palette size={18} color="var(--accent)" />
        <h4 className="grow">{t(isNew ? 'settings.themeMaker.new' : 'settings.themeMaker.edit')}</h4>
        <span className="hint">{t('settings.themeMaker.live')}</span>
        <button className="btn sm ghost icon-only" onClick={onClose} title={t('common.cancel')} aria-label={t('common.cancel')}>
          <X />
        </button>
      </div>

      <div className="theme-maker-grid">
        {field(
          t('settings.themeMaker.name'),
          <input className="input" value={d.name} maxLength={40} placeholder={t('settings.themeMaker.namePlaceholder')} onChange={(e) => edit({ name: e.target.value })} autoFocus />
        )}
        {field(
          t('settings.themeMaker.description'),
          <input className="input" value={d.description ?? ''} maxLength={80} placeholder={t('settings.themeMaker.descriptionPlaceholder')} onChange={(e) => edit({ description: e.target.value })} />,
          true
        )}
        {field(
          t('settings.themeMaker.style'),
          <select
            className="select"
            value={d.style}
            onChange={(e) => {
              const style = e.target.value as CustomTheme['style']
              // Glass needs something to see through: start it frosted.
              edit(style === 'glass' && d.panelOpacity === 100 ? { style, panelOpacity: 72, blur: Math.max(d.blur, 14) } : { style })
            }}
          >
            <option value="flat">{t('settings.themeMaker.style.flat')}</option>
            <option value="glass">{t('settings.themeMaker.style.glass')}</option>
            <option value="soft">{t('settings.themeMaker.style.soft')}</option>
          </select>
        )}
      </div>

      <div className="theme-maker-grid">
        {field(t('settings.themeMaker.bg'), <input type="color" value={d.bg} onChange={(e) => edit({ bg: e.target.value })} />)}
        {field(t('settings.themeMaker.panel'), <input type="color" value={d.panel} onChange={(e) => edit({ panel: e.target.value })} />)}
        {field(t('settings.themeMaker.text'), <input type="color" value={d.text} onChange={(e) => edit({ text: e.target.value })} />)}
        {field(t('settings.themeMaker.accent'), <input type="color" value={d.accent} onChange={(e) => edit({ accent: e.target.value })} />)}
        {field(
          t('settings.themeMaker.font'),
          <select className="select" value={d.font} onChange={(e) => edit({ font: e.target.value })}>
            {FONTS.map((f) => (
              <option key={f.id} value={f.id}>
                {f.id ? f.label : t('settings.themeMaker.fontDefault')}
              </option>
            ))}
          </select>
        )}
      </div>
      {!readable && (
        <p className="hint row" role="status">
          <AlertTriangle size={14} color="var(--warning)" /> {t('settings.themeMaker.hardToRead')}
        </p>
      )}

      <div className="theme-maker-grid">
        {field(t('settings.themeMaker.radius'), range('radius', 0, 24, 'px'))}
        {field(t('settings.themeMaker.panelOpacity'), range('panelOpacity', 20, 100, '%'), false, t('settings.themeMaker.panelOpacityHint'))}
        {field(t('settings.themeMaker.blur'), range('blur', 0, 30, 'px'), false, t('settings.themeMaker.blurHint'))}
        {field(
          t('settings.themeMaker.effect'),
          <select className="select" value={d.effect} onChange={(e) => edit({ effect: e.target.value as CustomTheme['effect'] })}>
            {(['none', 'snow', 'petals', 'rain', 'embers', 'fireflies'] as const).map((x) => (
              <option key={x} value={x}>
                {t(`settings.themeMaker.effect.${x}`)}
              </option>
            ))}
          </select>
        )}
      </div>

      <div className="theme-maker-grid">
        {field(
          t('settings.themeMaker.imageUrl'),
          <input
            className="input"
            value={upload || (shown.imageFile && !dropPicture) ? '' : (d.imageUrl ?? '')}
            disabled={!!upload || (!!d.imageFile && !dropPicture)}
            placeholder="https://…"
            onChange={(e) => edit({ imageUrl: e.target.value })}
          />,
          true,
          d.imageUrl && !/^https:/i.test(d.imageUrl.trim()) ? t('settings.themeMaker.httpsOnly') : undefined
        )}
        <div className="field wide">
          <span className="hint">{t('settings.themeMaker.upload')}</span>
          <div className="theme-maker-picture">
            <span className="thumb" style={thumb ? { backgroundImage: `url("${thumb}")` } : undefined} aria-hidden />
            <input ref={file} type="file" accept="image/png,image/jpeg,image/webp,image/gif,image/avif" hidden onChange={(e) => pick(e.target.files?.[0])} />
            <button className="btn sm" onClick={() => file.current?.click()}>
              <ImagePlus /> {t('settings.themeMaker.choose')}
            </button>
            {thumb && (
              <button className="btn sm ghost" onClick={removePicture}>
                {t('settings.themeMaker.removePicture')}
              </button>
            )}
          </div>
        </div>
        {field(
          t('settings.themeMaker.fit'),
          <select className="select" value={d.imageFit} onChange={(e) => edit({ imageFit: e.target.value as CustomTheme['imageFit'] })} disabled={!thumb}>
            {(['cover', 'contain', 'tile', 'center'] as const).map((x) => (
              <option key={x} value={x}>
                {t(`settings.themeMaker.fit.${x}`)}
              </option>
            ))}
          </select>
        )}
        {field(t('settings.themeMaker.darken'), range('darken', 0, 90, '%'))}
      </div>

      <details className="theme-maker-tune" open={!!(d.css || Object.keys(d.vars ?? {}).length)}>
        <summary>{t('settings.themeMaker.tune')}</summary>
        <span className="hint">{t('settings.themeMaker.tuneHint')}</span>
        <CssGuide open add={addCss} color={colorOf} setColor={setColor} />
        <label className="field">
          {t('settings.themeMaker.themeCss')}
          <textarea
            className="input mono"
            value={d.css ?? ''}
            spellCheck={false}
            placeholder={'.sidebar {\n  border-right: 2px solid var(--accent);\n}'}
            onChange={(e) => edit({ css: e.target.value })}
            aria-label={t('settings.themeMaker.themeCss')}
          />
          <span className="hint">{t('settings.themeMaker.themeCssHint')}</span>
        </label>
        {(d.css || Object.keys(d.vars ?? {}).length > 0) && (
          <div className="row">
            <button className="btn sm ghost" onClick={() => edit({ vars: {}, css: '' })}>
              {t('settings.themeMaker.resetTune')}
            </button>
          </div>
        )}
      </details>

      <div className="theme-maker-foot">
        <button className="btn primary" disabled={busy} onClick={() => void save()}>
          {busy && <Loader2 className="spin" />} {t(isNew ? 'settings.themeMaker.create' : 'common.save')}
        </button>
        <button className="btn ghost" onClick={onClose}>
          {t('common.cancel')}
        </button>
      </div>
    </div>
  )
}
