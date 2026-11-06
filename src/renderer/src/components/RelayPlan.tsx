// The Servers tab's ResinCraft Relay plan: what each signed-in Microsoft account has, the add-ons and packs the relay
// sells, and buying them with the launcher's own payment form (Stripe's card fields inside it, no Stripe page).
// While the relay's admin has payments off, nothing is for sale and this shows nothing.
import { useCallback, useEffect, useRef, useState } from 'react'
import {
  Check,
  ChevronDown,
  ChevronUp,
  CreditCard,
  Gift,
  ListOrdered,
  Loader2,
  Lock,
  Package,
  Plus,
  Puzzle,
  RefreshCw,
  Repeat,
  ShoppingCart,
  Trash2,
  X
} from 'lucide-react'
import type { AccountPlan, PayStep, PlanInfo, PlanProduct, PlanQuote, PlanSub, RelayPlans } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { t, tx, type Key } from '../i18n'
import { ConfirmModal, Modal } from './ui'
import { loadStripe, stripeAppearance, type StripeJs } from '../stripe'
import '../relay-plan.css'

function money(amount: number, currency: string) {
  try {
    return new Intl.NumberFormat(undefined, { style: 'currency', currency: currency.toUpperCase(), maximumFractionDigits: amount % 100 ? 2 : 0 }).format(
      amount / 100
    )
  } catch {
    return `${(amount / 100).toLocaleString()} ${currency.toUpperCase()}`
  }
}

const gb = (bytes: number) => `${Math.round((bytes / 1e9) * 10) / 10} GB`
const day = (unix: number) => new Date(unix * 1000).toLocaleDateString()

/** What a product gives, in a few words. */
function gives(p: PlanProduct) {
  const g = p.grants
  const parts: string[] = []
  if (g.servers) parts.push(t('servers.relayPlan.giveServers', { count: g.servers }))
  if (g.names) parts.push(t('servers.relayPlan.giveNames', { count: g.names }))
  if (g.traffic_gb) parts.push(t('servers.relayPlan.giveTraffic', { amount: `${g.traffic_gb} GB` }))
  if (g.unlimited_traffic) parts.push(t('servers.relayPlan.trafficUnlimited'))
  if (g.no_session_limit) parts.push(t('servers.relayPlan.sessionUnlimited'))
  if (g.whitelist_page) parts.push(t('servers.relayPlan.whitelistOn'))
  return parts.join(' · ')
}

function usePlans() {
  const [plans, setPlans] = useState<RelayPlans>()
  const load = useCallback(async () => {
    try {
      setPlans(await api.server.relayPlans())
    } catch {
      setPlans({ available: false, accounts: [] })
    }
  }, [])
  useEffect(() => void load(), [load])
  return { plans, setPlans, load }
}

/** Other signed-in accounts that already have what `p` gives (switches), or a subscription to it. */
function othersWith(accounts: AccountPlan[], me: string, p: PlanProduct) {
  return accounts
    .filter((a) => a.account !== me && a.plan)
    .filter((a) => a.plan!.mine.some((s) => s.active && s.product === p.id) || a.plan!.products.some((x) => x.id === p.id && x.owned))
    .map((a) => a.accountName)
}

/** The payment form: Stripe's Payment Element in the launcher's colours, confirmed without leaving the launcher. */
function PayModal({
  account,
  accountName,
  title,
  step,
  onClose,
  onPaid
}: {
  account: string
  accountName: string
  title: string
  step: PayStep
  onClose: () => void
  onPaid: (plan: PlanInfo) => void
}) {
  const box = useRef<HTMLDivElement>(null)
  const stripe = useRef<{ stripe: StripeJs; elements: StripeJs }>(undefined)
  const [ready, setReady] = useState(false)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string>()
  const mock = step.publishable_key === 'pk_mock'

  useEffect(() => {
    if (mock) {
      setReady(true)
      return
    }
    let gone = false
    loadStripe()
      .then((Stripe) => {
        if (gone || !box.current) return
        const s = Stripe(step.publishable_key)
        const elements = s.elements({ clientSecret: step.client_secret, appearance: stripeAppearance() })
        const pe = elements.create('payment', { layout: 'tabs' })
        pe.on('ready', () => setReady(true))
        pe.mount(box.current)
        stripe.current = { stripe: s, elements }
      })
      .catch((e: Error) => setError(t('servers.relayPlan.formFailed', { error: e.message })))
    return () => {
      gone = true
    }
  }, [step, mock])

  const pay = async () => {
    setBusy(true)
    setError(undefined)
    try {
      if (!mock) {
        const s = stripe.current!
        const confirm = step.intent === 'setup' ? s.stripe.confirmSetup : s.stripe.confirmPayment
        // Cards confirm here (3-D Secure opens in Stripe's own frame); return_url only for methods that must redirect.
        const r = await confirm.call(s.stripe, { elements: s.elements, confirmParams: { return_url: location.href }, redirect: 'if_required' })
        if (r.error) throw new Error(r.error.message)
      }
      // Stripe has the payment: read the subscription again until it's active (a few seconds at most).
      let plan = await api.server.relayPlanRefresh(account, step.sub)
      for (let i = 0; i < 6 && !plan.mine.some((m) => m.id === step.sub && m.active); i++) {
        await new Promise((r) => setTimeout(r, 1500))
        plan = await api.server.relayPlanRefresh(account, step.sub)
      }
      onPaid(plan)
    } catch (e) {
      setError((e as Error).message)
      setBusy(false)
    }
  }

  const price = money(step.amount, step.currency)
  const carried = step.carried ?? 0
  return (
    <Modal
      title={title}
      subtitle={t('servers.relayPlan.paySubtitle', { account: accountName, price: carried ? money(carried, step.currency) : price })}
      onClose={busy ? () => undefined : onClose}
      footer={
        <>
          <span className="hint grow rp-secure">
            <Lock /> {t('servers.relayPlan.secure')}
          </span>
          <button className="btn ghost" disabled={busy} onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={!ready || busy} onClick={() => void pay()}>
            {busy ? <Loader2 className="spin" /> : <CreditCard />}{' '}
            {busy ? t('servers.relayPlan.paying') : step.intent === 'setup' ? t('servers.relayPlan.saveCard') : t('servers.relayPlan.payFor', { price })}
          </button>
        </>
      }
    >
      {!ready && !error && (
        <p className="hint">
          <Loader2 className="spin" /> {t('servers.relayPlan.loadingForm')}
        </p>
      )}
      {(step.discount ?? 0) > 0 && (
        <p className="hint">
          {t('servers.relayPlan.discountLine', { price: money(step.subtotal ?? 0, step.currency), discount: money(step.discount ?? 0, step.currency) })}
        </p>
      )}
      {carried > 0 && (
        <p className="hint it-warn">
          {t('servers.relayPlan.carried', { amount: money(carried, step.currency), date: step.period_end ? day(step.period_end) : '–' })}
        </p>
      )}
      {mock ? <div className="rp-mock-card">4242 4242 4242 4242 · 12/34 · 123</div> : <div ref={box} className="rp-element" />}
      {error && <p className="hint it-warn">{error}</p>}
    </Modal>
  )
}

type PayWay = 'once' | 'renew' | 'angpao'

/** Checkout, for one product, the cart or a renewal: the price and promo discount of each item (from the relay), how
 * to pay (once by card or PromptPay, the default; a card subscription that renews; a TrueMoney angpao), then the
 * payment form. */
function CheckoutModal({
  a,
  items,
  yearly,
  renewOf,
  onClose,
  onPaid
}: {
  a: AccountPlan
  items: PlanProduct[]
  yearly: boolean
  /** Renewing this one-time purchase. */
  renewOf?: string
  onClose: () => void
  onPaid: (plan: PlanInfo) => void
}) {
  const plan = a.plan!
  const [promo, setPromo] = useState('')
  const [way, setWay] = useState<PayWay>('once')
  const [link, setLink] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string>()
  const [step, setStep] = useState<PayStep>()
  const [quote, setQuote] = useState<PlanQuote>()
  const ids = items.map((p) => p.id)
  const key = ids.join(',')
  const currency = plan.currency
  const title = renewOf
    ? t('servers.relayPlan.renewTitle', { name: items.map((p) => p.name).join(' + ') })
    : items.length === 1
      ? t('servers.relayPlan.payTitle', { name: items[0].name })
      : t('servers.relayPlan.checkoutTitle')

  // The quote again whenever the promo code changes (a moment after typing stops).
  useEffect(() => {
    let gone = false
    const timer = setTimeout(() => {
      api.server
        .relayPlanQuote(a.account, ids, yearly, promo.trim() || null)
        .then((q) => !gone && setQuote(q))
        .catch(() => {})
    }, 300)
    return () => {
      gone = true
      clearTimeout(timer)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [a.account, key, yearly, promo])

  if (step) return <PayModal account={a.account} accountName={a.accountName} title={title} step={step} onClose={onClose} onPaid={onPaid} />

  const go = async () => {
    setBusy(true)
    setError(undefined)
    try {
      const code = promo.trim() || null
      if (way === 'angpao') {
        onPaid(await api.server.relayPlanAngpao(a.account, renewOf ? [] : ids, yearly, link.trim(), code, renewOf ?? null))
        return
      }
      const s = await api.server.relayPlanCart(a.account, renewOf ? [] : ids, yearly, code, way === 'renew', renewOf ?? null)
      // Nothing to pay now (a 100% promo code): it's active already.
      if (s.client_secret) setStep(s)
      else onPaid(await api.server.relayPlanRefresh(a.account, s.sub))
    } catch (e) {
      setError((e as Error).message)
      setBusy(false)
    }
  }

  const per = (n: number) =>
    yearly ? t('servers.relayPlan.perYear', { price: money(n, currency) }) : t('servers.relayPlan.perMonth', { price: money(n, currency) })
  const fallback = items.reduce((n, p) => n + (yearly ? p.yearly : p.monthly), 0)
  const total = quote ? quote.total : fallback
  const below = quote && quote.total > 0 && quote.total < quote.min_charge
  // A renewal's items are the account's already: the cart's checks don't apply to it.
  const cartError = renewOf ? null : quote?.error
  const ways: { value: PayWay; label: string }[] = [
    { value: 'once', label: plan.promptpay ? t('servers.relayPlan.wayOnce') : t('servers.relayPlan.wayOnceCard') }
  ]
  if (!renewOf) ways.push({ value: 'renew', label: t('servers.relayPlan.wayRenew') })
  if (plan.truemoney) ways.push({ value: 'angpao', label: t('servers.relayPlan.wayAngpao') })
  const period = yearly ? t('servers.relayPlan.oneYear') : t('servers.relayPlan.oneMonth')
  const ready = !busy && !cartError && (way !== 'angpao' || link.trim().length > 8) && !(below && way === 'once')

  return (
    <Modal
      title={title}
      subtitle={t('servers.relayPlan.paySubtitle', { account: a.accountName, price: way === 'renew' ? per(total) : `${money(total, currency)} · ${period}` })}
      onClose={onClose}
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={!ready} onClick={() => void go()}>
            {busy ? <Loader2 className="spin" /> : way === 'angpao' ? <Gift /> : <CreditCard />}{' '}
            {way === 'angpao' ? t('servers.relayPlan.angpaoPay') : t('servers.relayPlan.continue')}
          </button>
        </>
      }
    >
      <div className="rp-lines">
        {(quote?.lines ?? items.map((p) => ({ product: p.id, name: p.name, price: yearly ? p.yearly : p.monthly, discount: 0 }))).map((l) => (
          <div key={l.product} className="rp-line">
            <span className="grow">{l.name}</span>
            {l.discount > 0 && <span className="rp-was">{money(l.price, currency)}</span>}
            <b>{money(l.price - l.discount, currency)}</b>
          </div>
        ))}
        {quote && quote.discount > 0 && (
          <div className="rp-line rp-total">
            <span className="grow">{t('servers.relayPlan.discount')}</span>
            <b>−{money(quote.discount, currency)}</b>
          </div>
        )}
        <div className="rp-line rp-total">
          <span className="grow">{t('servers.relayPlan.total')}</span>
          <b>{way === 'renew' ? per(total) : `${money(total, currency)} · ${period}`}</b>
        </div>
      </div>
      <div className="field">
        <label>{t('servers.relayPlan.payWith')}</label>
        <div className="segmented rp-ways">
          {ways.map((w) => (
            <button key={w.value} className={way === w.value ? 'on' : undefined} onClick={() => setWay(w.value)}>
              {w.label}
            </button>
          ))}
        </div>
        <span className="hint">
          {way === 'once'
            ? t('servers.relayPlan.wayOnceHint', { period })
            : way === 'renew'
              ? t('servers.relayPlan.wayRenewHint')
              : t('servers.relayPlan.angpaoHint', { price: money(total, currency) })}
        </span>
      </div>
      {way === 'angpao' && (
        <div className="field">
          <label htmlFor="rp-angpao">{t('servers.relayPlan.angpaoLink')}</label>
          <input
            id="rp-angpao"
            className="input mono"
            value={link}
            maxLength={200}
            spellCheck={false}
            placeholder="https://gift.truemoney.com/campaign/?v=…"
            onChange={(e) => setLink(e.target.value)}
          />
          <span className="hint it-warn">{t('servers.relayPlan.angpaoOver')}</span>
        </div>
      )}
      {items.length > 1 && way === 'renew' && <p className="hint">{t('servers.relayPlan.cartHint')}</p>}
      {below && (
        <p className="hint it-warn">
          {way === 'renew'
            ? t('servers.relayPlan.belowMin', { min: money(quote.min_charge, currency) })
            : t('servers.relayPlan.belowMinOnce', { min: money(quote.min_charge, currency) })}
        </p>
      )}
      {cartError && <p className="hint it-warn">{cartError}</p>}
      <div className="field">
        <label htmlFor="rp-promo">{t('servers.relayPlan.promo')}</label>
        <input
          id="rp-promo"
          className="input mono"
          value={promo}
          maxLength={40}
          spellCheck={false}
          placeholder={t('servers.relayPlan.promoPlaceholder')}
          onChange={(e) => setPromo(e.target.value.toUpperCase().replace(/[^A-Z0-9-]/g, ''))}
          onKeyDown={(e) => e.key === 'Enter' && ready && void go()}
        />
        {quote?.promo_error && <span className="hint it-warn">{quote.promo_error}</span>}
        {error && <span className="hint it-warn">{error}</span>}
      </div>
    </Modal>
  )
}

/** Server priority: the order the account's names keep ResinCraft Relay in when it has fewer slots than it uses. */
function Priority({ a, onSaved }: { a: AccountPlan; onSaved: (p: PlanInfo) => void }) {
  const { toast } = useStore()
  const access = a.plan!.access!
  const start = access.names.filter((n) => !n.assigned).map((n) => n.name)
  const [order, setOrder] = useState(start)
  const [busy, setBusy] = useState(false)
  useEffect(() => setOrder(access.names.filter((n) => !n.assigned).map((n) => n.name)), [access])
  if (start.length < 2) return null
  const move = (i: number, by: number) => {
    const next = [...order]
    const [x] = next.splice(i, 1)
    next.splice(i + by, 0, x)
    setOrder(next)
  }
  const changed = order.join(',') !== start.join(',')
  const save = async () => {
    setBusy(true)
    try {
      onSaved(await api.server.relayPlanPriority(a.account, order))
      toast(t('servers.relayPlan.prioritySaved'), 'success')
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(false)
    }
  }
  const info = (n: string) => access.names.find((x) => x.name === n)
  return (
    <>
      <h4>
        <ListOrdered /> {t('servers.relayPlan.priority')}
      </h4>
      <p className="hint">{access.cutoff ? t('servers.relayPlan.priorityHint', { days: access.grace_days }) : t('servers.relayPlan.priorityOff')}</p>
      <div className="list rp-priority">
        {order.map((n, i) => (
          <div key={n} className="list-row">
            <span className="rp-rank">{i + 1}</span>
            <div className="grow mono ellipsis">
              {n}
              {info(n)?.online && <span className="chip compact success">{t('servers.relayNames.online')}</span>}
              {info(n)?.paused && <span className="chip compact danger">{t('servers.relayPlan.paused')}</span>}
            </div>
            <button
              className="btn ghost icon sm"
              disabled={i === 0}
              title={t('servers.relayPlan.moveUp')}
              aria-label={t('servers.relayPlan.moveUp')}
              onClick={() => move(i, -1)}
            >
              <ChevronUp />
            </button>
            <button
              className="btn ghost icon sm"
              disabled={i === order.length - 1}
              title={t('servers.relayPlan.moveDown')}
              aria-label={t('servers.relayPlan.moveDown')}
              onClick={() => move(i, 1)}
            >
              <ChevronDown />
            </button>
          </div>
        ))}
      </div>
      {changed && (
        <div className="row">
          <button className="btn sm primary" disabled={busy} onClick={() => void save()}>
            {busy ? <Loader2 className="spin" /> : <Check />} {t('servers.relayPlan.prioritySave')}
          </button>
          <button className="btn sm ghost" onClick={() => setOrder(start)}>
            {t('common.cancel')}
          </button>
        </div>
      )}
    </>
  )
}

/** What's ending or ended: grace, paused names, and one-time purchases that end within a week. */
function Notices({ a, onRenew }: { a: AccountPlan; onRenew: (s: PlanSub) => void }) {
  const plan = a.plan!
  const access = plan.access
  const now = Date.now() / 1000
  const soon = plan.mine.filter((s) => s.once && s.active && s.period_end > 0 && s.period_end - now < 7 * 86400)
  return (
    <>
      {access && access.grace_until > 0 && (
        <div className="callout pending rp-notice">
          <b>{t('servers.relayPlan.graceTitle')}</b>
          <span>
            {t('servers.relayPlan.graceBody', {
              date: new Date(access.grace_until * 1000).toLocaleString(),
              servers: access.servers,
              names: access.will_pause.length ? access.will_pause.join(', ') : '–'
            })}
          </span>
        </div>
      )}
      {access && access.paused.length > 0 && (
        <div className="callout danger rp-notice">
          <b>{t('servers.relayPlan.pausedTitle')}</b>
          <span>{t('servers.relayPlan.pausedBody', { names: access.paused.join(', ') })}</span>
        </div>
      )}
      {soon.map((s) => (
        <div key={s.id} className="callout pending rp-notice">
          <span className="grow">
            {t('servers.relayPlan.endsSoon', {
              name: s.name,
              date: new Date(s.period_end * 1000).toLocaleString(),
              days: Math.max(0, Math.ceil((s.period_end - now) / 86400))
            })}
          </span>
          <button className="btn sm primary" onClick={() => onRenew(s)}>
            <RefreshCw /> {t('servers.relayPlan.renew')}
          </button>
        </div>
      ))}
    </>
  )
}

/** The cart of one account, kept on this PC (closing the Store keeps it). */
function useCart(account: string) {
  const key = `rc-store-cart:${account}`
  const read = () => {
    try {
      const v = JSON.parse(localStorage.getItem(key) ?? '[]')
      return Array.isArray(v) ? (v.filter((x) => typeof x === 'string') as string[]) : []
    } catch {
      return []
    }
  }
  const [items, setItems] = useState<string[]>(read)
  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => setItems(read()), [key])
  const save = (next: string[]) => {
    setItems(next)
    try {
      localStorage.setItem(key, JSON.stringify(next))
    } catch {
      // Storage off: the cart lasts until the Store closes.
    }
  }
  return {
    items,
    add: (id: string) => !items.includes(id) && save([...items, id]),
    remove: (id: string) => save(items.filter((x) => x !== id)),
    removeAll: (ids: string[]) => save(items.filter((x) => !ids.includes(x))),
    clear: () => save([])
  }
}

function ChangeModal({ a, sub, onClose, onDone }: { a: AccountPlan; sub: PlanSub; onClose: () => void; onDone: (step: PayStep, name: string) => void }) {
  const plan = a.plan!
  const options = plan.products.filter((p) => (sub.yearly ? p.yearly : p.monthly) > 0 || p.monthly > 0)
  const [product, setProduct] = useState(options.find((p) => p.id !== sub.product)?.id ?? '')
  const [yearly, setYearly] = useState(sub.yearly)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string>()
  const chosen = options.find((p) => p.id === product)
  const price = chosen ? (yearly ? chosen.yearly : chosen.monthly) : 0
  const same = product === sub.product && yearly === sub.yearly

  const save = async () => {
    setBusy(true)
    setError(undefined)
    try {
      onDone(await api.server.relayPlanChange(a.account, sub.id, product, yearly), chosen!.name)
    } catch (e) {
      setError((e as Error).message)
      setBusy(false)
    }
  }

  return (
    <Modal
      title={t('servers.relayPlan.changeTitle', { name: sub.name })}
      subtitle={t('servers.relayPlan.changeHint')}
      onClose={onClose}
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={!chosen || !price || same || busy} onClick={() => void save()}>
            {busy ? <Loader2 className="spin" /> : <Repeat />} {t('servers.relayPlan.change')}
          </button>
        </>
      }
    >
      <div className="field">
        <label htmlFor="rp-change">{t('servers.relayPlan.changeTo')}</label>
        <select id="rp-change" className="input" value={product} onChange={(e) => setProduct(e.target.value)}>
          {options.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
            </option>
          ))}
        </select>
        {chosen && <span className="hint">{gives(chosen)}</span>}
      </div>
      <div className="segmented">
        <button className={!yearly ? 'on' : undefined} disabled={!chosen?.monthly} onClick={() => setYearly(false)}>
          {t('servers.relayPlan.monthly')}
        </button>
        <button className={yearly ? 'on' : undefined} disabled={!chosen?.yearly} onClick={() => setYearly(true)}>
          {t('servers.relayPlan.yearly')}
        </button>
      </div>
      {chosen && price > 0 && (
        <p className="hint">
          {yearly
            ? t('servers.relayPlan.perYear', { price: money(price, plan.currency) })
            : t('servers.relayPlan.perMonth', { price: money(price, plan.currency) })}
        </p>
      )}
      {error && <p className="hint it-warn">{error}</p>}
    </Modal>
  )
}

function SubRow({
  s,
  plan,
  onCancel,
  onResume,
  onChange,
  onPay,
  onRenew
}: {
  s: PlanSub
  plan: PlanInfo
  onCancel: () => void
  onResume: () => void
  onChange: () => void
  onPay: () => void
  onRenew: () => void
}) {
  const unpaid = s.status === 'incomplete' || s.status === 'past_due' || s.status === 'unpaid'
  const when = s.period_end
    ? s.cancel_at_period_end || !s.paid
      ? t('servers.relayPlan.ends', { date: day(s.period_end) })
      : t('servers.relayPlan.renews', { date: day(s.period_end) })
    : ''
  return (
    <div className="list-row">
      <div className="ico">{s.paid ? <Repeat /> : <Check />}</div>
      <div className="grow" style={{ minWidth: 0 }}>
        <div className="title ellipsis">
          {s.name}
          {s.active && !unpaid && <span className="chip compact success">{t('servers.relayPlan.active')}</span>}
          {s.status === 'past_due' && <span className="chip compact warn">{t('servers.relayPlan.pastDue')}</span>}
          {s.status === 'incomplete' && <span className="chip compact">{t('servers.relayPlan.unpaid')}</span>}
          {!s.active && !unpaid && <span className="chip compact danger">{t('servers.relayPlan.ended')}</span>}
        </div>
        <div className="hint ellipsis">
          {s.once
            ? `${t('servers.relayPlan.paidOnce', { price: money(s.amount, plan.currency), method: t(`servers.relayPlan.method.${s.method || 'card'}` as Key) })}${s.period_end ? ` · ${t('servers.relayPlan.until', { date: day(s.period_end) })}` : ''}`
            : s.paid
              ? `${
                  s.yearly
                    ? t('servers.relayPlan.perYear', { price: money(s.amount, plan.currency) })
                    : t('servers.relayPlan.perMonth', { price: money(s.amount, plan.currency) })
                }${when ? ` · ${when}` : ''}`
              : `${t('servers.relayPlan.given')}${when ? ` · ${when}` : ''}`}
        </div>
      </div>
      {s.once && plan.enabled && (
        <button className="btn sm primary" onClick={onRenew}>
          <RefreshCw /> {t('servers.relayPlan.renew')}
        </button>
      )}
      {s.paid && unpaid && (
        <button className="btn sm primary" onClick={onPay}>
          <CreditCard /> {t('servers.relayPlan.payNow')}
        </button>
      )}
      {s.paid && s.active && !unpaid && plan.enabled && !(s.items && s.items.length > 1) && (
        <button className="btn sm" onClick={onChange}>
          <Repeat /> {t('servers.relayPlan.change')}
        </button>
      )}
      {s.paid &&
        s.active &&
        (s.cancel_at_period_end ? (
          <button className="btn sm" onClick={onResume}>
            <Check /> {t('servers.relayPlan.resume')}
          </button>
        ) : (
          <button className="btn sm danger" onClick={onCancel}>
            <X /> {t('servers.relayPlan.cancel')}
          </button>
        ))}
    </div>
  )
}

/** The add-ons and packs, the account's subscriptions and gift codes. `page`: as the Store page (no section title). */
export default function RelayPlan({ page, empty }: { page?: boolean; empty?: React.ReactNode } = {}) {
  const { toast } = useStore()
  const { plans, setPlans, load } = usePlans()
  const [picked, setPicked] = useState<string>()
  const [yearly, setYearly] = useState(false)
  const [busy, setBusy] = useState<string>()
  const [paying, setPaying] = useState<{ step: PayStep; title: string; name: string }>()
  const [buying, setBuying] = useState<PlanProduct[]>()
  const [renewing, setRenewing] = useState<PlanSub>()
  const [gift, setGift] = useState('')
  const [redeeming, setRedeeming] = useState(false)
  const [changing, setChanging] = useState<PlanSub>()
  const [cancelling, setCancelling] = useState<PlanSub>()

  // Nothing for sale anywhere (payments off, no relay, an older relay): nothing to show.
  const selling = plans?.accounts.filter((a) => a.plan?.enabled) ?? []
  // The cart belongs to the account shown (a hook: before the returns below).
  const cart = useCart((selling.find((x) => x.account === picked) ?? selling[0])?.account ?? '')
  if (!plans) return page ? <Loader2 className="spin" /> : null
  if (!plans.available || selling.length === 0) return <>{empty ?? null}</>
  const a = selling.find((x) => x.account === picked) ?? selling[0]
  const plan = a.plan!
  const put = (p: PlanInfo) => setPlans((all) => all && { ...all, accounts: all.accounts.map((x) => (x.account === a.account ? { ...x, plan: p } : x)) })

  const buy = (p: PlanProduct) => setBuying([p])
  const productsOf = (s: PlanSub) =>
    (s.items && s.items.length ? s.items.map((i) => i.product) : [s.product])
      .map((id) => plan.products.find((p) => p.id === id))
      .filter((p): p is PlanProduct => !!p)
  const renew = (s: PlanSub) => {
    if (productsOf(s).length === 0) return toast(t('servers.relayPlan.notForSaleNow'), 'error')
    setYearly(s.yearly)
    setRenewing(s)
    setBuying(productsOf(s))
  }

  const redeem = async () => {
    setRedeeming(true)
    try {
      put(await api.server.relayPlanRedeem(a.account, gift.trim()))
      setGift('')
      toast(t('servers.relayPlan.redeemed', { account: a.accountName }), 'success')
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setRedeeming(false)
    }
  }

  const payOpen = async (s: PlanSub) => {
    try {
      const step = await api.server.relayPlanPay(a.account, s.id)
      if (step.client_secret) setPaying({ step, title: t('servers.relayPlan.payTitle', { name: s.name }), name: s.name })
      else put(await api.server.relayPlanRefresh(a.account, s.id))
    } catch (e) {
      toast((e as Error).message, 'error')
    }
  }

  const resume = async (s: PlanSub) => {
    try {
      put(await api.server.relayPlanCancel(a.account, s.id, true))
      toast(t('servers.relayPlan.resumed', { name: s.name }), 'success')
    } catch (e) {
      toast((e as Error).message, 'error')
    }
  }

  // What's in the cart and still for sale (a product taken off sale drops out).
  const inCart = cart.items.map((id) => plan.products.find((p) => p.id === id)).filter((p): p is PlanProduct => !!p)
  const cartTotal = inCart.reduce((n, p) => n + (yearly ? p.yearly : p.monthly), 0)
  const cartMissing = inCart.filter((p) => !(yearly ? p.yearly : p.monthly))

  const tot = plan.totals
  const addons = plan.products.filter((p) => p.kind !== 'pack')
  const packs = plan.products.filter((p) => p.kind === 'pack')
  const card = (p: PlanProduct) => {
    const price = yearly ? p.yearly : p.monthly
    if (!price) return null
    const others = othersWith(plans.accounts, a.account, p)
    return (
      <div key={p.id} className={`rp-card${p.owned ? ' owned' : ''}`}>
        <div className="rp-card-name">{p.name}</div>
        <div className="hint">{gives(p)}</div>
        <div className="rp-price">
          {money(price, plan.currency)}
          <span className="hint"> / {yearly ? t('servers.relayPlan.year') : t('servers.relayPlan.month')}</span>
        </div>
        {others.length > 0 && <div className="hint it-warn">{t('servers.relayPlan.otherHas', { account: others.join(', ') })}</div>}
        <div className="row rp-card-buttons">
          <button className="btn sm primary" disabled={p.owned || busy != null} onClick={() => void buy(p)}>
            {busy === p.id ? <Loader2 className="spin" /> : p.owned ? <Check /> : <CreditCard />}{' '}
            {p.owned ? t('servers.relayPlan.owned') : t('servers.relayPlan.buy')}
          </button>
          {!p.owned && (
            <button className="btn sm" disabled={cart.items.includes(p.id)} onClick={() => cart.add(p.id)}>
              {cart.items.includes(p.id) ? <Check /> : <Plus />} {cart.items.includes(p.id) ? t('servers.relayPlan.inCart') : t('servers.relayPlan.addToCart')}
            </button>
          )}
        </div>
      </div>
    )
  }

  return (
    <div className={page ? 'relay-plan rp-page' : 'settings-section relay-plan'}>
      <div className="row">
        <h3 className="grow">{page ? '' : t('servers.relayPlan.title')}</h3>
        {plan.test_mode && <span className="chip compact warn">{t('servers.relayPlan.testMode')}</span>}
        <button className="btn ghost icon sm" title={t('common.refresh')} onClick={() => void load()}>
          <RefreshCw />
        </button>
      </div>
      <p className="hint">{t('servers.relayPlan.hint')}</p>
      {plans.accounts.length > 1 && (
        <div className="segmented rp-accounts" role="tablist" aria-label={t('servers.relayPlan.account')}>
          {plans.accounts.map((x) => (
            <button
              key={x.account}
              role="tab"
              aria-selected={x.account === a.account}
              className={x.account === a.account ? 'on' : undefined}
              disabled={!x.plan?.enabled}
              title={x.error ? tx(x.error) : undefined}
              onClick={() => setPicked(x.account)}
            >
              {x.accountName}
            </button>
          ))}
        </div>
      )}
      {plans.accounts
        .filter((x) => x.error)
        .map((x) => (
          <p key={x.account} className="hint bad">
            {x.accountName}: {tx(x.error)}
          </p>
        ))}
      <div className="rp-totals">
        <span className="chip">{t('servers.relayPlan.servers', { count: tot.servers })}</span>
        <span className="chip">{t('servers.relayPlan.names', { count: tot.names })}</span>
        <span className="chip">
          {tot.unlimited_traffic ? t('servers.relayPlan.trafficUnlimited') : t('servers.relayPlan.traffic', { amount: gb(tot.monthly_bytes) })}
        </span>
        <span className="chip">
          {tot.no_session_limit ? t('servers.relayPlan.sessionUnlimited') : t('servers.relayPlan.session', { minutes: Math.round(tot.session_secs / 60) })}
        </span>
        <span className={`chip${tot.whitelist_page ? ' success' : ''}`}>
          {tot.whitelist_page ? t('servers.relayPlan.whitelistOn') : t('servers.relayPlan.whitelistOff')}
        </span>
      </div>
      <p className="hint">
        {t('servers.relayPlan.freeLine', {
          servers: plan.free.servers,
          names: plan.free.names,
          traffic: gb(plan.free.monthly_bytes),
          bonus: gb(plan.free.bonus_bytes)
        })}
      </p>
      <Notices a={a} onRenew={renew} />
      <div className="row rp-redeem">
        <input
          className="input mono"
          value={gift}
          maxLength={40}
          spellCheck={false}
          aria-label={t('servers.relayPlan.giftCode')}
          placeholder={t('servers.relayPlan.giftPlaceholder')}
          onChange={(e) => setGift(e.target.value.toUpperCase().replace(/[^A-Z0-9-]/g, ''))}
          onKeyDown={(e) => e.key === 'Enter' && gift.trim().length >= 3 && void redeem()}
        />
        <button className="btn sm" disabled={redeeming || gift.trim().length < 3} onClick={() => void redeem()}>
          {redeeming ? <Loader2 className="spin" /> : <Gift />} {t('servers.relayPlan.redeem')}
        </button>
      </div>
      {plan.mine.length > 0 && (
        <>
          <h4>{t('servers.relayPlan.mine')}</h4>
          <div className="list">
            {plan.mine.map((s) => (
              <SubRow
                key={s.id}
                s={s}
                plan={plan}
                onCancel={() => setCancelling(s)}
                onResume={() => void resume(s)}
                onChange={() => setChanging(s)}
                onPay={() => void payOpen(s)}
                onRenew={() => renew(s)}
              />
            ))}
          </div>
        </>
      )}
      {plan.access && <Priority a={a} onSaved={put} />}
      {plan.products.length > 0 && (
        <>
          <div className="row rp-period">
            <div className="segmented">
              <button className={!yearly ? 'on' : undefined} onClick={() => setYearly(false)}>
                {t('servers.relayPlan.monthly')}
              </button>
              <button className={yearly ? 'on' : undefined} onClick={() => setYearly(true)}>
                {t('servers.relayPlan.yearly')}
              </button>
            </div>
            {yearly && <span className="hint">{t('servers.relayPlan.yearlyHint')}</span>}
          </div>
          {inCart.length > 0 && (
            <div className="card rp-cart">
              <h4>
                <ShoppingCart /> {t('servers.relayPlan.cart')}
              </h4>
              {inCart.map((p) => (
                <div key={p.id} className="rp-line">
                  <span className="grow">{p.name}</span>
                  <b>{(yearly ? p.yearly : p.monthly) ? money(yearly ? p.yearly : p.monthly, plan.currency) : t('servers.relayPlan.notThisPeriod')}</b>
                  <button
                    className="btn ghost icon sm"
                    title={t('servers.relayPlan.removeFromCart')}
                    aria-label={t('servers.relayPlan.removeFromCart')}
                    onClick={() => cart.remove(p.id)}
                  >
                    <X />
                  </button>
                </div>
              ))}
              <div className="rp-line rp-total">
                <span className="grow">{t('servers.relayPlan.total')}</span>
                <b>
                  {yearly
                    ? t('servers.relayPlan.perYear', { price: money(cartTotal, plan.currency) })
                    : t('servers.relayPlan.perMonth', { price: money(cartTotal, plan.currency) })}
                </b>
              </div>
              <p className="hint">{t('servers.relayPlan.cartHint')}</p>
              <div className="row">
                <button className="btn primary" disabled={cartMissing.length > 0} onClick={() => setBuying(inCart)}>
                  <CreditCard /> {t('servers.relayPlan.checkout')}
                </button>
                <button className="btn ghost" onClick={() => cart.clear()}>
                  <Trash2 /> {t('servers.relayPlan.clearCart')}
                </button>
              </div>
            </div>
          )}
          {packs.length > 0 && (
            <>
              <h4>
                <Package /> {t('servers.relayPlan.packs')}
              </h4>
              <div className="rp-grid">{packs.map(card)}</div>
            </>
          )}
          {addons.length > 0 && (
            <>
              <h4>
                <Puzzle /> {t('servers.relayPlan.addons')}
              </h4>
              <div className="rp-grid">{addons.map(card)}</div>
            </>
          )}
        </>
      )}
      {buying && (
        <CheckoutModal
          a={a}
          items={buying}
          yearly={yearly}
          renewOf={renewing?.id}
          onClose={() => {
            setBuying(undefined)
            setRenewing(undefined)
            void load()
          }}
          onPaid={(plan) => {
            put(plan)
            toast(t('servers.relayPlan.paid', { name: buying.map((p) => p.name).join(' + '), account: a.accountName }), 'success')
            // Bought from the cart: it's empty again.
            if (!renewing) cart.removeAll(buying.map((p) => p.id))
            setBuying(undefined)
            setRenewing(undefined)
          }}
        />
      )}
      {paying && (
        <PayModal
          account={a.account}
          accountName={a.accountName}
          title={paying.title}
          step={paying.step}
          onClose={() => {
            setPaying(undefined)
            void load()
          }}
          onPaid={(p) => {
            put(p)
            setPaying(undefined)
            toast(t('servers.relayPlan.paid', { name: paying.name, account: a.accountName }), 'success')
          }}
        />
      )}
      {changing && (
        <ChangeModal
          a={a}
          sub={changing}
          onClose={() => setChanging(undefined)}
          onDone={(step, name) => {
            setChanging(undefined)
            if (step.client_secret) setPaying({ step, title: t('servers.relayPlan.changeTitle', { name: changing.name }), name })
            else {
              toast(t('servers.relayPlan.changed', { name }), 'success')
              void api.server.relayPlanRefresh(a.account, step.sub).then(put, () => load())
            }
          }}
        />
      )}
      {cancelling && (
        <ConfirmModal
          title={t('servers.relayPlan.cancelTitle', { name: cancelling.name })}
          body={t('servers.relayPlan.cancelBody', { date: cancelling.period_end ? day(cancelling.period_end) : '–' })}
          confirm={t('servers.relayPlan.cancel')}
          onClose={() => setCancelling(undefined)}
          onConfirm={async () => {
            try {
              put(await api.server.relayPlanCancel(a.account, cancelling.id, false))
              toast(t('servers.relayPlan.cancelled', { name: cancelling.name }))
            } catch (e) {
              toast((e as Error).message, 'error')
            }
          }}
        />
      )}
    </div>
  )
}
