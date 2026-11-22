// Admin → Payments → Resin Credit: the Store's wallet, as the relay dashboard's Resin Credit page has it. The switch,
// the top-up amounts, what's outstanding, every balance and the newest ledger entries, and one account opened: its
// ledger, giving or taking credit with a note, and refunding its purchases as credit (never money).
// Routes: GET /api/payments/wallet[?uuid=|?name=], POST /api/payments/wallet/{switch,topups,grant,deduct,refund}.
import { useEffect, useState } from 'react'
import { Minus, Plus, RotateCcw, Search, Wallet, X } from 'lucide-react'
import { currentLanguage, t, type Key } from '../../i18n'
import { useStore } from '../../store'
import { ConfirmModal, Empty, Modal, Switch } from '../ui'
import { account, ago, Loading, shortId, Stat, useAct, useAdmin, when } from './common'
import '../../resin-credit.css'

interface Entry {
  id: number
  at: number
  kind: string
  amount: number
  balanceAfter: number
  ref?: string
  note?: string
  by?: string
  /** In `recent`: whose. */
  uuid?: string
  name?: string
}
interface Purchase {
  id: string
  name: string
  source: string
  method?: string
  yearly: boolean
  status: string
  amount: number
  active: boolean
  periodEnd?: number
  created?: number
  /** What its days left are worth (running ones). */
  unused?: number
  refunded?: number
}
interface WalletData {
  enabled: boolean
  switchFromFile: boolean
  live: boolean
  currency: string
  minCharge: number
  topups: number[]
  topupsFromFile: boolean
  fileTopups: number[]
  /** Cancelling a running purchase gives its unused time back as credit (refundable products only). */
  refundOnCancel?: boolean
  refundOnCancelFromFile?: boolean
  /** Players may type their own top-up, from `customMin` to `customMax`; card top-ups start at `cardMin`; an unpaid
   *  top-up is cancelled after `topupExpireMinutes`. Missing on relays from before these. */
  customTopup?: boolean
  customMin?: number
  customMax?: number
  cardMin?: number
  topupExpireMinutes?: number
  customFromFile?: boolean
  methods: string[]
  outstanding: number
  accounts: number
  pending: number
  balances: { uuid: string; name?: string; balance: number; updated: number }[]
  recent: Entry[]
  account?: {
    uuid: string
    name?: string
    balance: number
    history: Entry[]
    older?: { entries: number; net: number; from: number; until: number } | null
    purchases?: Purchase[]
  }
}

type Who = { uuid: string } | { name: string }

const KINDS: Record<string, Key> = {
  topup: 'servers.wallet.kind.topup',
  purchase: 'servers.wallet.kind.purchase',
  refund: 'servers.wallet.kind.refund',
  grant: 'servers.wallet.kind.grant',
  deduct: 'servers.wallet.kind.deduct',
  gift: 'servers.wallet.kind.gift'
}

function money(amount: number, currency: string) {
  try {
    return new Intl.NumberFormat(currentLanguage(), { style: 'currency', currency: currency.toUpperCase(), maximumFractionDigits: amount % 100 ? 2 : 0 }).format(amount / 100)
  } catch {
    return `${(amount / 100).toLocaleString()} ${currency.toUpperCase()}`
  }
}
const signed = (n: number, cur: string) => `${n > 0 ? '+' : n < 0 ? '−' : ''}${money(Math.abs(n), cur)}`
/** What the admin types (100 or 100.50, in the currency) in the smallest unit the relay keeps (10000). */
function toMinor(v: string): number | null {
  const n = Number(v.replace(/,/g, '').trim())
  return Number.isFinite(n) && n > 0 ? Math.round(n * 100) : null
}

/** Players' own top-up amounts, the card minimum and how long an unpaid top-up waits (`/api/payments/wallet/custom`). */
function TopupRules({ d, act }: { d: WalletData; act: ReturnType<typeof useAct> }) {
  const cur = d.currency
  const baht = (n?: number) => (n ? String(n / 100) : '')
  const [f, setF] = useState({ min: baht(d.customMin), max: baht(d.customMax), card: baht(d.cardMin), expire: String(d.topupExpireMinutes ?? 10) })
  useEffect(
    () => setF({ min: baht(d.customMin), max: baht(d.customMax), card: baht(d.cardMin), expire: String(d.topupExpireMinutes ?? 10) }),
    [d.customMin, d.customMax, d.cardMin, d.topupExpireMinutes]
  )
  const num = (k: keyof typeof f, label: Key) => (
    <label className="field">
      <span>{t(label, { currency: cur.toUpperCase() })}</span>
      <input className="input" inputMode="numeric" value={f[k]} onChange={(e) => setF({ ...f, [k]: e.target.value.replace(/[^\d]/g, '') })} />
    </label>
  )
  const save = () => {
    const min = toMinor(f.min)
    const max = toMinor(f.max)
    const card = toMinor(f.card)
    const expire = Number(f.expire)
    if (!min || !max || !card || min > max || !(expire >= 1 && expire <= 60)) return void act('/api/payments/wallet/custom', { min: min ?? 0, max: max ?? 0, cardMin: card ?? 0, topupExpireMinutes: expire })
    void act('/api/payments/wallet/custom', { min, max, cardMin: card, topupExpireMinutes: expire }, 'servers.admin.pay.saved')
  }
  return (
    <div className="setting stack">
      <div className="row">
        <div className="text grow">
          <b>{t('servers.admin.wallet.customTopup')}</b>
          <span>
            {d.customTopup
              ? t('servers.admin.wallet.customTopupOn', { min: money(d.customMin ?? 0, cur), max: money(d.customMax ?? 0, cur) })
              : t('servers.admin.wallet.customTopupOff')}
          </span>
          {d.customFromFile && <span className="hint">{t('servers.admin.wallet.customFromFile')}</span>}
        </div>
        {!d.customFromFile && (
          <button className="btn sm" onClick={() => void act('/api/payments/wallet/custom', { reset: true }, 'servers.admin.pay.saved')}>
            {t('servers.admin.set.reset')}
          </button>
        )}
        <Switch on={!!d.customTopup} label={t('servers.admin.wallet.customTopup')} onChange={(on) => void act('/api/payments/wallet/custom', { on }, 'servers.admin.pay.saved')} />
      </div>
      <div className="field-row">
        {num('min', 'servers.admin.wallet.customMin')}
        {num('max', 'servers.admin.wallet.customMax')}
        {num('card', 'servers.admin.wallet.cardMin')}
        {num('expire', 'servers.admin.wallet.expireMinutes')}
      </div>
      <div className="row end">
        <button className="btn sm primary" onClick={save}>
          {t('common.save')}
        </button>
      </div>
    </div>
  )
}

/** Admin → Payments → Resin Credit. `open`: an account to open first (Refund as credit on Purchases), and the
 * purchase whose refund to ask about. */
export function WalletAdmin({ open }: { open?: { uuid: string; refund?: string } }) {
  const { toast } = useStore()
  const [who, setWho] = useState<Who | null>(open ? { uuid: open.uuid } : null)
  const path = who ? `/api/payments/wallet?${'uuid' in who ? `uuid=${encodeURIComponent(who.uuid)}` : `name=${encodeURIComponent(who.name)}`}` : '/api/payments/wallet'
  const { data, error, reload } = useAdmin<WalletData>(path)
  const act = useAct(reload)
  const [turningOff, setTurningOff] = useState(false)
  const [amounts, setAmounts] = useState('')
  const [look, setLook] = useState('')
  useEffect(() => setAmounts((data?.topups ?? []).map((n) => n / 100).join(', ')), [data?.topups])
  // A name the relay hasn't seen: the page without it.
  useEffect(() => {
    if (error && who) {
      toast(error, 'error')
      setWho(null)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [error])
  if (!data) return <Loading error={error} />
  const cur = data.currency
  const methods = data.methods.map((m) => t(`servers.wallet.method.${m}` as Key)).join(', ')

  const saveAmounts = () => {
    const list = amounts.split(/[,\s]+/).filter(Boolean).map(toMinor)
    if (!list.length || list.some((n) => !n)) return toast(t('servers.admin.wallet.amountsBad', { currency: cur.toUpperCase() }), 'error')
    void act('/api/payments/wallet/topups', { amounts: list }, 'servers.admin.wallet.amountsSaved')
  }

  return (
    <div className="rc-admin">
      <div className="stats admin-stats">
        <Stat label={t('servers.admin.wallet.outstanding')} value={money(data.outstanding, cur)} />
        <Stat label={t('servers.admin.wallet.accounts')} value={data.accounts} />
        <Stat label={t('servers.admin.wallet.pending')} value={data.pending} tone={data.pending ? 'warn' : undefined} />
        <Stat label={t('servers.wallet.title')} value={t(data.enabled ? 'servers.admin.wallet.on' : 'servers.admin.wallet.off')} tone={data.enabled ? 'ok' : 'warn'} />
      </div>

      {data.account && <AccountView d={data} act={act} refund={open?.refund} onClose={() => setWho(null)} />}

      <div className="settings-section">
        <h3>{t('servers.wallet.title')}</h3>
        <div className="card">
          <div className="setting">
            <div className="text">
              <b>{t('servers.admin.wallet.switch')}</b>
              <span>{t(data.enabled ? 'servers.admin.wallet.onHint' : 'servers.admin.wallet.offHint')}</span>
            </div>
            <Switch
              on={data.enabled}
              label={t('servers.admin.wallet.switch')}
              onChange={(on) => (on ? void act('/api/payments/wallet/switch', { on: true }, 'servers.admin.wallet.switchedOn') : setTurningOff(true))}
            />
          </div>
          {data.enabled && !data.live && (
            <div className="setting">
              <span className="hint it-warn">{t('servers.admin.wallet.notLive')}</span>
            </div>
          )}
          {data.enabled && data.live && (
            <div className="setting">
              <span className="hint">{methods ? t('servers.admin.wallet.methods', { methods }) : t('servers.admin.wallet.noMethods')}</span>
            </div>
          )}
          {data.customTopup !== undefined && <TopupRules d={data} act={act} />}
          {data.refundOnCancel !== undefined && (
            <div className="setting">
              <div className="text">
                <b>{t('servers.admin.wallet.refundOnCancel')}</b>
                <span>{t(data.refundOnCancel ? 'servers.admin.wallet.refundOnCancelOn' : 'servers.admin.wallet.refundOnCancelOff')}</span>
                {data.refundOnCancelFromFile && <span className="hint">{t('servers.admin.wallet.refundOnCancelFromFile')}</span>}
              </div>
              {!data.refundOnCancelFromFile && (
                <button className="btn sm" onClick={() => void act('/api/payments/wallet/refunds', { reset: true }, 'servers.admin.pay.saved')}>
                  {t('servers.admin.set.reset')}
                </button>
              )}
              <Switch
                on={data.refundOnCancel}
                label={t('servers.admin.wallet.refundOnCancel')}
                onChange={(on) => void act('/api/payments/wallet/refunds', { on }, 'servers.admin.pay.saved')}
              />
            </div>
          )}
          <div className="setting">
            {data.switchFromFile ? (
              <span className="hint">{t('servers.admin.wallet.switchFromFile')}</span>
            ) : (
              <>
                <span className="hint grow">{t('servers.admin.wallet.switchHere')}</span>
                <button className="btn sm" onClick={() => void act('/api/payments/wallet/switch', { reset: true }, 'servers.admin.pay.saved')}>
                  {t('servers.admin.set.reset')}
                </button>
              </>
            )}
          </div>
        </div>
      </div>

      <div className="settings-section">
        <h3>{t('servers.admin.wallet.topups')}</h3>
        <div className="card">
          <div className="setting stack">
            <span className="hint">{t('servers.admin.wallet.topupsHint', { currency: cur.toUpperCase(), min: money(data.minCharge, cur) })}</span>
            <form
              className="rc-admin-form"
              onSubmit={(e) => {
                e.preventDefault()
                saveAmounts()
              }}
            >
              <input
                className="input mono"
                value={amounts}
                inputMode="decimal"
                spellCheck={false}
                placeholder="100, 200, 500, 1000"
                aria-label={t('servers.admin.wallet.topups')}
                onChange={(e) => setAmounts(e.target.value)}
              />
              <button className="btn sm primary" type="submit">
                {t('servers.admin.set.save')}
              </button>
              {!data.topupsFromFile && (
                <button className="btn sm" type="button" onClick={() => void act('/api/payments/wallet/topups', { reset: true }, 'servers.admin.pay.saved')}>
                  {t('servers.admin.set.reset')}
                </button>
              )}
            </form>
            <div className="rc-chips">
              {data.topups.map((n) => (
                <span key={n} className="chip">
                  {money(n, cur)}
                </span>
              ))}
            </div>
            <span className="hint">
              {data.topupsFromFile
                ? t('servers.admin.wallet.topupsFromFile')
                : t('servers.admin.wallet.topupsFile', { amounts: data.fileTopups.map((n) => money(n, cur)).join(', ') || '–' })}
            </span>
          </div>
        </div>
      </div>

      <div className="settings-section">
        <h3>{t('servers.admin.wallet.find')}</h3>
        <form
          className="rc-admin-form"
          onSubmit={(e) => {
            e.preventDefault()
            if (look.trim()) setWho(account(look) as Who)
          }}
        >
          <input className="input" spellCheck={false} value={look} placeholder={t('servers.admin.pay.whoPlaceholder')} onChange={(e) => setLook(e.target.value)} />
          <button className="btn sm" type="submit" disabled={!look.trim()}>
            <Search /> {t('servers.admin.wallet.open')}
          </button>
        </form>
      </div>

      <div className="settings-section">
        <h3>{t('servers.admin.wallet.balances')}</h3>
        {data.balances.length === 0 ? (
          <Empty icon={<Wallet />} title={t('servers.admin.wallet.noBalances')} />
        ) : (
          <div className="card admin-items">
            {data.balances.map((b) => (
              <div key={b.uuid} className="setting admin-item">
                <div className="text">
                  <b>
                    <button className="rc-link" onClick={() => setWho({ uuid: b.uuid })}>
                      {b.name || shortId(b.uuid)}
                    </button>
                  </b>
                  <span className="hint mono">{b.uuid}</span>
                </div>
                <div className="rc-entry-amount">
                  <b>{money(b.balance, cur)}</b>
                  <span className="hint" title={when(b.updated)}>
                    {ago(b.updated)}
                  </span>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="settings-section">
        <h3>{t('servers.admin.wallet.recent')}</h3>
        {data.recent.length === 0 ? (
          <Empty icon={<Wallet />} title={t('servers.admin.wallet.noChanges')} />
        ) : (
          <div className="card">
            <Ledger entries={data.recent} cur={cur} onOpen={(uuid) => setWho({ uuid })} />
          </div>
        )}
      </div>

      {turningOff && (
        <ConfirmModal
          title={t('servers.admin.wallet.offTitle')}
          body={t('servers.admin.wallet.offBody')}
          confirm={t('servers.admin.wallet.turnOff')}
          onConfirm={() => void act('/api/payments/wallet/switch', { on: false }, 'servers.admin.wallet.switchedOff')}
          onClose={() => setTurningOff(false)}
        />
      )}
    </div>
  )
}

type Act = ReturnType<typeof useAct>

/** Ledger entries: what, whose (with `onOpen`), the signed amount, the balance after, the note and its ref, who, when. */
function Ledger({ entries, cur, onOpen }: { entries: Entry[]; cur: string; onOpen?: (uuid: string) => void }) {
  return (
    <div className="rc-ledger">
      {entries.map((e) => (
        <div key={e.id} className="rc-entry">
          <div className="rc-entry-text">
            <b>
              {KINDS[e.kind] ? t(KINDS[e.kind]) : e.kind}
              {onOpen && e.uuid && (
                <span className="rc-entry-who">
                  {' · '}
                  <button className="rc-link" onClick={() => onOpen(e.uuid!)}>{e.name || shortId(e.uuid)}</button>
                </span>
              )}
            </b>
            {e.note && <span className="hint rc-entry-note">{e.note}</span>}
            <span className="hint">
              <span title={when(e.at)}>{ago(e.at)}</span>
              {e.by ? ` · ${e.by}` : ''}
              {e.ref ? <span className="mono"> · {e.ref}</span> : null}
            </span>
          </div>
          <div className="rc-entry-amount">
            <b className={e.amount < 0 ? 'rc-out' : 'rc-in'}>{signed(e.amount, cur)}</b>
            <span className="hint">{t('servers.wallet.after', { balance: money(e.balanceAfter, cur) })}</span>
          </div>
        </div>
      ))}
    </div>
  )
}

/** One account opened: balance, give or take credit, its ledger and its purchases (Refund as credit). */
function AccountView({ d, act, refund, onClose }: { d: WalletData; act: Act; refund?: string; onClose: () => void }) {
  const { toast } = useStore()
  const a = d.account!
  const cur = d.currency
  const who = a.name || shortId(a.uuid)
  const [amount, setAmount] = useState('')
  const [note, setNote] = useState('')
  const [taking, setTaking] = useState<number>()
  const [refunding, setRefunding] = useState<Purchase | undefined>(() => a.purchases?.find((p) => p.id === refund && !p.refunded))
  const change = async (kind: 'grant' | 'deduct', n: number) => {
    const ok = await act(`/api/payments/wallet/${kind}`, { uuid: a.uuid, amount: n, ...(note.trim() ? { note: note.trim() } : {}) }, kind === 'grant' ? 'servers.admin.wallet.given' : 'servers.admin.wallet.taken', {
      amount: money(n, cur),
      name: who
    })
    if (ok) {
      setAmount('')
      setNote('')
    }
  }
  const typed = () => {
    const n = toMinor(amount)
    if (!n) toast(t('servers.admin.wallet.amountBad', { currency: cur.toUpperCase() }), 'error')
    return n
  }
  return (
    <div className="settings-section">
      <div className="row">
        <h3 className="grow">{t('servers.admin.wallet.accountTitle', { name: who })}</h3>
        <button className="btn sm ghost" onClick={onClose}>
          <X /> {t('common.close')}
        </button>
      </div>
      <div className="card rc-admin-account">
        <div className="setting stack">
          <div className="stats admin-stats">
            <Stat label={t('servers.wallet.balance')} value={money(a.balance, cur)} tone={a.balance ? 'ok' : undefined} />
            <Stat label={t('servers.admin.wallet.entries')} value={a.history.length + (a.older?.entries ?? 0)} />
          </div>
          <span className="hint mono">{a.uuid}</span>
          <form
            className="rc-admin-form"
            onSubmit={(e) => {
              e.preventDefault()
              const n = typed()
              if (n) void change('grant', n)
            }}
          >
            <input
              className="input rc-amount-input"
              inputMode="decimal"
              value={amount}
              placeholder={t('servers.admin.wallet.amountIn', { currency: cur.toUpperCase() })}
              aria-label={t('servers.admin.wallet.amountIn', { currency: cur.toUpperCase() })}
              onChange={(e) => setAmount(e.target.value)}
            />
            <input
              className="input"
              maxLength={200}
              value={note}
              placeholder={t('servers.admin.wallet.notePlaceholder')}
              aria-label={t('servers.admin.pay.f.note')}
              onChange={(e) => setNote(e.target.value)}
            />
            <button className="btn sm primary" type="submit">
              <Plus /> {t('servers.admin.wallet.give')}
            </button>
            <button
              className="btn sm"
              type="button"
              onClick={() => {
                const n = typed()
                if (n) setTaking(n)
              }}
            >
              <Minus /> {t('servers.admin.wallet.take')}
            </button>
          </form>
        </div>
      </div>

      <h4>{t('servers.admin.wallet.ledger')}</h4>
      {a.history.length === 0 ? (
        <p className="hint">{t('servers.admin.wallet.noChanges')}</p>
      ) : (
        <div className="card">
          <Ledger entries={a.history} cur={cur} />
        </div>
      )}
      {a.older && (
        <p className="hint">
          {t('servers.admin.wallet.older', { count: a.older.entries, from: when(a.older.from), until: when(a.older.until), net: signed(a.older.net, cur) })}
        </p>
      )}

      <h4>{t('servers.admin.wallet.purchases')}</h4>
      {(a.purchases ?? []).length === 0 ? (
        <p className="hint">{t('servers.admin.pay.noPurchases')}</p>
      ) : (
        <div className="card admin-items">
          {a.purchases!.map((p) => (
            <div key={p.id} className="setting admin-item">
              <div className="text">
                <b>
                  {p.name}
                  <span className={`chip compact ${p.active ? 'success' : 'danger'}`}>{p.active ? t('servers.relayPlan.active') : p.status}</span>
                  {(p.refunded ?? 0) > 0 && <span className="chip compact">{t('servers.admin.wallet.refundedChip', { amount: money(p.refunded!, cur) })}</span>}
                </b>
                <span>
                  {t(p.yearly ? 'servers.admin.pay.yearly' : 'servers.admin.pay.monthly')} · {money(p.amount, cur)} · {p.method ? t(`servers.relayPlan.method.${p.method}` as Key) : p.source}
                  {p.active && p.unused ? ` · ${t('servers.admin.wallet.unused', { amount: money(p.unused, cur) })}` : ''}
                </span>
                <span className="hint">
                  {p.periodEnd ? t('servers.admin.pay.periodEnds', { when: when(p.periodEnd) }) : t('servers.admin.pay.neverEnds')} · <span className="mono">{p.id}</span>
                </span>
              </div>
              {!p.refunded && (
                <button className="btn sm" onClick={() => setRefunding(p)}>
                  <RotateCcw /> {t('servers.admin.wallet.refund')}
                </button>
              )}
            </div>
          ))}
        </div>
      )}

      {taking != null && (
        <ConfirmModal
          title={t('servers.admin.wallet.takeTitle', { amount: money(taking, cur), name: who })}
          body={t('servers.admin.wallet.takeBody', { balance: money(a.balance, cur) })}
          confirm={t('servers.admin.wallet.take')}
          onConfirm={() => change('deduct', taking)}
          onClose={() => setTaking(undefined)}
        />
      )}
      {refunding && <RefundModal p={refunding} cur={cur} who={who} act={act} onClose={() => setRefunding(undefined)} />}
    </div>
  )
}

/** Refund as credit: ends the purchase now and gives credit back (never money); the amount starts at the relay's
 * suggestion (the unused part of a running purchase, else what was paid). */
function RefundModal({ p, cur, who, act, onClose }: { p: Purchase; cur: string; who: string; act: Act; onClose: () => void }) {
  const suggested = p.active && p.unused ? p.unused : p.amount
  const [amount, setAmount] = useState((suggested / 100).toFixed(2))
  const [note, setNote] = useState(t('servers.admin.wallet.refundNote', { name: p.name }))
  const [busy, setBusy] = useState(false)
  const n = toMinor(amount)
  const go = async () => {
    if (!n) return
    setBusy(true)
    const ok = await act('/api/payments/wallet/refund', { id: p.id, amount: n, ...(note.trim() ? { note: note.trim() } : {}) }, 'servers.admin.wallet.refunded', {
      amount: money(n, cur),
      name: who
    })
    setBusy(false)
    if (ok) onClose()
  }
  return (
    <Modal
      title={t('servers.admin.wallet.refundTitle', { name: p.name })}
      subtitle={t('servers.admin.wallet.refundHint')}
      onClose={onClose}
      footer={
        <>
          <button className="btn" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={busy || !n} onClick={() => void go()}>
            <RotateCcw /> {n ? t('servers.admin.wallet.refundAmount', { amount: money(n, cur) }) : t('servers.admin.wallet.refund')}
          </button>
        </>
      }
    >
      <p className="hint">
        {p.active
          ? t('servers.admin.wallet.refundPaidUnused', { paid: money(p.amount, cur), unused: money(p.unused ?? 0, cur) })
          : t('servers.admin.wallet.refundPaid', { paid: money(p.amount, cur) })}
      </p>
      <div className="listing-fields">
        <label className="field">
          <span>{t('servers.admin.wallet.amountIn', { currency: cur.toUpperCase() })}</span>
          <input className="input" autoFocus inputMode="decimal" value={amount} onChange={(e) => setAmount(e.target.value)} />
        </label>
        <label className="field">
          <span>{t('servers.admin.pay.f.note')}</span>
          <input className="input" maxLength={200} value={note} onChange={(e) => setNote(e.target.value)} />
        </label>
      </div>
    </Modal>
  )
}
