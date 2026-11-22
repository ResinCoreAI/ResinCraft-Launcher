// Settings → Store history: one Microsoft account's Resin Credit top-ups, everything that moved its balance, and what it
// has bought, from the relay (the Store's plan and its `wallet` ledger, paged). Kept out of the Store itself; the
// Store's History button opens it on Top-ups (`openStoreHistory`).
import { useEffect, useState } from 'react'
import { History, Receipt } from 'lucide-react'
import type { AccountPlan, PlanSub, WalletEntry } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { t, type Key } from '../i18n'
import { money } from '../storeStatus'
import { Segmented } from './ui'
import { WalletEntries } from './RelayPlan'

type Tab = 'topups' | 'activity' | 'purchases'

let nextTab: Tab | undefined
/** From the Store: Settings → Store history, on Top-ups (or another tab). */
export function openStoreHistory(go: (r: { page: 'settings'; section: string }) => void, tab: Tab = 'topups') {
  nextTab = tab
  go({ page: 'settings', section: 'store-history' })
}

const METHOD: Record<string, Key> = {
  card: 'servers.wallet.method.card',
  promptpay: 'servers.wallet.method.promptpay',
  truemoney: 'servers.wallet.method.truemoney',
  wallet: 'servers.wallet.title'
}
const day = (unix: number) => new Date(unix * 1000).toLocaleDateString(undefined, { dateStyle: 'medium' })

export function StoreHistorySection() {
  const { activeAccount } = useStore()
  const [accounts, setAccounts] = useState<AccountPlan[]>()
  const [error, setError] = useState<string>()
  const [who, setWho] = useState<string>()
  const [tab, setTab] = useState<Tab>(() => nextTab ?? 'topups')
  useEffect(() => {
    nextTab = undefined
    api.server.relayPlans().then(
      (r) => {
        const list = r.accounts.filter((a) => a.plan)
        setAccounts(list)
        setWho((w) => w ?? list.find((a) => a.account === activeAccount?.id)?.account ?? list[0]?.account)
      },
      (e) => setError((e as Error).message)
    )
  }, [])

  return (
    <section className="settings-section" id="store-history">
      <h3>{t('storeHistory.title')}</h3>
      <p>{t('storeHistory.lead')}</p>
      {error ? (
        <p className="hint bad">{error}</p>
      ) : !accounts ? (
        <p className="hint">{t('common.loading')}</p>
      ) : accounts.length === 0 ? (
        <p className="hint">{t('storeHistory.none')}</p>
      ) : (
        <div className="card pad sh-card">
          <div className="row wrap sh-bar">
            {accounts.length > 1 && (
              <Segmented value={who ?? ''} onChange={setWho} options={accounts.map((a) => ({ value: a.account, label: a.accountName }))} />
            )}
            <span className="grow" />
            <Segmented<Tab>
              value={tab}
              onChange={setTab}
              options={[
                { value: 'topups', label: t('storeHistory.tab.topups') },
                { value: 'activity', label: t('storeHistory.tab.activity') },
                { value: 'purchases', label: t('storeHistory.tab.purchases') }
              ]}
            />
          </div>
          {who && <AccountHistory key={`${who}:${tab}`} a={accounts.find((x) => x.account === who)!} tab={tab} />}
        </div>
      )}
    </section>
  )
}

function AccountHistory({ a, tab }: { a: AccountPlan; tab: Tab }) {
  const plan = a.plan!
  const cur = plan.currency
  if (tab === 'purchases') return <Purchases subs={plan.mine ?? []} currency={cur} />
  if (!plan.wallet) return <p className="hint">{t('storeHistory.noCredit')}</p>
  return <Ledger account={a.account} currency={cur} only={tab === 'topups' ? ['topup', 'gift'] : undefined} />
}

/** The Resin Credit ledger, newest first, read a page at a time (`only`: just these kinds). */
function Ledger({ account, currency, only }: { account: string; currency: string; only?: string[] }) {
  const [entries, setEntries] = useState<WalletEntry[]>([])
  const [more, setMore] = useState(true)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string>()
  const load = async (before?: number) => {
    setBusy(true)
    try {
      const w = await api.server.relayWallet(account, before)
      setEntries((old) => [...old, ...w.history.filter((e) => !old.some((x) => x.id === e.id))])
      setMore(w.more)
    } catch (e) {
      setError((e as Error).message)
    } finally {
      setBusy(false)
    }
  }
  useEffect(() => void load(), [account])
  const shown = only ? entries.filter((e) => only.includes(e.kind)) : entries
  // Top-ups are a few among many: keep reading older pages until a screenful shows (or there are no more).
  useEffect(() => {
    if (only && more && !busy && !error && shown.length < 10 && entries.length > 0) void load(entries[entries.length - 1].id)
  }, [entries.length, more])
  return (
    <>
      {shown.length > 0 ? (
        <WalletEntries entries={shown} currency={currency} />
      ) : (
        !busy && !error && <p className="hint">{t(only ? 'storeHistory.noTopups' : 'storeHistory.noActivity')}</p>
      )}
      {error && <p className="hint bad">{error}</p>}
      {more && entries.length > 0 && (
        <div className="row end">
          <button className="btn sm" disabled={busy} onClick={() => void load(entries[entries.length - 1].id)}>
            <History /> {t('storeHistory.older')}
          </button>
        </div>
      )}
    </>
  )
}

/** Everything bought or given on this account: running first, then the ones that ended. */
function Purchases({ subs, currency }: { subs: PlanSub[]; currency: string }) {
  if (subs.length === 0) return <p className="hint">{t('storeHistory.noPurchases')}</p>
  const sorted = [...subs].sort((x, y) => Number(y.active) - Number(x.active) || y.period_end - x.period_end)
  return (
    <div className="sh-list">
      {sorted.map((s) => {
        const status: Key = s.refunded
          ? 'storeHistory.st.refunded'
          : s.active
            ? s.cancel_at_period_end
              ? 'storeHistory.st.ending'
              : 'storeHistory.st.active'
            : 'storeHistory.st.ended'
        const how = s.paid ? (s.method && METHOD[s.method] ? t(METHOD[s.method]) : s.once ? '' : t('storeHistory.renews')) : t('storeHistory.given')
        return (
          <div key={s.id} className="sh-row">
            <Receipt />
            <div className="grow">
              <b>{s.items?.length ? s.items.map((i) => i.name).join(', ') : s.name}</b>
              <span className="hint">
                {[s.paid ? `${money(s.amount, currency)} / ${t(s.yearly ? 'storeHistory.year' : 'storeHistory.month')}` : '', how, s.period_end ? t(s.active ? 'storeHistory.until' : 'storeHistory.endedOn', { date: day(s.period_end) }) : '']
                  .filter(Boolean)
                  .join(' · ')}
              </span>
              {s.refunded ? <span className="hint">{t('storeHistory.refundedAs', { amount: money(s.refunded, currency) })}</span> : null}
              {s.note && <span className="hint">{s.note}</span>}
            </div>
            <span className={`chip compact ${s.active ? 'success' : ''}`}>{t(status)}</span>
          </div>
        )
      })}
    </div>
  )
}
