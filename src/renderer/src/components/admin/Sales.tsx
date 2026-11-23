// Admin → Payments → Sales: a percentage off with no code to type while it runs (relay sales.rs): once (from a date to
// a date), every year (Black Friday: a month and day, for some days) or every month (a day, for some days). One
// percentage for everything and/or one per chosen item. Card purchases get it on the first payment.
import { useState } from 'react'
import { Ban, CalendarClock, Pencil, Plus, Tag, Trash2 } from 'lucide-react'
import { currentLanguage, t } from '../../i18n'
import { ConfirmModal, Empty, Modal, Segmented } from '../ui'
import { ago, when } from './common'

export interface Sale {
  id: string
  name: string
  percent: number
  items: Record<string, number>
  repeat: 'once' | 'yearly' | 'monthly'
  start: number
  end: number
  month: number
  day: number
  days: number
  active: boolean
  created: number
  by: string
  live: boolean
  ends: number | null
  next: number | null
  ended: boolean
}
type Act = (path: string, body?: unknown, done?: never) => Promise<boolean>

const monthName = (m: number) => new Date(2026, m - 1, 1).toLocaleDateString(currentLanguage(), { month: 'long' })

/** "every year from 27 November, 4 days" and the like. */
function whenText(s: Sale): string {
  if (s.repeat === 'once') return t('servers.admin.sales.once', { from: when(s.start), to: when(s.end) })
  if (s.repeat === 'yearly') return t('servers.admin.sales.yearly', { day: s.day, month: monthName(s.month), days: s.days })
  return t('servers.admin.sales.monthly', { day: s.day, days: s.days })
}

export function Sales({ sales, products, act }: { sales: Sale[]; products: { id: string; name: string }[]; act: (path: string, body?: unknown) => Promise<boolean> }) {
  const [editing, setEditing] = useState<Sale | 'new'>()
  const [removing, setRemoving] = useState<Sale>()
  const name = (id: string) => products.find((p) => p.id === id)?.name ?? id
  const what = (s: Sale) =>
    [s.percent ? t('servers.admin.sales.offAll', { percent: s.percent }) : '', ...Object.entries(s.items).map(([id, p]) => t('servers.admin.sales.offItem', { percent: p, item: name(id) }))]
      .filter(Boolean)
      .join(' · ')
  const running = sales.filter((s) => !s.ended)
  const over = sales.filter((s) => s.ended)
  const row = (s: Sale) => (
    <div key={s.id} className={`setting admin-item ${s.ended ? 'faint' : ''}`}>
      <div className="text">
        <b>
          {s.name}{' '}
          <span className={`chip compact ${s.live ? 'success' : s.ended ? '' : 'warn'}`}>
            {s.live ? t('servers.admin.sales.live') : s.ended ? t('servers.admin.sales.over') : t('servers.admin.sales.waiting')}
          </span>
        </b>
        <span>{what(s)}</span>
        <span className="hint">
          <CalendarClock className="inline" /> {whenText(s)}
          {s.live && s.ends ? ` · ${t('servers.admin.sales.endsAt', { when: when(s.ends) })}` : ''}
          {!s.live && s.next ? ` · ${t('servers.admin.sales.nextAt', { when: when(s.next) })}` : ''}
          {` · ${s.by} · ${ago(s.created)}`}
        </span>
      </div>
      <div className="row">
        <button className="btn sm ghost" title={t('servers.admin.sales.edit')} aria-label={t('servers.admin.sales.edit')} onClick={() => setEditing(s)}>
          <Pencil />
        </button>
        {s.active && (
          <button className="btn sm" onClick={() => void act('/api/payments/sale/off', { id: s.id })}>
            <Ban /> {t('servers.admin.sales.end')}
          </button>
        )}
        <button className="btn sm ghost danger" title={t('common.delete')} aria-label={t('common.delete')} onClick={() => setRemoving(s)}>
          <Trash2 />
        </button>
      </div>
    </div>
  )
  return (
    <div className="settings-section">
      <div className="toolbar">
        <h3 className="grow">{t('servers.admin.sales.title')}</h3>
        <button className="btn sm primary" disabled={products.length === 0} onClick={() => setEditing('new')}>
          <Plus /> {t('servers.admin.sales.add')}
        </button>
      </div>
      <p className="hint">{t('servers.admin.sales.hint')}</p>
      {running.length === 0 ? <Empty icon={<Tag />} title={t('servers.admin.sales.none')} /> : <div className="card admin-items">{running.map(row)}</div>}
      {over.length > 0 && (
        <details className="ended-log">
          <summary className="hint">{t('servers.admin.sales.overList', { n: over.length })}</summary>
          <div className="card admin-items">{over.map(row)}</div>
        </details>
      )}
      {editing && <SaleModal sale={editing === 'new' ? undefined : editing} products={products} act={act} onClose={() => setEditing(undefined)} />}
      {removing && (
        <ConfirmModal
          title={t('servers.admin.sales.removeTitle', { name: removing.name })}
          body={t('servers.admin.sales.removeBody')}
          confirm={t('common.delete')}
          onConfirm={async () => void (await act('/api/payments/sale/remove', { id: removing.id }))}
          onClose={() => setRemoving(undefined)}
        />
      )}
    </div>
  )
}

/** A date-and-time input's value for Unix seconds (local time). */
const local = (secs: number) => {
  if (!secs) return ''
  const d = new Date(secs * 1000)
  d.setMinutes(d.getMinutes() - d.getTimezoneOffset())
  return d.toISOString().slice(0, 16)
}

function SaleModal({ sale, products, act, onClose }: { sale?: Sale; products: { id: string; name: string }[]; act: (path: string, body?: unknown) => Promise<boolean>; onClose: () => void }) {
  const [name, setName] = useState(sale?.name ?? '')
  const [repeat, setRepeat] = useState<Sale['repeat']>(sale?.repeat ?? 'yearly')
  const [percent, setPercent] = useState(String(sale?.percent ?? 20))
  const [items, setItems] = useState<Record<string, string>>(() => Object.fromEntries(Object.entries(sale?.items ?? {}).map(([k, v]) => [k, String(v)])))
  const [start, setStart] = useState(local(sale?.start ?? 0))
  const [end, setEnd] = useState(local(sale?.end ?? 0))
  const [month, setMonth] = useState(String(sale?.month || 11))
  const [day, setDay] = useState(String(sale?.day || 27))
  const [days, setDays] = useState(String(sale?.days || 4))
  const [busy, setBusy] = useState(false)
  const secs = (v: string) => (v ? Math.floor(new Date(v).getTime() / 1000) : 0)
  const save = async () => {
    setBusy(true)
    const body = {
      id: sale?.id,
      name: name.trim(),
      percent: Number(percent) || 0,
      items: Object.fromEntries(Object.entries(items).filter(([, v]) => Number(v) > 0).map(([k, v]) => [k, Number(v)])),
      repeat,
      ...(repeat === 'once' ? { start: secs(start), end: secs(end) } : { month: Number(month), day: Number(day), days: Number(days) })
    }
    const ok = await act('/api/payments/sale', body)
    setBusy(false)
    if (ok) onClose()
  }
  const field = (label: string, input: React.ReactNode) => (
    <label className="field">
      <span>{label}</span>
      {input}
    </label>
  )
  return (
    <Modal
      title={sale ? t('servers.admin.sales.editTitle', { name: sale.name }) : t('servers.admin.sales.add')}
      onClose={onClose}
      wide
      footer={
        <>
          <button className="btn" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={busy || !name.trim()} onClick={() => void save()}>
            {t('common.save')}
          </button>
        </>
      }
    >
      <div className="listing-fields">
        {field(t('servers.admin.sales.name'), <input className="input" autoFocus maxLength={40} value={name} placeholder="Black Friday" onChange={(e) => setName(e.target.value)} />)}
        <div className="field">
          <span>{t('servers.admin.sales.when')}</span>
          <Segmented<Sale['repeat']>
            value={repeat}
            onChange={setRepeat}
            options={[
              { value: 'yearly', label: t('servers.admin.sales.everyYear') },
              { value: 'monthly', label: t('servers.admin.sales.everyMonth') },
              { value: 'once', label: t('servers.admin.sales.oneTime') }
            ]}
          />
        </div>
        {repeat === 'once' ? (
          <div className="field-row">
            {field(t('servers.admin.sales.from'), <input className="input" type="datetime-local" value={start} onChange={(e) => setStart(e.target.value)} />)}
            {field(t('servers.admin.sales.to'), <input className="input" type="datetime-local" value={end} onChange={(e) => setEnd(e.target.value)} />)}
          </div>
        ) : (
          <div className="field-row">
            {repeat === 'yearly' &&
              field(
                t('servers.admin.sales.month'),
                <select className="input" value={month} onChange={(e) => setMonth(e.target.value)}>
                  {Array.from({ length: 12 }, (_, i) => (
                    <option key={i + 1} value={i + 1}>
                      {monthName(i + 1)}
                    </option>
                  ))}
                </select>
              )}
            {field(t('servers.admin.sales.day'), <input className="input" type="number" min={1} max={31} value={day} onChange={(e) => setDay(e.target.value)} />)}
            {field(t('servers.admin.sales.days'), <input className="input" type="number" min={1} max={repeat === 'monthly' ? 27 : 62} value={days} onChange={(e) => setDays(e.target.value)} />)}
          </div>
        )}
        <p className="hint">{t('servers.admin.sales.timeHint')}</p>
        {field(t('servers.admin.sales.percentAll'), <input className="input" type="number" min={0} max={90} value={percent} onChange={(e) => setPercent(e.target.value)} />)}
        <div className="field">
          <span>{t('servers.admin.sales.perItem')}</span>
          <div className="sale-items">
            {products.map((p) => (
              <label key={p.id} className="sale-item">
                <span className="grow ellipsis">{p.name}</span>
                <input
                  className="input"
                  type="number"
                  min={0}
                  max={90}
                  placeholder={percent || '0'}
                  value={items[p.id] ?? ''}
                  onChange={(e) => setItems({ ...items, [p.id]: e.target.value })}
                />
                <span className="hint">%</span>
              </label>
            ))}
          </div>
          <span className="hint">{t('servers.admin.sales.perItemHint')}</span>
        </div>
      </div>
    </Modal>
  )
}
