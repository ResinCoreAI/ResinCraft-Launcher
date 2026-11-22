// The Servers tab's ResinCraft Relay plan: what each signed-in Microsoft account has, the add-ons and packs the relay
// sells, and buying them with the launcher's own payment form (Stripe's card fields inside it, no Stripe page).
// While the relay's admin has payments off, nothing is for sale and this shows nothing.
import { useCallback, useEffect, useRef, useState } from 'react'
import {
  ArrowRightLeft,
  Check,
  ChevronDown,
  ChevronUp,
  CreditCard,
  ExternalLink,
  Gift,
  ListOrdered,
  Loader2,
  Lock,
  Package,
  Plus,
  Puzzle,
  RefreshCw,
  Repeat,
  ShoppingBag,
  ShoppingCart,
  Trash2,
  History,
  Wallet,
  X
} from 'lucide-react'
import type { AccountPlan, PayStep, PlanInfo, PlanProduct, PlanQuote, PlanSub, RelayPlans, WalletEntry, WalletInfo } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { t, tx, type Key } from '../i18n'
import { ConfirmModal, Modal } from './ui'
import { loadStripe, stripeAppearance, type StripeJs } from '../stripe'
import '../relay-plan.css'
import '../resin-credit.css'
import { countText, UNLIMITED_COUNT } from '../relayCount'
import { money, setWalletBalance } from '../storeStatus'


const gb = (bytes: number) => `${Math.round((bytes / 1e9) * 10) / 10} GB`
const day = (unix: number) => new Date(unix * 1000).toLocaleDateString()

/** What a product gives, in a few words. */
function gives(p: PlanProduct) {
  const g = p.grants
  const parts: string[] = []
  if (g.servers >= UNLIMITED_COUNT) parts.push(t('servers.relayPlan.giveServersInf'))
  else if (g.servers) parts.push(t('servers.relayPlan.giveServers', { count: g.servers }))
  if (g.names >= UNLIMITED_COUNT) parts.push(t('servers.relayPlan.giveNamesInf'))
  else if (g.names) parts.push(t('servers.relayPlan.giveNames', { count: g.names }))
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
  testMode,
  onClose,
  onPaid,
  paid,
  onDone
}: {
  account: string
  accountName: string
  title: string
  step: PayStep
  testMode?: boolean
  onClose: () => void
  onPaid?: (plan: PlanInfo) => void
  /** Not a purchase (a Resin Credit top-up): reads the payment again and says whether it went through; then `onDone`. */
  paid?: () => Promise<boolean>
  onDone?: () => void
}) {
  const box = useRef<HTMLDivElement>(null)
  const stripe = useRef<{ stripe: StripeJs; elements: StripeJs }>(undefined)
  const [ready, setReady] = useState(false)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string>()
  // The PromptPay QR code was closed without paying: the same payment stays open for another try.
  const [notPaid, setNotPaid] = useState(false)
  const [qrPage, setQrPage] = useState<string>()
  const mock = step.publishable_key === 'pk_mock'

  /** Where the QR code can be opened: in test mode the code's own link (Stripe's page that simulates the scan), else
   * Stripe's page with the QR code to scan with a phone. */
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const qrLink = (pi: any): string | undefined => {
    const qr = pi?.next_action?.promptpay_display_qr_code
    if (!qr) return undefined
    return testMode && typeof qr.data === 'string' && qr.data.startsWith('https://') ? qr.data : qr.hosted_instructions_url
  }

  /** Paid: the purchase as the relay has it now (a renewal folds into the purchase it renews). */
  const finish = async () => {
    if (paid) {
      for (let i = 0; i < 7; i++) {
        if (i) await new Promise((r) => setTimeout(r, 1500))
        if (await paid()) {
          onDone?.()
          return true
        }
      }
      return false
    }
    let plan = await api.server.relayPlanRefresh(account, step.sub)
    const done = (p: PlanInfo) => {
      const m = p.mine.find((x) => x.id === step.sub)
      return m ? m.active : !!step.once
    }
    for (let i = 0; i < 6 && !done(plan); i++) {
      await new Promise((r) => setTimeout(r, 1500))
      plan = await api.server.relayPlanRefresh(account, step.sub)
    }
    if (!done(plan)) return false
    onPaid?.(plan)
    return true
  }

  // While a QR code is out (scanned on a phone, or simulated in the browser in test mode): look every few seconds.
  useEffect(() => {
    if (!notPaid) return
    let gone = false
    const timer = setInterval(async () => {
      if (gone) return
      if (paid) {
        if ((await paid().catch(() => false)) && !gone) {
          gone = true
          onDone?.()
        }
        return
      }
      const plan = await api.server.relayPlanRefresh(account, step.sub).catch(() => undefined)
      const m = plan?.mine.find((x) => x.id === step.sub)
      if (plan && (m ? m.active : step.once && !plan.mine.some((x) => x.id === step.sub && !x.active)) && !gone) {
        gone = true
        onPaid?.(plan)
      }
    }, 4000)
    return () => {
      gone = true
      clearInterval(timer)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [notPaid])

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
        // A QR code that stays open (or was closed without Stripe saying so): after a few seconds the form says it isn't
        // paid yet, offers the QR page, and keeps looking; the same payment is used for every try.
        const watchdog = setTimeout(async () => {
          if (step.intent !== 'payment' || !step.client_secret) return
          const pi = (await s.stripe.retrievePaymentIntent(step.client_secret).catch(() => ({}))).paymentIntent
          if (pi?.status === 'requires_action') {
            setQrPage(qrLink(pi))
            setNotPaid(true)
            setBusy(false)
          }
        }, 8000)
        // Cards confirm here (3-D Secure opens in Stripe's own frame); return_url only for methods that must redirect.
        const r = await confirm
          .call(s.stripe, { elements: s.elements, confirmParams: { return_url: location.href }, redirect: 'if_required' })
          .finally(() => clearTimeout(watchdog))
        if (r.error) throw new Error(r.error.message)
        const status = r.paymentIntent?.status ?? r.setupIntent?.status
        if (status && status !== 'succeeded' && status !== 'processing') {
          // The QR code was closed: nothing paid. Its page (to scan, or to simulate in test mode) stays one click away.
          const pi = r.paymentIntent ?? (await s.stripe.retrievePaymentIntent(step.client_secret)).paymentIntent
          setQrPage(qrLink(pi))
          setNotPaid(true)
          setBusy(false)
          return
        }
      }
      setNotPaid(false)
      if (!(await finish())) {
        setNotPaid(true)
        setBusy(false)
      }
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
      onClose={onClose}
      footer={
        <>
          <span className="hint grow rp-secure">
            <Lock /> {t('servers.relayPlan.secure')}
          </span>
          <button className="btn ghost" onClick={onClose}>
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
      {notPaid && (
        <div className="callout pending rp-notice">
          <span className="grow">{t('servers.relayPlan.notPaidQr')}</span>
          {qrPage && (
            <button className="btn sm" onClick={() => void api.app.openExternal(qrPage)}>
              <ExternalLink /> {testMode ? t('servers.relayPlan.simulateQr') : t('servers.relayPlan.openQr')}
            </button>
          )}
          <button
            className="btn sm"
            disabled={busy}
            onClick={async () => {
              setBusy(true)
              if (!(await finish().catch(() => false))) setBusy(false)
            }}
          >
            <RefreshCw /> {t('servers.relayPlan.checkPaid')}
          </button>
        </div>
      )}
      {error && <p className="hint it-warn">{error}</p>}
    </Modal>
  )
}

// ---------- Resin Credit: the Store's wallet (only while the plan says `wallet`) ----------

/** The account's Resin Credit: loaded while `on`, with older ledger pages on request. */
function useWallet(account: string, on: boolean) {
  const [wallet, setWallet] = useState<WalletInfo>()
  const [error, setError] = useState<string>()
  const [loadingMore, setLoadingMore] = useState(false)
  const reload = useCallback(async () => {
    if (!on) return setWallet(undefined)
    try {
      setWallet(await api.server.relayWallet(account))
      setError(undefined)
    } catch (e) {
      setError((e as Error).message)
    }
  }, [account, on])
  useEffect(() => {
    setWallet(undefined)
    void reload()
  }, [reload])
  const more = async () => {
    const last = wallet?.history[wallet.history.length - 1]
    if (!wallet || !last) return
    setLoadingMore(true)
    try {
      const next = await api.server.relayWallet(account, last.id)
      setWallet({ ...next, history: [...wallet.history, ...next.history.filter((e) => !wallet.history.some((x) => x.id === e.id))] })
    } catch (e) {
      setError((e as Error).message)
    } finally {
      setLoadingMore(false)
    }
  }
  return { wallet, setWallet, error, reload, more, loadingMore }
}

const WALLET_KINDS: Record<string, Key> = {
  topup: 'servers.wallet.kind.topup',
  purchase: 'servers.wallet.kind.purchase',
  refund: 'servers.wallet.kind.refund',
  grant: 'servers.wallet.kind.grant',
  deduct: 'servers.wallet.kind.deduct'
}
const signed = (n: number, currency: string) => `${n > 0 ? '+' : n < 0 ? '−' : ''}${money(Math.abs(n), currency)}`
const stamp = (unix: number) => new Date(unix * 1000).toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' })

/** Ledger entries, newest first: what, when, the amount (+ in, − out), the balance after and the note. */
function WalletEntries({ entries, currency }: { entries: WalletEntry[]; currency: string }) {
  return (
    <div className="rc-ledger">
      {entries.map((e) => (
        <div key={e.id} className="rc-entry">
          <div className="rc-entry-text">
            <b>{WALLET_KINDS[e.kind] ? t(WALLET_KINDS[e.kind]) : e.kind}</b>
            <span className="hint">{stamp(e.at)}</span>
            {e.note && <span className="hint rc-entry-note">{e.note}</span>}
          </div>
          <div className="rc-entry-amount">
            <b className={e.amount < 0 ? 'rc-out' : 'rc-in'}>{signed(e.amount, currency)}</b>
            <span className="hint">{t('servers.wallet.after', { balance: money(e.balance_after, currency) })}</span>
          </div>
        </div>
      ))}
    </div>
  )
}

/** Every ledger entry, a page at a time ("Show more" asks for the ones before the last). */
function WalletHistoryModal({ w, accountName, onClose }: { w: ReturnType<typeof useWallet>; accountName: string; onClose: () => void }) {
  const wallet = w.wallet!
  return (
    <Modal
      title={t('servers.wallet.historyTitle')}
      subtitle={t('servers.wallet.balanceOf', { account: accountName, balance: money(wallet.balance, wallet.currency) })}
      onClose={onClose}
      footer={
        <button className="btn" onClick={onClose}>
          {t('common.close')}
        </button>
      }
    >
      {wallet.history.length === 0 ? <p className="hint">{t('servers.wallet.noHistory')}</p> : <WalletEntries entries={wallet.history} currency={wallet.currency} />}
      {wallet.more && (
        <button className="btn sm rc-more" disabled={w.loadingMore} onClick={() => void w.more()}>
          {w.loadingMore ? <Loader2 className="spin" /> : <ChevronDown />} {t('servers.wallet.showMore')}
        </button>
      )}
    </Modal>
  )
}

/** Buys Resin Credit: one of the admin's amounts, then card or PromptPay (the payment form) or a TrueMoney angpao. */
function TopupModal({
  a,
  wallet,
  need,
  onClose,
  onDone
}: {
  a: AccountPlan
  wallet: WalletInfo
  /** At least this much is missing (from checkout): the first amount that covers it is picked. */
  need?: number
  onClose: () => void
  onDone: (w: WalletInfo, added: number) => void
}) {
  const cur = wallet.currency
  const [amount, setAmount] = useState(() => wallet.topups.find((n) => n >= (need ?? 0)) ?? wallet.topups[0] ?? 0)
  const [method, setMethod] = useState(wallet.methods[0] ?? '')
  const [link, setLink] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string>()
  const [step, setStep] = useState<PayStep>()
  const before = wallet.balance

  if (step)
    return (
      <PayModal
        account={a.account}
        accountName={a.accountName}
        title={t('servers.wallet.topupTitleAmount', { amount: money(step.amount, cur) })}
        step={step}
        testMode={a.plan!.test_mode}
        onClose={onClose}
        paid={async () => {
          const w = await api.server.relayWalletRefresh(a.account, step.sub)
          if (!w.history.some((e) => e.kind === 'topup' && e.ref === step.sub)) return false
          onDone(w, w.balance - before)
          return true
        }}
      />
    )

  const go = async () => {
    setBusy(true)
    setError(undefined)
    try {
      const r = await api.server.relayWalletTopup(a.account, amount, method, method === 'truemoney' ? link.trim() : null)
      if (r.wallet) onDone(r.wallet, r.wallet.balance - before)
      else if (r.pay) setStep(r.pay)
    } catch (e) {
      setError((e as Error).message)
      setBusy(false)
    }
  }
  const ready = !busy && amount > 0 && !!method && (method !== 'truemoney' || link.trim().length > 8)
  return (
    <Modal
      title={t('servers.wallet.topupTitle')}
      subtitle={t('servers.wallet.balanceOf', { account: a.accountName, balance: money(before, cur) })}
      onClose={onClose}
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={!ready} onClick={() => void go()}>
            {busy ? <Loader2 className="spin" /> : method === 'truemoney' ? <Gift /> : <CreditCard />}{' '}
            {method === 'truemoney' ? t('servers.wallet.topupAngpao') : t('servers.wallet.topupFor', { amount: money(amount, cur) })}
          </button>
        </>
      }
    >
      {wallet.methods.length === 0 || wallet.topups.length === 0 ? (
        <p className="hint it-warn">{t('servers.wallet.topupClosed')}</p>
      ) : (
        <>
          <div className="field">
            <label>{t('servers.wallet.amount')}</label>
            <div className="rc-amounts" role="radiogroup" aria-label={t('servers.wallet.amount')}>
              {wallet.topups.map((n) => (
                <button key={n} role="radio" aria-checked={n === amount} className={`rc-amount${n === amount ? ' on' : ''}`} onClick={() => setAmount(n)}>
                  {money(n, cur)}
                </button>
              ))}
            </div>
            <span className="hint">{t('servers.wallet.afterTopup', { balance: money(before + amount, cur) })}</span>
          </div>
          <div className="field">
            <label>{t('servers.relayPlan.payWith')}</label>
            <div className="segmented rp-ways">
              {wallet.methods.map((m) => (
                <button key={m} className={method === m ? 'on' : undefined} onClick={() => setMethod(m)}>
                  {WALLET_METHODS[m] ? t(WALLET_METHODS[m]) : m}
                </button>
              ))}
            </div>
          </div>
          {method === 'truemoney' && (
            <div className="field">
              <label htmlFor="rc-angpao">{t('servers.relayPlan.angpaoLink')}</label>
              <input
                id="rc-angpao"
                className="input mono"
                value={link}
                maxLength={200}
                spellCheck={false}
                placeholder="https://gift.truemoney.com/campaign/?v=…"
                onChange={(e) => setLink(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && ready && void go()}
              />
              <span className="hint">{t('servers.wallet.angpaoHint', { amount: money(amount, cur) })}</span>
            </div>
          )}
          <p className="hint">{t('servers.wallet.topupHint')}</p>
        </>
      )}
      {error && <p className="hint it-warn">{error}</p>}
    </Modal>
  )
}

const WALLET_METHODS: Record<string, Key> = {
  card: 'servers.wallet.method.card',
  promptpay: 'servers.wallet.method.promptpay',
  truemoney: 'servers.wallet.method.truemoney'
}

/** The Store's Resin Credit card: the balance, Top up, the newest changes and the whole history. */
function WalletCard({ a, onBalance }: { a: AccountPlan; onBalance: (balance: number) => void }) {
  const { toast } = useStore()
  const plan = a.plan!
  const w = useWallet(a.account, !!plan.wallet)
  const [topup, setTopup] = useState(false)
  const [history, setHistory] = useState(false)
  const wallet = w.wallet
  const cur = wallet?.currency || plan.currency
  const balance = wallet?.balance ?? plan.wallet_balance ?? 0
  // The plan's balance moved (a purchase paid with credit, a top-up from checkout, the plan read again): the ledger too.
  const planBalance = plan.wallet_balance ?? 0
  useEffect(() => {
    if (w.wallet && w.wallet.balance !== planBalance) void w.reload()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [planBalance])
  return (
    <div className="rc-wallet">
      <div className="row rc-wallet-head">
        <Wallet />
        <span className="grow">{t('servers.wallet.title')}</span>
        {wallet && !wallet.enabled && <span className="chip compact">{t('servers.wallet.off')}</span>}
      </div>
      <div className="rc-balance">{money(balance, cur)}</div>
      <div className="row rc-wallet-buttons">
        <button className="btn sm primary" disabled={!wallet?.enabled || wallet.methods.length === 0} onClick={() => setTopup(true)}>
          <Plus /> {t('servers.wallet.topup')}
        </button>
        <button className="btn sm" disabled={!wallet} onClick={() => setHistory(true)}>
          <History /> {t('servers.wallet.history')}
        </button>
      </div>
      {wallet && wallet.history.length > 0 && <WalletEntries entries={wallet.history.slice(0, 3)} currency={cur} />}
      {w.error && <p className="hint bad">{w.error}</p>}
      {topup && wallet && (
        <TopupModal
          a={a}
          wallet={wallet}
          onClose={() => setTopup(false)}
          onDone={(next, added) => {
            w.setWallet(next)
            onBalance(next.balance)
            setTopup(false)
            toast(t('servers.wallet.added', { amount: money(Math.max(added, 0), next.currency), account: a.accountName }), 'success')
          }}
        />
      )}
      {history && wallet && <WalletHistoryModal w={w} accountName={a.accountName} onClose={() => setHistory(false)} />}
    </div>
  )
}

type PayWay = 'once' | 'renew' | 'angpao' | 'wallet'

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
  // Resin Credit: the player picked a way already (else credit is picked once it covers the cart), the top-up dialog
  // (with the wallet it needs), the "Pay … from Resin Credit?" question, and a new quote after a top-up.
  const touched = useRef(false)
  const [topupWallet, setTopupWallet] = useState<WalletInfo>()
  const [confirming, setConfirming] = useState(false)
  const [requote, setRequote] = useState(0)
  const { toast } = useStore()
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
        .relayPlanQuote(a.account, ids, yearly, promo.trim() || null, renewOf ?? null)
        .then((q) => {
          if (gone) return
          setQuote(q)
          if (q.wallet_covers && !touched.current) setWay('wallet')
        })
        .catch(() => {})
    }, 300)
    return () => {
      gone = true
      clearTimeout(timer)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [a.account, key, yearly, promo, requote])

  if (step)
    return <PayModal account={a.account} accountName={a.accountName} title={title} step={step} testMode={plan.test_mode} onClose={onClose} onPaid={onPaid} />

  const payWithCredit = async () => {
    setError(undefined)
    try {
      onPaid(await api.server.relayWalletPay(a.account, renewOf ? [] : ids, yearly, promo.trim() || null, renewOf ?? null))
    } catch (e) {
      setError((e as Error).message)
      // A balance that changed meanwhile: the quote says what it is now.
      setRequote((n) => n + 1)
    }
  }

  const openTopup = async () => {
    try {
      setTopupWallet(await api.server.relayWallet(a.account))
    } catch (e) {
      setError((e as Error).message)
    }
  }

  const go = async () => {
    if (way === 'wallet') return setConfirming(true)
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
  const period = yearly ? t('servers.relayPlan.oneYear') : t('servers.relayPlan.oneMonth')
  const ways: { value: PayWay; label: string; off?: boolean }[] = [
    { value: 'once', label: plan.promptpay ? t('servers.relayPlan.wayOnce') : t('servers.relayPlan.wayOnceCard') }
  ]
  if (!renewOf) ways.push({ value: 'renew', label: t('servers.relayPlan.wayRenew') })
  // TrueMoney: offered when the relay takes it; greyed out (with how to switch it on) for the relay's admin.
  if (plan.truemoney) ways.push({ value: 'angpao', label: t('servers.relayPlan.wayAngpao') })
  else if (plan.admin) ways.push({ value: 'angpao', label: t('servers.relayPlan.wayAngpao'), off: true })
  // Resin Credit: only from relays that have it switched on.
  const walletBalance = quote?.wallet ? (quote.wallet_balance ?? 0) : (plan.wallet_balance ?? 0)
  if (plan.wallet) ways.push({ value: 'wallet', label: t('servers.wallet.way', { balance: money(walletBalance, currency) }) })
  const missing = Math.max(0, total - walletBalance)
  const credit = quote?.credit ?? 0
  // A subscription takes the unused days off its first payment only.
  const totalText =
    way === 'renew'
      ? credit > 0
        ? t('servers.relayPlan.firstThen', { first: money(total, currency), then: per(total + credit) })
        : per(total)
      : `${money(total, currency)} · ${period}`
  const ready =
    !busy &&
    !cartError &&
    (way !== 'angpao' || link.trim().length > 8) &&
    !(below && way === 'once') &&
    (way !== 'wallet' || !!quote?.wallet_covers)

  return (
    <Modal
      title={title}
      subtitle={t('servers.relayPlan.paySubtitle', { account: a.accountName, price: totalText })}
      onClose={onClose}
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={!ready} onClick={() => void go()}>
            {busy ? <Loader2 className="spin" /> : way === 'angpao' ? <Gift /> : way === 'wallet' ? <Wallet /> : <CreditCard />}{' '}
            {way === 'angpao' ? t('servers.relayPlan.angpaoPay') : way === 'wallet' ? t('servers.wallet.payWith') : t('servers.relayPlan.continue')}
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
        {(quote?.credits ?? []).map((c) => (
          <div key={c.sub} className="rp-line rp-total">
            <span className="grow">{t('servers.relayPlan.creditLine', { name: c.name, days: c.days_left })}</span>
            <b>−{money(c.amount, currency)}</b>
          </div>
        ))}
        <div className="rp-line rp-total">
          <span className="grow">{t('servers.relayPlan.total')}</span>
          <b>{totalText}</b>
        </div>
      </div>
      {credit > 0 && <p className="hint">{t('servers.relayPlan.creditHint')}</p>}
      <div className="field">
        <label>{t('servers.relayPlan.payWith')}</label>
        <div className="segmented rp-ways">
          {ways.map((w) => (
            <button
              key={w.value}
              className={way === w.value ? 'on' : undefined}
              disabled={w.off}
              title={w.off ? t('servers.relayPlan.angpaoOffAdmin') : undefined}
              onClick={() => {
                touched.current = true
                setWay(w.value)
              }}
            >
              {w.label}
            </button>
          ))}
        </div>
        {ways.some((w) => w.off) && <span className="hint muted">{t('servers.relayPlan.angpaoOffAdmin')}</span>}
        <span className="hint">
          {way === 'once'
            ? t('servers.relayPlan.wayOnceHint', { period })
            : way === 'renew'
              ? t('servers.relayPlan.wayRenewHint')
              : way === 'wallet'
                ? t('servers.wallet.wayHint', { period })
                : t('servers.relayPlan.angpaoHint', { price: money(total, currency) })}
        </span>
      </div>
      {way === 'wallet' && (
        <div className="rc-checkout">
          <div className="rp-line">
            <span className="grow">{t('servers.wallet.balance')}</span>
            <b>{money(walletBalance, currency)}</b>
          </div>
          {quote?.wallet_covers ? (
            <div className="rp-line">
              <span className="grow">{t('servers.wallet.balanceAfter')}</span>
              <b>{money(walletBalance - total, currency)}</b>
            </div>
          ) : (
            quote &&
            missing > 0 && (
              <div className="rp-line rc-missing">
                <span className="grow">{t('servers.wallet.missing', { amount: money(missing, currency) })}</span>
                <button className="btn sm" onClick={() => void openTopup()}>
                  <Plus /> {t('servers.wallet.topup')}
                </button>
              </div>
            )
          )}
          {quote?.wallet_note && <p className="hint">{quote.wallet_note}</p>}
        </div>
      )}
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
      {items.length > 1 && <p className="hint">{t('servers.relayPlan.cartHint')}</p>}
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
      {topupWallet && (
        <TopupModal
          a={a}
          wallet={topupWallet}
          need={missing}
          onClose={() => setTopupWallet(undefined)}
          onDone={(w, added) => {
            setTopupWallet(undefined)
            toast(t('servers.wallet.added', { amount: money(Math.max(added, 0), w.currency), account: a.accountName }), 'success')
            touched.current = false
            setRequote((n) => n + 1)
          }}
        />
      )}
      {confirming && (
        <Modal
          title={t('servers.wallet.confirmTitle', { amount: money(total, currency) })}
          onClose={() => setConfirming(false)}
          footer={
            <>
              <button className="btn ghost" onClick={() => setConfirming(false)}>
                {t('common.cancel')}
              </button>
              <button
                className="btn primary"
                disabled={busy}
                onClick={async () => {
                  setBusy(true)
                  await payWithCredit()
                  setBusy(false)
                  setConfirming(false)
                }}
              >
                {busy ? <Loader2 className="spin" /> : <Wallet />} {t('servers.wallet.payAmount', { amount: money(total, currency) })}
              </button>
            </>
          }
        >
          <p>{t('servers.wallet.confirmBody', { items: items.map((p) => p.name).join(' + '), period, account: a.accountName })}</p>
          <p className="hint">{t('servers.wallet.confirmAfter', { balance: money(Math.max(0, walletBalance - total), currency) })}</p>
        </Modal>
      )}
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

/** The Servers tab's server priority: for each signed-in account the relay limits, the order its names stay online in
 *  when it has fewer slots than it uses (an add-on ended). Shown only while the relay sells and cuts off. */
export function ServerPriority() {
  const { plans, setPlans } = usePlans()
  const rows = (plans?.accounts ?? []).filter((a) => a.plan?.access && a.plan.access.names.filter((n) => !n.assigned).length >= 2)
  if (rows.length === 0) return null
  const put = (account: string) => (p: PlanInfo) =>
    setPlans((all) => all && { ...all, accounts: all.accounts.map((x) => (x.account === account ? { ...x, plan: p } : x)) })
  return (
    <div className="card pad rp-page" style={{ marginTop: 12 }}>
      {rows.map((a) => (
        <div key={a.account}>
          {rows.length > 1 && <p className="hint">{a.accountName}</p>}
          <Priority a={a} onSaved={put(a.account)} />
        </div>
      ))}
    </div>
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
              servers: countText(access.servers),
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
          {!plan.store_closed && (
            <button className="btn sm primary" onClick={() => onRenew(s)}>
              <RefreshCw /> {t('servers.relayPlan.renew')}
            </button>
          )}
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

/** Moves a purchase to another Microsoft account signed in here (only while the relay's admin allows it). */
function MoveModal({
  a,
  sub,
  targets,
  onClose,
  onMoved
}: {
  a: AccountPlan
  sub: PlanSub
  targets: AccountPlan[]
  onClose: () => void
  onMoved: (p: PlanInfo, to: AccountPlan) => void
}) {
  const [to, setTo] = useState(targets[0]?.account ?? '')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string>()
  const target = targets.find((x) => x.account === to)

  const save = async () => {
    if (!target) return
    setBusy(true)
    setError(undefined)
    try {
      onMoved(await api.server.relayPlanMove(a.account, sub.id, target.account), target)
    } catch (e) {
      setError((e as Error).message)
      setBusy(false)
    }
  }

  return (
    <Modal
      title={t('servers.relayPlan.moveTitle', { name: sub.name })}
      subtitle={t('servers.relayPlan.moveHint')}
      onClose={onClose}
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={!target || busy} onClick={() => void save()}>
            {busy ? <Loader2 className="spin" /> : <ArrowRightLeft />} {t('servers.relayPlan.move')}
          </button>
        </>
      }
    >
      <div className="field">
        <label htmlFor="rp-move">{t('servers.relayPlan.moveTo')}</label>
        <select id="rp-move" className="input" value={to} onChange={(e) => setTo(e.target.value)}>
          {targets.map((x) => (
            <option key={x.account} value={x.account}>
              {x.accountName}
            </option>
          ))}
        </select>
      </div>
      {target && <p className="hint">{t('servers.relayPlan.moveWhat', { name: sub.name, from: a.accountName, to: target.accountName })}</p>}
      {sub.paid && <p className="hint">{t('servers.relayPlan.moveBilling', { from: a.accountName })}</p>}
      <p className="hint it-warn">{t('servers.relayPlan.moveGrace', { from: a.accountName })}</p>
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
  onRenew,
  onMove
}: {
  s: PlanSub
  plan: PlanInfo
  onCancel: () => void
  onResume: () => void
  onChange: () => void
  onPay: () => void
  onRenew: () => void
  /** Set when it can be moved to another account signed in here. */
  onMove?: () => void
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
          {s.status === 'credited' ? (
            <span className="chip compact">{t('servers.relayPlan.credited')}</span>
          ) : (
            !s.active && !unpaid && <span className="chip compact danger">{t('servers.relayPlan.ended')}</span>
          )}
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
              : `${s.note || t('servers.relayPlan.given')}${when ? ` · ${when}` : ''}`}
        </div>
        {/* The admin's note on something bought (on a grant it stands in for "Given by the relay admin" above). */}
        {s.note && s.paid !== false && (s.paid || s.once) && <div className="hint rp-note">{s.note}</div>}
        {(s.refunded ?? 0) > 0 && (
          <div className="hint rc-refunded">
            <Wallet /> {t('servers.wallet.refunded', { amount: money(s.refunded!, plan.currency) })}
          </div>
        )}
      </div>
      {s.once && !s.cancel_at_period_end && plan.enabled && !plan.store_closed && s.status !== 'credited' && !s.refunded && (
        <button className="btn sm primary" onClick={onRenew}>
          <RefreshCw /> {t('servers.relayPlan.renew')}
        </button>
      )}
      {s.paid && unpaid && (
        <button className="btn sm primary" onClick={onPay}>
          <CreditCard /> {t('servers.relayPlan.payNow')}
        </button>
      )}
      {s.paid && s.active && !unpaid && plan.enabled && !plan.store_closed && !(s.items && s.items.length > 1) && (
        <button className="btn sm" onClick={onChange}>
          <Repeat /> {t('servers.relayPlan.change')}
        </button>
      )}
      {onMove && (
        <button className="btn sm" title={t('servers.relayPlan.moveHint')} onClick={onMove}>
          <ArrowRightLeft /> {t('servers.relayPlan.move')}
        </button>
      )}
      {/* Paid once: Cancel only stops it being renewed (it runs to its end date, nothing is refunded); Resume undoes. */}
      {(s.paid || s.once) &&
        s.active &&
        s.status !== 'credited' &&
        !s.refunded &&
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
/**
 * `page`: the Store page's own layout (a side panel with the account, what it has, the cart and gift codes; the packs
 * and add-ons to its right), with `heading` at the top of the panel. Without it, the compact block in Servers.
 */
export default function RelayPlan({ page, empty, heading }: { page?: boolean; empty?: React.ReactNode; heading?: React.ReactNode } = {}) {
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
  const [moving, setMoving] = useState<PlanSub>()

  // Nothing for sale anywhere (payments off, no relay, an older relay): nothing to show.
  const selling = plans?.accounts.filter((a) => a.plan?.enabled) ?? []
  // The cart belongs to the account shown (a hook: before the returns below).
  const cart = useCart((selling.find((x) => x.account === picked) ?? selling[0])?.account ?? '')
  if (!plans) return page ? <Loader2 className="spin" /> : null
  if (!plans.available || selling.length === 0) return <>{empty ?? null}</>
  const a = selling.find((x) => x.account === picked) ?? selling[0]
  const plan = a.plan!
  const put = (p: PlanInfo) => {
    setPlans((all) => all && { ...all, accounts: all.accounts.map((x) => (x.account === a.account ? { ...x, plan: p } : x)) })
    // The header's balance follows (a purchase paid with credit, a top-up).
    if (p.wallet) setWalletBalance(a.account, p.wallet_balance ?? 0, p.currency)
  }
  // Where a purchase can move: the other Microsoft accounts signed in here that the relay answered for.
  const moveTargets = plan.moves ? plans.accounts.filter((x) => x.account !== a.account && x.plan && !x.error) : []

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

  const accountTabs = plans.accounts.length > 1 && (
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
  )
  const errors = plans.accounts
    .filter((x) => x.error)
    .map((x) => (
      <p key={x.account} className="hint bad">
        {x.accountName}: {tx(x.error)}
      </p>
    ))
  const totals = (
    <div className="rp-totals">
      <span className="chip">{t('servers.relayPlan.servers', { count: countText(tot.servers) })}</span>
      <span className="chip">{t('servers.relayPlan.names', { count: countText(tot.names) })}</span>
      <span className="chip">{tot.unlimited_traffic ? t('servers.relayPlan.trafficUnlimited') : t('servers.relayPlan.traffic', { amount: gb(tot.monthly_bytes) })}</span>
      <span className="chip">
        {tot.no_session_limit ? t('servers.relayPlan.sessionUnlimited') : t('servers.relayPlan.session', { minutes: Math.round(tot.session_secs / 60) })}
      </span>
      <span className={`chip${tot.whitelist_page ? ' success' : ''}`}>
        {tot.whitelist_page ? t('servers.relayPlan.whitelistOn') : t('servers.relayPlan.whitelistOff')}
      </span>
    </div>
  )
  const freeLine = (
    <p className="hint">
      {t('servers.relayPlan.freeLine', { servers: plan.free.servers, names: plan.free.names, traffic: gb(plan.free.monthly_bytes), bonus: gb(plan.free.bonus_bytes) })}
    </p>
  )
  const closedNotice = plan.store_closed && (
    <div className="callout rp-notice" style={{ alignItems: 'center' }}>
      <ShoppingBag />
      <span>{t('servers.relayPlan.storeClosed')}</span>
    </div>
  )
  const redeemRow = (
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
  )
  const mine = plan.mine.length > 0 && (
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
            onMove={s.movable && moveTargets.length > 0 ? () => setMoving(s) : undefined}
          />
        ))}
      </div>
    </>
  )
  const period = (
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
  )
  const cartCard = (
    <div className="card rp-cart">
      <h4>
        <ShoppingCart /> {t('servers.relayPlan.cart')}
      </h4>
      {inCart.length === 0 && <p className="hint">{t('servers.relayPlan.cartEmpty')}</p>}
      {inCart.map((p) => (
        <div key={p.id} className="rp-line">
          <span className="grow">{p.name}</span>
          <b>{(yearly ? p.yearly : p.monthly) ? money(yearly ? p.yearly : p.monthly, plan.currency) : t('servers.relayPlan.notThisPeriod')}</b>
          <button className="btn ghost icon sm" title={t('servers.relayPlan.removeFromCart')} aria-label={t('servers.relayPlan.removeFromCart')} onClick={() => cart.remove(p.id)}>
            <X />
          </button>
        </div>
      ))}
      {inCart.length > 0 && (
        <>
          <div className="rp-line rp-total">
            <span className="grow">{t('servers.relayPlan.total')}</span>
            <b>
              {yearly ? t('servers.relayPlan.perYear', { price: money(cartTotal, plan.currency) }) : t('servers.relayPlan.perMonth', { price: money(cartTotal, plan.currency) })}
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
        </>
      )}
    </div>
  )
  const refresh = (
    <button className="btn ghost icon sm" title={t('common.refresh')} aria-label={t('common.refresh')} onClick={() => void load()}>
      <RefreshCw />
    </button>
  )
  /** A product as a row, for the Store page: what it is, what it gives, its price, and the two buttons. */
  const row = (p: PlanProduct) => {
    const price = yearly ? p.yearly : p.monthly
    if (!price) return null
    const others = othersWith(plans.accounts, a.account, p)
    return (
      <div key={p.id} className={`rp-row${p.owned ? ' owned' : ''}`}>
        <span className="rp-row-ic">{p.kind === 'pack' ? <Package /> : <Puzzle />}</span>
        <div className="rp-row-text">
          <b>{p.name}</b>
          <span className="hint">{gives(p)}</span>
          {others.length > 0 && <span className="hint it-warn">{t('servers.relayPlan.otherHas', { account: others.join(', ') })}</span>}
        </div>
        <div className="rp-row-price">
          {money(price, plan.currency)}
          <span className="hint"> / {yearly ? t('servers.relayPlan.year') : t('servers.relayPlan.month')}</span>
        </div>
        <div className="rp-row-buttons">
          <button className="btn sm primary" disabled={p.owned || busy != null} onClick={() => void buy(p)}>
            {busy === p.id ? <Loader2 className="spin" /> : p.owned ? <Check /> : <CreditCard />} {p.owned ? t('servers.relayPlan.owned') : t('servers.relayPlan.buy')}
          </button>
          {!p.owned && (
            <button
              className="btn sm icon"
              disabled={cart.items.includes(p.id)}
              title={cart.items.includes(p.id) ? t('servers.relayPlan.inCart') : t('servers.relayPlan.addToCart')}
              aria-label={cart.items.includes(p.id) ? t('servers.relayPlan.inCart') : t('servers.relayPlan.addToCart')}
              onClick={() => cart.add(p.id)}
            >
              {cart.items.includes(p.id) ? <Check /> : <Plus />}
            </button>
          )}
        </div>
      </div>
    )
  }

  const dialogs = (
    <>
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
        testMode={plan.test_mode}
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
    {moving && (
      <MoveModal
        a={a}
        sub={moving}
        targets={moveTargets}
        onClose={() => setMoving(undefined)}
        onMoved={(p, to) => {
          put(p)
          setMoving(undefined)
          toast(t('servers.relayPlan.moved', { name: moving.name, account: to.accountName }), 'success')
          // Both accounts' plans change: read them all again.
          void load()
        }}
      />
    )}
    {cancelling && (
      <ConfirmModal
        title={t('servers.relayPlan.cancelTitle', { name: cancelling.name })}
        body={
          (cancelling.cancel_credit ?? 0) > 0
            ? t('servers.wallet.cancelCreditBody', { amount: money(cancelling.cancel_credit!, plan.currency) })
            : t('servers.relayPlan.cancelBody', { date: cancelling.period_end ? day(cancelling.period_end) : '–' })
        }
        confirm={
          (cancelling.cancel_credit ?? 0) > 0
            ? t('servers.wallet.cancelCredit', { amount: money(cancelling.cancel_credit!, plan.currency) })
            : t('servers.relayPlan.cancel')
        }
        onClose={() => setCancelling(undefined)}
        onConfirm={async () => {
          try {
            put(await api.server.relayPlanCancel(a.account, cancelling.id, false))
            // A cancel that credits ended it now: the balance (and the wallet card's ledger) follow from the plan.
            if ((cancelling.cancel_credit ?? 0) > 0)
              toast(t('servers.wallet.cancelCredited', { name: cancelling.name, amount: money(cancelling.cancel_credit!, plan.currency) }), 'success')
            else toast(t('servers.relayPlan.cancelled', { name: cancelling.name }))
          } catch (e) {
            toast((e as Error).message, 'error')
          }
        }}
      />
    )}
    </>
  )

  // The Store page: the account's side of things in a panel that stays put, what is for sale to its right.
  if (page)
    return (
      <div className="relay-plan rp-page rp-split">
        <aside className="rp-aside">
          {heading}
          {accountTabs}
          {errors}
          {plan.wallet && <WalletCard key={a.account} a={a} onBalance={(b) => put({ ...plan, wallet_balance: b })} />}
          <div className="rp-have">
            <div className="row">
              <span className="hint grow">{t('servers.relayPlan.youHave', { account: a.accountName })}</span>
              {plan.test_mode && <span className="chip compact warn">{t('servers.relayPlan.testMode')}</span>}
              {refresh}
            </div>
            {totals}
            {freeLine}
          </div>
          {plan.products.length > 0 && cartCard}
          <div className="field">
            <span className="hint">{t('servers.relayPlan.giftCode')}</span>
            {redeemRow}
          </div>
        </aside>
        <div className="rp-main">
          {closedNotice}
          <Notices a={a} onRenew={renew} />
          {mine}
          {plan.products.length > 0 && (
            <>
              <div className="row rp-main-head">
                <h2 className="grow">
                  <Package /> {t('servers.relayPlan.packs')}
                </h2>
                {period}
              </div>
              {packs.length > 0 && <div className="rp-rows">{packs.map(row)}</div>}
              {addons.length > 0 && (
                <>
                  <h2>
                    <Puzzle /> {t('servers.relayPlan.addons')}
                  </h2>
                  <div className="rp-rows">{addons.map(row)}</div>
                </>
              )}
            </>
          )}
        </div>
        {dialogs}
      </div>
    )

  return (
    <div className="settings-section relay-plan">
      <div className="row">
        <h3 className="grow">{t('servers.relayPlan.title')}</h3>
        {plan.test_mode && <span className="chip compact warn">{t('servers.relayPlan.testMode')}</span>}
        {refresh}
      </div>
      <p className="hint">{t('servers.relayPlan.hint')}</p>
      {accountTabs}
      {errors}
      {totals}
      {freeLine}
      {closedNotice}
      <Notices a={a} onRenew={renew} />
      {redeemRow}
      {mine}
      {plan.products.length > 0 && (
        <>
          {period}
          {inCart.length > 0 && cartCard}
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
      {dialogs}
    </div>
  )
}
