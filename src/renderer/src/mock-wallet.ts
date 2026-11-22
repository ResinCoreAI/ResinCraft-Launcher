// Resin Credit (the Store's wallet) for the browser preview: each account's balance and ledger, top-ups (card and
// PromptPay through the mock payment form, a TrueMoney angpao at once), spending and the admin's side
// (Admin → Payments → Resin Credit). ?wallet=off previews a relay without Resin Credit, ?wallet=empty a zero balance.
import type { PayStep, WalletEntry, WalletInfo } from '@shared/types'

const mode = () => new URLSearchParams(location.search).get('wallet')
/** The relay has Resin Credit on (the plan's `wallet`). */
export const walletOn = () => mode() !== 'off'
const now = () => Math.floor(Date.now() / 1000)
const PAGE = 20
let seq = 1000

const admin = {
  enabled: true,
  switchFromFile: true,
  topups: [10000, 20000, 50000, 100000],
  topupsFromFile: true,
  fileTopups: [10000, 20000, 50000, 100000],
  pending: 1,
  refundOnCancel: true,
  refundOnCancelFromFile: true
}

interface Book {
  name: string
  balance: number
  /** Newest first. */
  history: WalletEntry[]
  /** Card and PromptPay top-ups started and not paid yet: id → amount. */
  pending: Record<string, number>
}
const books: Record<string, Book> = {}
const key = (uuid: string) => uuid.replace(/-/g, '').toLowerCase()

/** A ledger with history to page through for the preview's first account, a short one for the others. */
function seed(uuid: string, name: string, long: boolean): Book {
  const b: Book = { name, balance: 0, history: [], pending: {} }
  if (mode() === 'empty') return b
  const steps: [WalletEntry['kind'], number, string, string][] = long
    ? [
        ['topup', 50000, 'pi_mock_t1', 'Top-up by card'],
        ['purchase', -6900, 'wallet-mock-a', '+1 server online (1 month)'],
        ['purchase', -4900, 'wallet-mock-b', '+1 name (1 month)'],
        ['grant', 10000, '', 'Thanks for reporting the relay bug'],
        ['topup', 20000, 'tm-019a2b', 'TrueMoney angpao'],
        ['purchase', -19900, 'wallet-mock-c', '+1 server, +1 name, unlimited traffic (1 month)'],
        ['refund', 3200, 'wallet-mock-b', 'Refund of +1 name'],
        ['deduct', -2000, '', 'Duplicate grant taken back']
      ]
    : [
        ['topup', 10000, 'pi_mock_t9', 'Top-up by PromptPay'],
        ['purchase', -1900, 'wallet-mock-z', '+20 GB traffic (1 month)']
      ]
  // The long ledger repeats a few times so "Show more" has pages to load.
  const rounds = long ? 4 : 1
  let at = now() - 86400 * 90
  for (let r = 0; r < rounds; r++)
    for (const [kind, amount, ref, note] of steps) {
      at += 3600 * 31
      b.balance = Math.max(0, b.balance + amount)
      b.history.unshift({ id: ++seq, at, kind, amount, balance_after: b.balance, ref, note, by: kind === 'grant' || kind === 'deduct' ? 'Admin Puirkle_Gamer' : name })
    }
  void uuid
  return b
}

function book(uuid: string, name = ''): Book {
  const k = key(uuid)
  if (!books[k]) books[k] = seed(uuid, name, Object.keys(books).length === 0)
  if (name && !books[k].name) books[k].name = name
  return books[k]
}

function change(uuid: string, kind: string, amount: number, ref: string, note: string, by: string) {
  const b = book(uuid)
  if (b.balance + amount < 0) throw new Error(`the balance is only ${(b.balance / 100).toFixed(2)} THB: it can't go below 0`)
  b.balance += amount
  b.history.unshift({ id: ++seq, at: now(), kind, amount, balance_after: b.balance, ref, note, by })
}

const methods = () => (admin.enabled ? ['card', 'promptpay', 'truemoney'] : [])

/** The player's `wallet`: the newest page, or the one before `before`. */
export function mockWallet(uuid: string, name: string, before?: number | null): WalletInfo {
  const b = book(uuid, name)
  const older = before ? b.history.filter((e) => e.id < before) : b.history
  return {
    enabled: admin.enabled,
    balance: b.balance,
    currency: 'thb',
    topups: admin.topups,
    methods: methods(),
    history: older.slice(0, PAGE),
    more: older.length > PAGE
  }
}

export function mockWalletBalance(uuid: string, name: string) {
  return book(uuid, name).balance
}

/** `wallet_topup`: card or PromptPay give the payment form's step; a TrueMoney angpao is credited at once. */
export function mockWalletTopup(uuid: string, name: string, amount: number, method: string, link?: string | null): { pay?: PayStep; wallet?: WalletInfo } {
  if (!admin.enabled) throw new Error('That isn’t for sale, or this account has it already.')
  if (!admin.topups.includes(amount)) throw new Error('That isn’t for sale: pick one of the top-up amounts')
  const b = book(uuid, name)
  if (method === 'truemoney') {
    if (!link?.includes('v=')) throw new Error("That code doesn't work: that isn't a TrueMoney angpao link")
    change(uuid, 'topup', amount, `tm-${Math.random().toString(16).slice(2, 8)}`, 'TrueMoney angpao', name)
    return { wallet: mockWallet(uuid, name) }
  }
  const id = `pi_mock_topup_${++seq}`
  b.pending[id] = amount
  return {
    pay: { sub: id, client_secret: 'mock_secret', intent: 'payment', publishable_key: 'pk_mock', amount, currency: 'thb', subtotal: amount, once: true, methods: ['card', 'promptpay'] }
  }
}

/** `wallet_refresh`: the mock payment form "paid" it, so it's credited (once). */
export function mockWalletRefresh(uuid: string, name: string, id: string): WalletInfo {
  const b = book(uuid, name)
  const amount = b.pending[id]
  if (amount) {
    delete b.pending[id]
    change(uuid, 'topup', amount, id, 'Top-up by card', name)
  }
  return mockWallet(uuid, name)
}

/** Paying with credit: refused like the relay when the balance is short. */
export function mockWalletSpend(uuid: string, name: string, amount: number, ref: string, note: string) {
  const b = book(uuid, name)
  if (!admin.enabled) throw new Error('That isn’t for sale: Resin Credit is off on this relay')
  if (b.balance < amount)
    throw new Error(
      `Your Resin Credit doesn't cover this: not enough Resin Credit: this costs ${(amount / 100).toFixed(2)} THB, the balance is ${(b.balance / 100).toFixed(2)} THB (${((amount - b.balance) / 100).toFixed(2)} THB more is needed)`
    )
  change(uuid, 'purchase', -amount, ref, note, name)
}

/** What cancelling a running purchase now would credit (its unused time), while the admin refunds cancels. */
export function mockCancelCredit(s: { active: boolean; period_end: number; amount: number; yearly: boolean; status: string; refunded?: number; cancel_at_period_end: boolean }) {
  if (!walletOn() || !admin.enabled || !admin.refundOnCancel || !s.active || s.refunded || s.cancel_at_period_end || s.status === 'granted' || !s.amount) return 0
  return unused(s)
}

/** A cancel that credits: the unused time back as Resin Credit. */
export function mockWalletRefund(uuid: string, name: string, amount: number, ref: string, note: string) {
  change(uuid, 'refund', amount, ref, note, name)
}

/** The quote's Resin Credit fields. */
export function mockQuoteWallet(uuid: string, name: string, total: number, ok: boolean) {
  if (!walletOn()) return {}
  const balance = book(uuid, name).balance
  const short = total > balance
  return {
    wallet: true,
    wallet_balance: balance,
    wallet_covers: admin.enabled && ok && !short,
    wallet_note:
      "Resin Credit pays for one month or one year at a time; a subscription that renews by itself can't be paid with it." +
      (short ? ` ${((total - balance) / 100).toFixed(2)} THB more is needed (the balance is ${(balance / 100).toFixed(2)} THB).` : '')
  }
}

// ---------- Admin → Payments → Resin Credit (GET /api/payments/wallet, POST /api/payments/wallet/…) ----------

type Row = Record<string, any> // eslint-disable-line @typescript-eslint/no-explicit-any

export const walletAdminOn = () => (walletOn() ? admin.enabled : undefined)

const NAMES: Record<string, string> = {
  '069a79f4-44e9-4726-a5be-fca90e38aaf5': 'Alex',
  '8667ba71-b85a-4004-af54-457a9734eed7': 'Puirkle_Gamer',
  '11111111-2222-3333-4444-555566667777': 'Steve'
}

/** The relay's answer, with `account` when one is named. `subs` and `products` are the Payments mock's. */
export function mockWalletAdmin(method: string, path: string, b: Row, subs: Row[], products: Row[]): unknown {
  for (const [uuid, name] of Object.entries(NAMES)) book(uuid, name)
  const [p, query] = path.split('?')
  const q = new URLSearchParams(query ?? '')
  const by = 'admin Puirkle_Gamer (Puirkle_Gamer) via launcher'
  const findUuid = (w: { uuid?: string; name?: string }) => {
    if (w.uuid) return w.uuid
    const hit = Object.entries(NAMES).find(([, n]) => n.toLowerCase() === String(w.name ?? '').toLowerCase())
    if (!hit) throw new Error(`The relay refused: no account named ${w.name} has used this relay`)
    return hit[0]
  }
  let who: string | undefined = q.get('uuid') ?? (q.get('name') ? findUuid({ name: q.get('name')! }) : undefined)
  if (method === 'POST') {
    if (p.endsWith('/refunds')) {
      if (b.reset) Object.assign(admin, { refundOnCancel: true, refundOnCancelFromFile: true })
      else Object.assign(admin, { refundOnCancel: !!b.on, refundOnCancelFromFile: false })
    } else if (p.endsWith('/switch')) {
      if (b.reset) Object.assign(admin, { enabled: true, switchFromFile: true })
      else Object.assign(admin, { enabled: !!b.on, switchFromFile: false })
    } else if (p.endsWith('/topups')) {
      if (b.reset) Object.assign(admin, { topups: [...admin.fileTopups], topupsFromFile: true })
      else {
        const list = [...new Set((b.amounts as number[]).map(Number))].sort((x, y) => x - y)
        if (!list.length || list.length > 12 || list.some((n) => !(n >= 1000 && n <= 10_000_000))) throw new Error('The relay refused: amounts: 1 to 12, each from 10.00 THB to 100000.00 THB')
        Object.assign(admin, { topups: list, topupsFromFile: false })
      }
    } else if (p.endsWith('/account')) who = findUuid(b)
    else if (p.endsWith('/grant') || p.endsWith('/deduct')) {
      who = findUuid(b)
      const deduct = p.endsWith('/deduct')
      try {
        change(who, deduct ? 'deduct' : 'grant', deduct ? -b.amount : b.amount, '', String(b.note ?? '').trim(), by)
      } catch (e) {
        throw new Error(`The relay refused: ${(e as Error).message}`)
      }
    } else if (p.endsWith('/refund')) {
      const s = subs.find((x) => x.id === b.id)
      if (!s) throw new Error('The relay refused: no such purchase')
      if (s.refunded) throw new Error('The relay refused: it was given back as Resin Credit already')
      who = s.uuid as string
      const amount = Number(b.amount) || unused(s) || s.amount
      change(who, 'refund', amount, s.id, String(b.note ?? '').trim() || `Refund of ${productName(s, products)}`, by)
      Object.assign(s, { refunded: amount, status: 'canceled', active: false })
    }
  }
  const all = Object.entries(books)
  const entry = (k: string, e: WalletEntry) => {
    const uuid = Object.keys(NAMES).find((u) => key(u) === k) ?? k
    return { id: e.id, at: e.at, kind: e.kind, amount: e.amount, balanceAfter: e.balance_after, ref: e.ref, note: e.note, by: e.by, uuid, name: books[k].name }
  }
  const out: Row = {
    enabled: admin.enabled,
    switchFromFile: admin.switchFromFile,
    live: true,
    currency: 'thb',
    minCharge: 1000,
    topups: admin.topups,
    topupsFromFile: admin.topupsFromFile,
    fileTopups: admin.fileTopups,
    refundOnCancel: admin.refundOnCancel,
    refundOnCancelFromFile: admin.refundOnCancelFromFile,
    methods: methods(),
    outstanding: all.reduce((n, [, x]) => n + x.balance, 0),
    accounts: all.filter(([, x]) => x.balance > 0).length,
    pending: admin.pending,
    balances: all
      .filter(([, x]) => x.balance > 0)
      .map(([k, x]) => ({ uuid: Object.keys(NAMES).find((u) => key(u) === k) ?? k, name: x.name, balance: x.balance, updated: x.history[0]?.at ?? 0 }))
      .sort((x, y) => y.balance - x.balance),
    recent: all
      .flatMap(([k, x]) => x.history.map((e) => entry(k, e)))
      .sort((x, y) => y.id - x.id)
      .slice(0, 50)
  }
  if (who) {
    const x = book(who, NAMES[who] ?? '')
    out.account = {
      uuid: who,
      name: x.name,
      balance: x.balance,
      history: x.history.map((e) => ({ ...entry(key(who!), e), uuid: undefined, name: undefined })),
      older: x.history.length > 20 ? { entries: 14, net: 12000, from: now() - 86400 * 400, until: now() - 86400 * 100 } : null,
      purchases: subs
        .filter((s) => key(String(s.uuid)) === key(who!) && s.status !== 'pending')
        .map((s) => ({
          id: s.id,
          name: productName(s, products),
          source: s.source,
          method: s.method,
          yearly: s.yearly,
          status: s.status,
          amount: s.amount,
          active: s.active,
          periodEnd: s.period_end,
          created: now() - 86400 * 10,
          unused: unused(s),
          refunded: s.refunded ?? 0
        }))
    }
  }
  return out
}

function productName(s: Row, products: Row[]) {
  return products.find((x) => x.id === s.product)?.name ?? s.product
}

/** What a running purchase's days left are worth (price × whole days left ÷ days in its period). */
function unused(s: Row) {
  if (!s.active || !s.period_end) return 0
  const days = Math.max(0, Math.floor((s.period_end - now()) / 86400))
  return Math.round((s.amount * days) / (s.yearly ? 365 : 30))
}
