// Admin → Mail → Welcome mail: a list of up to 10 mails a new Microsoft account gets after its first sign-in, each
// with its own words, rewards (Resin Credit and/or Store items) and delay; the master switch, counts, a preview per
// mail, order, duplicate, remove (relay-server's `/api/welcome`, `/api/welcome/save|reset`; the relay dashboard's
// Mail page has the same). Also the rewards editor Mail.tsx's compose form shares.
import { useEffect, useState } from 'react'
import { AlertTriangle, ArrowDown, ArrowUp, ChevronDown, ChevronRight, Copy, Gift, Plus, RotateCcw, Save, Trash2, Undo2 } from 'lucide-react'
import { t, type Key } from '../../i18n'
import { ConfirmModal, Switch } from '../ui'
import MailText from '../MailText'
import { money } from '../../storeStatus'
import { Loading, Stat, useAct, useAdmin, when } from './common'

export type Category = '' | 'news' | 'update' | 'event' | 'account' | 'support' | 'status'
export type Reward = { kind: 'credit'; amount: number } | { kind: 'item'; product: string; days: number; name?: string }
export interface Product {
  id: string
  name: string
  kind: string
}

export const CATEGORY: Record<Exclude<Category, ''>, Key> = {
  news: 'inbox.category.news',
  update: 'inbox.category.update',
  event: 'inbox.category.event',
  account: 'inbox.category.account',
  support: 'inbox.category.support',
  status: 'inbox.category.status'
}

/** A mail's rewards: Resin Credit (typed in the currency, kept in its smallest unit) and Store items for some days. */
export function RewardsEditor({
  value,
  onChange,
  products,
  currency,
  max,
  maxDays
}: {
  value: Reward[]
  onChange: (v: Reward[]) => void
  products: Product[]
  currency: string
  max: number
  maxDays: number
}) {
  const set = (i: number, r: Reward | null) => onChange(r ? value.map((x, n) => (n === i ? r : x)) : value.filter((_, n) => n !== i))
  return (
    <>
      {value.map((r, i) => (
        <div key={i} className="row ib-admin-reward">
          {r.kind === 'credit' ? (
            <>
              <span className="chip compact success">{t('inbox.reward.credit')}</span>
              <input
                className="input"
                type="number"
                min={0.01}
                step={0.01}
                value={(r.amount / 100).toFixed(2)}
                onChange={(e) => set(i, { kind: 'credit', amount: Math.round(Number(e.target.value) * 100) })}
                aria-label={t('servers.admin.mail.amount')}
              />
              <span className="hint">{currency.toUpperCase()}</span>
            </>
          ) : (
            <>
              <span className="chip compact accent">{t('servers.admin.mail.item')}</span>
              <select className="input" value={r.product} onChange={(e) => set(i, { ...r, product: e.target.value })} aria-label={t('servers.admin.mail.item')}>
                {!products.some((p) => p.id === r.product) && <option value={r.product}>{r.name ?? r.product}</option>}
                {products.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name}
                  </option>
                ))}
              </select>
              <input
                className="input"
                type="number"
                min={0}
                max={maxDays}
                value={r.days}
                onChange={(e) => set(i, { ...r, days: Math.max(0, Math.round(Number(e.target.value))) })}
                aria-label={t('servers.admin.mail.days')}
              />
              <span className="hint">{t('servers.admin.mail.daysHint')}</span>
            </>
          )}
          <button type="button" className="btn sm ghost" onClick={() => set(i, null)} aria-label={t('common.remove')} title={t('common.remove')}>
            <Trash2 />
          </button>
        </div>
      ))}
      <div className="row">
        <button type="button" className="btn sm" disabled={value.length >= max} onClick={() => onChange([...value, { kind: 'credit', amount: 5000 }])}>
          <Plus /> {t('inbox.reward.credit')}
        </button>
        <button
          type="button"
          className="btn sm"
          disabled={value.length >= max || !products.length}
          onClick={() => onChange([...value, { kind: 'item', product: products[0]?.id ?? '', days: 30 }])}
        >
          <Gift /> {t('servers.admin.mail.item')}
        </button>
      </div>
    </>
  )
}

/** A reward as a chip's words: "40.00 THB" or "+1 server online · 7 days". */
export const rewardText = (r: Reward, currency: string, products: Product[]) =>
  r.kind === 'credit'
    ? money(r.amount, currency)
    : `${r.name ?? products.find((p) => p.id === r.product)?.name ?? r.product} · ${r.days ? t('inbox.reward.days', { days: r.days }) : t('inbox.reward.forGood')}`

interface Entry {
  id?: string
  enabled: boolean
  from: string
  subject: string
  body: string
  category: Category
  attachments: Reward[]
  delaySecs: number
  since?: number
  sent?: number
  claimed?: number
}
interface WelcomeData {
  enabled: boolean
  entries: Entry[]
  fromFile: boolean
  walletOn: boolean
  currency: string
  counts: { accounts: number; known: number; welcomed: number; mails: number; gifts: number; claimed: number; claimedCredit: number }
  products: Product[]
  categories: Exclude<Category, ''>[]
  defaults: { file?: Partial<Entry>; builtIn?: Partial<Entry> }
  limits: { subject: number; body: number; from: number; entries: number; delaySecs: number; credit: number; attachments?: number; days?: number }
}

type Unit = 'minutes' | 'hours' | 'days'
const UNIT_SECS: Record<Unit, number> = { minutes: 60, hours: 3600, days: 86400 }
const unitOf = (secs: number): Unit => (secs % 86400 === 0 ? 'days' : secs % 3600 === 0 ? 'hours' : 'minutes')
/** An entry being edited: `key` tells React which card is which, `unit` how its delay is typed. */
type Row = Entry & { key: string; unit: Unit }
let keys = 0
const row = (e: Entry): Row => ({
  ...e,
  category: e.category ?? '',
  attachments: e.attachments ?? [],
  delaySecs: e.delaySecs ?? 0,
  key: `w${++keys}`,
  unit: unitOf(e.delaySecs ?? 0)
})
/** What the relay takes for an entry (its counts and `since` are its own). */
const wire = (e: Entry) => ({
  ...(e.id ? { id: e.id } : {}),
  enabled: e.enabled,
  from: e.from,
  subject: e.subject,
  body: e.body,
  ...(e.category ? { category: e.category } : {}),
  attachments: e.attachments.map((a) => (a.kind === 'credit' ? { kind: 'credit', amount: a.amount } : { kind: 'item', product: a.product, days: a.days })),
  delaySecs: e.delaySecs
})
const creditOf = (e: Entry) => e.attachments.reduce((n, a) => n + (a.kind === 'credit' ? a.amount : 0), 0)

/** The body as a player sees it: `{name}` and `{credit}` filled in; lines with `{credit}` go when there's no credit. */
function render(body: string, name: string, credit: string | null) {
  return body
    .split('\n')
    .filter((l) => credit != null || !l.includes('{credit}'))
    .map((l) => l.replaceAll('{name}', name).replaceAll('{credit}', credit ?? ''))
    .join('\n')
}

/** "at once", "after 3 days". */
function delayText(secs: number) {
  if (!secs) return t('servers.admin.mail.welcome.atOnce')
  const u = unitOf(secs)
  return t(`servers.admin.mail.welcome.after.${u}`, { count: secs / UNIT_SECS[u] })
}

export function WelcomeAdmin() {
  const { data, error, reload } = useAdmin<WelcomeData>('/api/welcome')
  const act = useAct(reload)
  const [enabled, setEnabled] = useState(false)
  const [rows, setRows] = useState<Row[]>()
  const [open, setOpen] = useState<Set<string>>(new Set())
  const [removing, setRemoving] = useState<Row | null>(null)
  const [restoring, setRestoring] = useState<Row | null>(null)
  const [resetting, setResetting] = useState(false)
  // The form starts from the list in force, and again after a save or a reset.
  useEffect(() => {
    if (!data) return
    setEnabled(data.enabled)
    setRows(data.entries.map(row))
    setOpen(new Set())
  }, [data])
  if (!data || !rows) return <Loading error={error} />

  const c = data.counts
  const lim = data.limits
  const products = data.products ?? []
  const changed =
    enabled !== data.enabled ||
    JSON.stringify(rows.map(wire)) !== JSON.stringify(data.entries.map((e) => wire({ ...e, category: e.category ?? '', attachments: e.attachments ?? [] })))
  const problem = (e: Row) =>
    !e.from.trim() ||
    !e.subject.trim() ||
    !e.body.trim() ||
    e.delaySecs < 0 ||
    e.delaySecs > lim.delaySecs ||
    e.attachments.some((a) => (a.kind === 'credit' ? a.amount <= 0 || a.amount > lim.credit : !a.product))
  const ok = rows.length <= lim.entries && rows.every((e) => !problem(e))
  const update = (key: string, patch: Partial<Row>) => setRows(rows.map((e) => (e.key === key ? { ...e, ...patch } : e)))
  const move = (i: number, by: number) => {
    const next = [...rows]
    ;[next[i], next[i + by]] = [next[i + by], next[i]]
    setRows(next)
  }
  const insert = (at: number, e: Row) => {
    setRows([...rows.slice(0, at), e, ...rows.slice(at)])
    setOpen(new Set(open).add(e.key))
  }
  const toggle = (key: string) => {
    const next = new Set(open)
    if (next.has(key)) next.delete(key)
    else next.add(key)
    setOpen(next)
  }
  const fallback = data.defaults.file ?? data.defaults.builtIn
  const save = () => void act('/api/welcome/save', { enabled, entries: rows.map(wire) }, 'servers.admin.mail.welcome.saved')

  return (
    <div className="settings-section">
      <h3>
        <Gift className="inline" /> {t('servers.admin.mail.welcome.listTitle')}
      </h3>
      <p className="hint">{t('servers.admin.mail.welcome.listLead', { max: lim.entries })}</p>
      {!data.walletOn && (
        <p className="hint bad">
          <AlertTriangle className="inline" /> {t('servers.admin.mail.welcome.walletOffList')}
        </p>
      )}
      <div className="stats admin-stats">
        <Stat label={t('servers.admin.mail.welcome.welcomed')} value={c.welcomed} />
        <Stat label={t('servers.admin.mail.welcome.mailed')} value={c.mails} />
        <Stat label={t('servers.admin.mail.welcome.gifts')} value={c.gifts} />
        <Stat label={t('servers.admin.mail.welcome.claimed')} value={`${c.claimed} · ${money(c.claimedCredit, data.currency)}`} tone="ok" />
      </div>
      <form
        className="welcome-list"
        onSubmit={(e) => {
          e.preventDefault()
          if (ok && changed) save()
        }}
      >
        <div className="card">
          <div className="setting">
            <div className="text">
              <b>{t('servers.admin.mail.welcome.enabled')}</b>
              <span>{t('servers.admin.mail.welcome.onlyNew')}</span>
            </div>
            <Switch on={enabled} onChange={setEnabled} label={t('servers.admin.mail.welcome.enabled')} />
          </div>
        </div>

        {rows.length === 0 && <p className="hint">{t('servers.admin.mail.welcome.none')}</p>}
        {rows.map((e, i) => {
          const isOpen = open.has(e.key)
          const credit = creditOf(e)
          const gift = credit > 0 && data.walletOn ? money(credit, data.currency) : null
          const bad = problem(e)
          return (
            <div key={e.key} className={`card welcome-entry ${e.enabled && enabled ? '' : 'off'}`}>
              <div className="welcome-entry-head">
                <button
                  type="button"
                  className="btn sm ghost icon"
                  onClick={() => toggle(e.key)}
                  aria-expanded={isOpen}
                  aria-label={isOpen ? t('servers.admin.mail.welcome.collapse') : t('common.edit')}
                >
                  {isOpen ? <ChevronDown /> : <ChevronRight />}
                </button>
                <span className="welcome-entry-num">{i + 1}</span>
                <div className="text grow" onClick={() => toggle(e.key)}>
                  <b>{e.subject.trim() || t('servers.admin.mail.welcome.untitled')}</b>
                  <div className="row wrap welcome-entry-chips">
                    <span className="chip compact">{delayText(e.delaySecs)}</span>
                    {e.category && <span className="chip compact">{t(CATEGORY[e.category])}</span>}
                    {e.attachments.map((a, n) => (
                      <span key={n} className={`chip compact ${a.kind === 'credit' && !data.walletOn ? '' : 'accent'}`}>
                        <Gift className="inline" /> {rewardText(a, data.currency, products)}
                      </span>
                    ))}
                    {e.id ? (
                      <span className="chip compact">{t('servers.admin.mail.welcome.sentClaimed', { sent: e.sent ?? 0, claimed: e.claimed ?? 0 })}</span>
                    ) : (
                      <span className="chip compact warn">{t('servers.admin.mail.welcome.unsaved')}</span>
                    )}
                    {bad && <span className="chip compact danger">{t('servers.admin.mail.welcome.incomplete')}</span>}
                  </div>
                </div>
                <div className="row welcome-entry-actions">
                  <button
                    type="button"
                    className="btn sm ghost icon"
                    disabled={i === 0}
                    onClick={() => move(i, -1)}
                    title={t('servers.admin.mail.welcome.moveUp')}
                    aria-label={t('servers.admin.mail.welcome.moveUp')}
                  >
                    <ArrowUp />
                  </button>
                  <button
                    type="button"
                    className="btn sm ghost icon"
                    disabled={i === rows.length - 1}
                    onClick={() => move(i, 1)}
                    title={t('servers.admin.mail.welcome.moveDown')}
                    aria-label={t('servers.admin.mail.welcome.moveDown')}
                  >
                    <ArrowDown />
                  </button>
                  <button
                    type="button"
                    className="btn sm ghost icon"
                    disabled={rows.length >= lim.entries}
                    onClick={() => insert(i + 1, row({ ...e, id: undefined, sent: 0, claimed: 0, since: undefined }))}
                    title={t('servers.admin.mail.welcome.duplicate')}
                    aria-label={t('servers.admin.mail.welcome.duplicate')}
                  >
                    <Copy />
                  </button>
                  <button
                    type="button"
                    className="btn sm ghost icon danger"
                    onClick={() => setRemoving(e)}
                    title={t('common.remove')}
                    aria-label={t('common.remove')}
                  >
                    <Trash2 />
                  </button>
                  <Switch on={e.enabled} onChange={(v) => update(e.key, { enabled: v })} label={t('servers.admin.mail.welcome.entryOn')} />
                </div>
              </div>
              {isOpen && (
                <div className="setting stack welcome-entry-body">
                  <div className="listing-fields">
                    <div className="field-row">
                      <label className="field">
                        <span>{t('servers.admin.mail.from')}</span>
                        <input
                          className="input"
                          value={e.from}
                          maxLength={lim.from}
                          placeholder="ResinCraft"
                          onChange={(v) => update(e.key, { from: v.target.value })}
                        />
                      </label>
                      <label className="field">
                        <span>{t('servers.admin.mail.category')}</span>
                        <select className="input" value={e.category} onChange={(v) => update(e.key, { category: v.target.value as Category })}>
                          <option value="">{t('servers.admin.mail.noCategory')}</option>
                          {data.categories.map((k) => (
                            <option key={k} value={k}>
                              {t(CATEGORY[k])}
                            </option>
                          ))}
                        </select>
                      </label>
                    </div>
                    <label className="field">
                      <span>{t('servers.admin.mail.subject')}</span>
                      <input className="input" value={e.subject} maxLength={lim.subject} onChange={(v) => update(e.key, { subject: v.target.value })} />
                    </label>
                    <label className="field">
                      <span>{t('servers.admin.mail.message')}</span>
                      <textarea className="input" rows={8} value={e.body} maxLength={lim.body} onChange={(v) => update(e.key, { body: v.target.value })} />
                      <span className="hint">{t('servers.admin.mail.welcome.placeholders', { name: '{name}', credit: '{credit}' })}</span>
                    </label>
                    <div className="field">
                      <span>{t('servers.admin.mail.rewards')}</span>
                      <RewardsEditor
                        value={e.attachments}
                        onChange={(attachments) => update(e.key, { attachments })}
                        products={products}
                        currency={data.currency}
                        max={lim.attachments ?? 5}
                        maxDays={lim.days ?? 3650}
                      />
                    </div>
                    <div className="field">
                      <span>{t('servers.admin.mail.welcome.delay')}</span>
                      <div className="row ib-admin-reward welcome-delay">
                        <input
                          className="input"
                          type="number"
                          min={0}
                          value={e.delaySecs / UNIT_SECS[e.unit]}
                          onChange={(v) =>
                            update(e.key, { delaySecs: Math.min(lim.delaySecs, Math.max(0, Math.round(Number(v.target.value) * UNIT_SECS[e.unit]))) })
                          }
                          aria-label={t('servers.admin.mail.welcome.delay')}
                        />
                        <select
                          className="input"
                          value={e.unit}
                          onChange={(v) => {
                            const unit = v.target.value as Unit
                            update(e.key, { unit, delaySecs: Math.min(lim.delaySecs, Math.round((e.delaySecs / UNIT_SECS[e.unit]) * UNIT_SECS[unit])) })
                          }}
                          aria-label={t('servers.admin.mail.welcome.unit')}
                        >
                          <option value="minutes">{t('servers.admin.mail.welcome.unit.minutes')}</option>
                          <option value="hours">{t('servers.admin.mail.welcome.unit.hours')}</option>
                          <option value="days">{t('servers.admin.mail.welcome.unit.days')}</option>
                        </select>
                        <span className="hint">{t('servers.admin.mail.welcome.delayHint')}</span>
                      </div>
                    </div>
                    <div className="card ib-admin-preview">
                      <div className="hint">
                        {t('servers.admin.mail.preview')} · {e.from.trim() || 'ResinCraft'}
                        {e.category && ` · ${t(CATEGORY[e.category])}`}
                      </div>
                      <b>{e.subject.trim() || t('servers.admin.mail.subject')}</b>
                      <MailText text={render(e.body, 'Steve', gift)} />
                      {e.attachments.some((a) => a.kind === 'item' || data.walletOn) && (
                        <div className="row wrap" style={{ gap: 6 }}>
                          {e.attachments
                            .filter((a) => a.kind === 'item' || data.walletOn)
                            .map((a, n) => (
                              <span key={n} className="chip compact success">
                                <Gift className="inline" /> {rewardText(a, data.currency, products)}
                              </span>
                            ))}
                        </div>
                      )}
                    </div>
                    <div className="row">
                      <span className="hint grow">
                        {e.id && e.since
                          ? t('servers.admin.mail.welcome.since', { when: when(e.since) })
                          : e.id
                            ? t('servers.admin.mail.welcome.sinceAlways')
                            : t('servers.admin.mail.welcome.sinceSave')}
                      </span>
                      {fallback && (
                        <button type="button" className="btn sm ghost" onClick={() => setRestoring(e)}>
                          <Undo2 /> {t('servers.admin.mail.welcome.restore')}
                        </button>
                      )}
                    </div>
                  </div>
                </div>
              )}
            </div>
          )
        })}

        <div className="row">
          <button
            type="button"
            className="btn sm"
            disabled={rows.length >= lim.entries}
            onClick={() =>
              insert(rows.length, row({ enabled: true, from: 'ResinCraft', subject: '', body: '', category: 'account', attachments: [], delaySecs: 86400 }))
            }
          >
            <Plus /> {t('servers.admin.mail.welcome.add')}
          </button>
          <span className="hint grow">{t('servers.admin.mail.welcome.count', { count: rows.length, max: lim.entries })}</span>
          {data.fromFile ? (
            <span className="hint">{t('servers.admin.mail.welcome.fromFile')}</span>
          ) : (
            <button type="button" className="btn sm" onClick={() => setResetting(true)}>
              <RotateCcw /> {t('servers.admin.mail.welcome.reset')}
            </button>
          )}
          {changed && (
            <button type="button" className="btn sm ghost" onClick={() => void reload()}>
              {t('servers.admin.mail.welcome.discard')}
            </button>
          )}
          <button className="btn sm primary" type="submit" disabled={!ok || !changed}>
            <Save /> {t('common.save')}
          </button>
        </div>
      </form>

      {removing && (
        <ConfirmModal
          title={t('servers.admin.mail.welcome.removeTitle')}
          body={t('servers.admin.mail.welcome.removeBody', { subject: removing.subject.trim() || t('servers.admin.mail.welcome.untitled') })}
          confirm={t('common.remove')}
          onConfirm={() => setRows(rows.filter((x) => x.key !== removing.key))}
          onClose={() => setRemoving(null)}
        />
      )}
      {restoring && fallback && (
        <ConfirmModal
          title={t('servers.admin.mail.welcome.restore')}
          body={t('servers.admin.mail.welcome.restoreBody')}
          confirm={t('servers.admin.mail.welcome.restore')}
          onConfirm={() => update(restoring.key, { from: fallback.from ?? 'ResinCraft', subject: fallback.subject ?? '', body: fallback.body ?? '' })}
          onClose={() => setRestoring(null)}
        />
      )}
      {resetting && (
        <ConfirmModal
          title={t('servers.admin.mail.welcome.reset')}
          body={t('servers.admin.mail.welcome.resetBody')}
          confirm={t('servers.admin.mail.welcome.reset')}
          onConfirm={async () => void (await act('/api/welcome/reset', {}, 'servers.admin.mail.welcome.resetDone'))}
          onClose={() => setResetting(false)}
        />
      )}
    </div>
  )
}
