import { useEffect, useState } from 'react'
import { api } from './api'
import { useStore } from './store'

// Whether ResinCraft Relay sells anything to one of the signed-in accounts (payments on): the Store shows then.
// One answer for the whole window (the sidebar and the Servers tab's link), asked again on every page change at most
// every 30 seconds, every minute while it's closed (the relay sign-in may not be done at start), and every ten minutes
// once open (the relay's admin can switch payments off).
let open = false
/** Resin Credit per account (UUID without dashes, lowercase) while its relay plan says `wallet`: the header shows it. */
let wallets: Record<string, { balance: number; currency: string }> = {}
let asked = 0
let asking: Promise<void> | null = null
const listeners = new Set<(open: boolean) => void>()

function check(force = false): Promise<void> {
  if (asking) return asking
  if (!force && Date.now() - asked < 30_000) return Promise.resolve()
  asking = api.server
    .relayPlans()
    .then(
      (p) => {
        open = p.available && p.accounts.some((a) => a.plan?.enabled)
        wallets = Object.fromEntries(
          p.accounts.filter((a) => a.plan?.enabled && a.plan.wallet).map((a) => [uuidKey(a.account), { balance: a.plan!.wallet_balance ?? 0, currency: a.plan!.currency }])
        )
      },
      () => {
        open = false
        wallets = {}
      }
    )
    .finally(() => {
      asked = Date.now()
      asking = null
      listeners.forEach((f) => f(open))
      walletListeners.forEach((f) => f())
    })
  return asking
}

const uuidKey = (uuid: string) => uuid.replace(/-/g, '').toLowerCase()

/** The Store's prices and balances: the currency's smallest unit (satang) as money, whole when it's whole. */
export function money(amount: number, currency: string) {
  try {
    return new Intl.NumberFormat(undefined, { style: 'currency', currency: currency.toUpperCase(), maximumFractionDigits: amount % 100 ? 2 : 0 }).format(
      amount / 100
    )
  } catch {
    return `${(amount / 100).toLocaleString()} ${currency.toUpperCase()}`
  }
}

const walletListeners = new Set<() => void>()

/** The Store changed an account's balance (a top-up, a purchase paid with credit): the header shows it now. */
export function setWalletBalance(uuid: string, balance: number, currency: string) {
  wallets = { ...wallets, [uuidKey(uuid)]: { balance, currency } }
  walletListeners.forEach((f) => f())
}

/** An account's Resin Credit as the Store last read it (undefined: no wallet, or not asked yet). */
export function walletOf(uuid: string | undefined) {
  return uuid ? wallets[uuidKey(uuid)] : undefined
}

/** `walletOf`, re-rendering when it changes (useStoreOpen does the asking). */
export function useWalletOf(uuid: string | undefined) {
  const [, bump] = useState(0)
  useEffect(() => {
    const f = () => bump((n) => n + 1)
    walletListeners.add(f)
    return () => void walletListeners.delete(f)
  }, [])
  return walletOf(uuid)
}

export function useStoreOpen(): boolean {
  const { activeAccount, route } = useStore()
  const [now, setNow] = useState(open)
  useEffect(() => {
    listeners.add(setNow)
    let timer = 0
    const loop = () => {
      timer = window.setTimeout(() => void check(true).then(loop), open ? 10 * 60_000 : 60_000)
    }
    void check().then(loop)
    return () => {
      listeners.delete(setNow)
      window.clearTimeout(timer)
    }
  }, [])
  // A new account, or another page: ask again (at most every 30 seconds).
  useEffect(() => void check(), [activeAccount?.id, route.page])
  return now
}
