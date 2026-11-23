// Admin → Payments: the relay's selling switch, free allowance, products and packs, promo and gift codes, purchases,
// unused-days credit, ended add-ons (grace and cut-off) and TrueMoney angpao. Same routes as the relay dashboard's
// Payments page (GET /api/payments, POST /api/payments/…); amounts are in the currency's smallest unit.
import { useEffect, useState, type ReactNode } from 'react'
import { Ban, Check, CloudUpload, Copy, CreditCard, Gift, Pencil, Plus, RefreshCw, Search, Send, ShoppingBag, Ticket, Trash2, Wallet, X } from 'lucide-react'
import { currentLanguage, t, type Key } from '../../i18n'
import { useStore } from '../../store'
import { ConfirmModal, Empty, Modal, MoreButton, Segmented, Switch, type ContextEntry } from '../ui'
import { account, AskModal, Loading, post, shortId, useAct, useAdmin, when } from './common'
import { UNLIMITED_COUNT } from '../../relayCount'
import { WalletAdmin } from './Wallet'
import { Sales, type Sale } from './Sales'
import { isProductIcon, ProductIcon, StoreProductRow } from '../StoreProduct'
import '../../relay-plan.css'
import '../../pay-product.css'

type Grants = {
  servers?: number
  names?: number
  traffic_gb?: number
  /** Minutes added to every session (relays that send productIcons). */
  session_minutes?: number
  unlimited_traffic?: boolean
  no_session_limit?: boolean
  whitelist_page?: boolean
}
interface Product {
  id: string
  name: string
  description?: string
  kind: 'addon' | 'pack'
  grants: Grants
  monthly: number
  yearly: number
  on_sale: boolean
  order?: number
  stripe_product?: string
  stripe_price_monthly?: string
  stripe_price_yearly?: string
  active: number
  /** Cancelling it can give its unused time back as Resin Credit (relays with Resin Credit; absent: true). */
  refundable?: boolean
  /** The Store card's look (relays that send productIcons): a short label, standing out, and one of productIcons. */
  badge?: string
  highlight?: boolean
  icon?: string
}
interface Sub {
  id: string
  uuid: string
  name: string
  product: string
  yearly: boolean
  status: string
  period_end: number
  cancel_at_period_end: boolean
  amount: number
  source: string
  method?: string
  credit?: number
  active: boolean
  /** The account it was moved from (its name then), and when. */
  moved_from?: string
  moved_at?: number
  /** The admin's note to the player. */
  note?: string
  /** Given back as Resin Credit (Refund as credit). */
  refunded?: number
}
interface Promo {
  code: string
  percent_off?: number
  amount_off?: number
  duration: 'once' | 'repeating' | 'forever'
  months?: number
  max_redemptions?: number
  expires_at?: number
  products: string[]
  active: boolean
  usable: boolean
  uses: number
  below_minimum?: string[]
}
interface GiftCode {
  code: string
  product: string
  /** A Resin Credit code: what it adds to the balance (smallest unit), instead of a product. */
  credit?: number
  days: number
  max_uses: number
  expires_at?: number
  active: boolean
  usable: boolean
  note?: string
  redeemed: { uuid: string; name: string; at: number }[]
}
type Free = { servers: number; names: number; monthly_gb: number; bonus_gb: number; session_minutes: number }
interface Payments {
  /** Sales (sales.rs); absent on relays before them. */
  sales?: Sale[]
  enabled: boolean
  switchFromFile: boolean
  creditUnused: boolean
  /** The Store sells while selling is on (fromFile: relay.toml's `[payments] store_open`). */
  storeOpen?: { on: boolean; fromFile: boolean }
  /** Players may move purchases between their accounts (fromFile: relay.toml's `[payments] moves`). */
  moves?: { on: boolean; fromFile: boolean }
  free: Free
  freeFromFile: boolean
  fileFree: Free
  currency: string
  stripe: { secretKey: boolean; publishableKey: boolean; webhookSecret: boolean; testMode: boolean; ready: boolean; webhookUrl: string }
  products: Product[]
  subs: Sub[]
  promos: Promo[]
  gifts: GiftCode[]
  access?: {
    cutoff: boolean
    graceDays: number
    grace: { uuid: string; name?: string; until: number; servers: number; names: number; serversAfter: number; namesAfter: number; willPause: string[] }[]
    cut: { uuid: string; name?: string; since: number; paused: string[] }[]
  }
  angpao: {
    at: number
    uuid: string
    name: string
    voucher: string
    amount: number
    price: number
    items: string[]
    result: string
    overpaid: number
    sub: string
  }[]
  promptpayRefused?: boolean
  truemoney?: { on: boolean; phone: string; enabled?: boolean; fromFile?: boolean }
  warning?: string
  made?: string[]
  /** Resin Credit's switch (relays without Resin Credit leave it out). */
  walletOn?: boolean
  /** The icons a product card can have; relays from before session minutes, badges and icons leave it out. */
  productIcons?: string[]
}

type View = 'selling' | 'products' | 'codes' | 'sales' | 'purchases' | 'truemoney' | 'wallet'
const COUNTS = ['servers', 'names', 'traffic_gb', 'session_minutes'] as const
/** The relay's limits for a product (its payments.rs). */
const MAX_BADGE = 20
const MAX_SESSION_MINUTES = 43200
const SWITCHES = ['unlimited_traffic', 'no_session_limit', 'whitelist_page'] as const
/** Counts that can be ∞ (no limit), like Unlimited in an account's own settings. */
const INF = ['servers', 'names'] as const
const FREE = ['servers', 'names', 'monthly_gb', 'bonus_gb', 'session_minutes'] as const

function money(amount: number, currency: string) {
  try {
    return new Intl.NumberFormat(currentLanguage(), {
      style: 'currency',
      currency: currency.toUpperCase(),
      maximumFractionDigits: amount % 100 ? 2 : 0
    }).format(amount / 100)
  } catch {
    return `${(amount / 100).toLocaleString()} ${currency.toUpperCase()}`
  }
}

function gives(g: Grants) {
  const parts: string[] = []
  if ((g.servers ?? 0) >= UNLIMITED_COUNT) parts.push(t('servers.admin.pay.giveServersInf'))
  else if (g.servers) parts.push(t('servers.admin.pay.giveServers', { count: g.servers }))
  if ((g.names ?? 0) >= UNLIMITED_COUNT) parts.push(t('servers.admin.pay.giveNamesInf'))
  else if (g.names) parts.push(t('servers.admin.pay.giveNames', { count: g.names }))
  if (g.traffic_gb) parts.push(t('servers.admin.pay.giveTraffic', { count: g.traffic_gb }))
  if (g.session_minutes && !g.no_session_limit) parts.push(t('servers.admin.pay.giveSessionMinutes', { minutes: g.session_minutes }))
  for (const k of SWITCHES) if (g[k]) parts.push(t(`servers.admin.pay.grant.${k}` as Key))
  return parts.join(', ') || t('servers.admin.pay.giveNothing')
}

export function PaymentsAdmin() {
  const { data, error, reload } = useAdmin<Payments>('/api/payments')
  const act = useAct(reload)
  const [view, setView] = useState<View>('selling')
  // Refund as credit on a purchase: Resin Credit with its account open, asking about that purchase.
  const [walletOpen, setWalletOpen] = useState<{ uuid: string; refund?: string }>()
  if (!data) return <Loading error={error} />
  const hasWallet = data.walletOn !== undefined
  const show = (v: View) => {
    if (v !== 'wallet') setWalletOpen(undefined)
    setView(v)
  }
  return (
    <div className="pay-admin">
      <div className="toolbar">
        <Segmented<View>
          value={view}
          onChange={show}
          options={[
            { value: 'selling', label: t('servers.admin.pay.view.selling') },
            { value: 'products', label: t('servers.admin.pay.view.products') },
            { value: 'codes', label: t('servers.admin.pay.view.codes') },
            { value: 'sales', label: t('servers.admin.sales.title') },
            { value: 'purchases', label: t('servers.admin.pay.view.purchases') },
            { value: 'truemoney', label: t('servers.admin.pay.view.truemoney') },
            ...(hasWallet ? [{ value: 'wallet' as const, label: t('servers.wallet.title') }] : [])
          ]}
        />
        <span className="grow" />
        <button className="btn sm ghost" title={t('servers.admin.pay.reload')} onClick={() => void reload()}>
          <RefreshCw />
        </button>
      </div>
      {view === 'selling' ? (
        <Selling d={data} act={act} />
      ) : view === 'products' ? (
        <Products d={data} act={act} reload={reload} />
      ) : view === 'codes' ? (
        <Codes d={data} act={act} reload={reload} />
      ) : view === 'sales' ? (
        <Sales sales={data.sales ?? []} products={data.products} act={(path, body) => act(path, body, 'servers.admin.pay.saved')} />
      ) : view === 'purchases' ? (
        <Purchases
          d={data}
          act={act}
          onRefund={(s) => {
            setWalletOpen({ uuid: s.uuid, refund: s.id })
            setView('wallet')
          }}
        />
      ) : view === 'wallet' && hasWallet ? (
        <WalletAdmin key={walletOpen ? `${walletOpen.uuid}:${walletOpen.refund}` : ''} open={walletOpen} />
      ) : (
        <TrueMoney d={data} act={act} />
      )}
    </div>
  )
}

type Act = ReturnType<typeof useAct>

/** A switch row: title, what it does now, and the switch. */
function SwitchRow({ title, hint, on, onChange }: { title: string; hint: string; on: boolean; onChange: (v: boolean) => void }) {
  return (
    <div className="setting">
      <div className="text">
        <b>{title}</b>
        <span>{hint}</span>
      </div>
      <Switch on={on} label={title} onChange={onChange} />
    </div>
  )
}

function Selling({ d, act }: { d: Payments; act: Act }) {
  const st = d.stripe
  const [stopping, setStopping] = useState(false)
  const [closing, setClosing] = useState(false)
  const [free, setFree] = useState<Record<string, string>>({})
  const ac = d.access ?? { cutoff: true, graceDays: 3, grace: [], cut: [] }
  const [grace, setGrace] = useState(String(ac.graceDays))
  useEffect(() => setFree(Object.fromEntries(FREE.map((k) => [k, String(d.free[k])]))), [d.free])
  useEffect(() => setGrace(String(ac.graceDays)), [ac.graceDays])
  return (
    <>
      <div className="settings-section">
        <h3>{t('servers.admin.pay.selling')}</h3>
        <div className="card">
          <SwitchRow
            title={t('servers.admin.pay.sellingSwitch')}
            hint={t(d.enabled ? 'servers.admin.pay.sellingOn' : 'servers.admin.pay.sellingOff')}
            on={d.enabled}
            onChange={(on) => (on ? void act('/api/payments/switch', { on: true }, 'servers.admin.pay.saved') : setStopping(true))}
          />
          {d.switchFromFile && (
            <div className="setting">
              <span className="hint">{t('servers.admin.pay.fromFile')}</span>
            </div>
          )}
          {d.enabled && d.storeOpen && (
            <SwitchRow
              title={t('servers.admin.pay.storeOpen')}
              hint={t(d.storeOpen.on ? 'servers.admin.pay.storeOpenOn' : 'servers.admin.pay.storeOpenOff')}
              on={d.storeOpen.on}
              onChange={(on) => (on ? void act('/api/payments/store', { on: true }, 'servers.admin.pay.saved') : setClosing(true))}
            />
          )}
          {d.enabled && d.storeOpen && !d.storeOpen.fromFile && (
            <div className="setting">
              <span className="hint grow">{t('servers.admin.pay.storeOverride')}</span>
              <button className="btn sm" onClick={() => void act('/api/payments/store', { reset: true }, 'servers.admin.pay.saved')}>
                {t('servers.admin.set.reset')}
              </button>
            </div>
          )}
          <div className="setting stack">
            <span className={st.ready ? 'hint' : 'hint bad'}>
              {st.ready ? t(st.testMode ? 'servers.admin.pay.stripeTest' : 'servers.admin.pay.stripeLive') : t('servers.admin.pay.stripeMissing')}{' '}
              {st.webhookSecret ? t('servers.admin.pay.webhook', { url: st.webhookUrl }) : t('servers.admin.pay.noWebhook', { url: st.webhookUrl })}
            </span>
            <span className="hint">{t('servers.admin.pay.currency', { currency: d.currency.toUpperCase() })}</span>
            {d.promptpayRefused && (
              <div className="row">
                <span className="hint bad grow">{t('servers.admin.pay.promptpayRefused')}</span>
                <button className="btn sm" onClick={() => void act('/api/payments/promptpay/retry', {}, 'servers.admin.pay.saved')}>
                  {t('servers.admin.pay.promptpayRetry')}
                </button>
              </div>
            )}
          </div>
        </div>
      </div>

      <div className="settings-section">
        <h3>{t('servers.admin.pay.free')}</h3>
        <div className="card">
          <div className="setting stack">
            <span className="hint">{t('servers.admin.pay.freeHint')}</span>
            <div className="admin-limits">
              {FREE.map((k) => (
                <label key={k} className="field">
                  <span>
                    {t(`servers.admin.pay.free.${k}` as Key)}
                    {!d.freeFromFile && d.fileFree[k] !== d.free[k] && <em className="hint"> (relay.toml: {d.fileFree[k]})</em>}
                  </span>
                  <input className="input" inputMode="decimal" value={free[k] ?? ''} onChange={(e) => setFree({ ...free, [k]: e.target.value })} />
                </label>
              ))}
            </div>
            <div className="row end">
              {!d.freeFromFile && (
                <button className="btn sm" onClick={() => void act('/api/payments/free', { reset: true }, 'servers.admin.pay.saved')}>
                  {t('servers.admin.set.reset')}
                </button>
              )}
              <button
                className="btn sm primary"
                onClick={() => void act('/api/payments/free', Object.fromEntries(FREE.map((k) => [k, Number(free[k]) || 0])), 'servers.admin.pay.saved')}
              >
                {t('servers.admin.set.save')}
              </button>
            </div>
          </div>
        </div>
      </div>

      <div className="settings-section">
        <h3>{t('servers.admin.pay.unused')}</h3>
        <div className="card">
          <SwitchRow
            title={t('servers.admin.pay.unusedSwitch')}
            hint={t(d.creditUnused ? 'servers.admin.pay.unusedOn' : 'servers.admin.pay.unusedOff')}
            on={d.creditUnused}
            onChange={(on) => void act('/api/payments/credit', { on }, 'servers.admin.pay.saved')}
          />
        </div>
      </div>

      <div className="settings-section">
        <h3>{t('servers.admin.pay.moves')}</h3>
        <div className="card admin-items">
          <SwitchRow
            title={t('servers.admin.pay.movesSwitch')}
            hint={t(d.moves?.on ? 'servers.admin.pay.movesOn' : 'servers.admin.pay.movesOff')}
            on={!!d.moves?.on}
            onChange={(on) => void act('/api/payments/moves', { on }, 'servers.admin.pay.saved')}
          />
          {d.moves && !d.moves.fromFile && (
            <div className="setting">
              <span className="hint grow">{t('servers.admin.pay.movesOverride')}</span>
              <button className="btn sm" onClick={() => void act('/api/payments/moves', { reset: true }, 'servers.admin.pay.saved')}>
                {t('servers.admin.set.reset')}
              </button>
            </div>
          )}
        </div>
      </div>

      <div className="settings-section">
        <h3>{t('servers.admin.pay.endedAddons')}</h3>
        <div className="card admin-items">
          <SwitchRow
            title={t('servers.admin.pay.cutoff')}
            hint={ac.cutoff ? t('servers.admin.pay.cutoffOn', { days: ac.graceDays }) : t('servers.admin.pay.cutoffOff')}
            on={ac.cutoff}
            onChange={(cutoff) => void act('/api/payments/access', { cutoff }, 'servers.admin.pay.saved')}
          />
          <div className="setting">
            <div className="text">
              <b>{t('servers.admin.pay.graceDays')}</b>
              <span>{t('servers.admin.pay.graceDaysHint')}</span>
            </div>
            <input className="input" style={{ width: '5rem' }} inputMode="numeric" value={grace} onChange={(e) => setGrace(e.target.value)} />
            <button
              className="btn sm"
              disabled={grace === String(ac.graceDays)}
              onClick={() => void act('/api/payments/access', { grace_days: Math.max(0, Math.floor(Number(grace) || 0)) }, 'servers.admin.pay.saved')}
            >
              {t('servers.admin.set.save')}
            </button>
          </div>
          {ac.grace.length === 0 && ac.cut.length === 0 && (
            <div className="setting admin-item">
              <span className="hint">{t('servers.admin.pay.nobodyHeld')}</span>
            </div>
          )}
          {ac.grace.map((g) => (
            <div key={g.uuid} className="setting admin-item">
              <div className="text">
                <b>
                  {g.name || shortId(g.uuid)} <span className="chip compact warn">{t('servers.admin.pay.inGrace')}</span>
                </b>
                <span>
                  {t('servers.admin.pay.graceLine', {
                    until: when(g.until),
                    servers: g.servers,
                    names: g.names,
                    serversAfter: g.serversAfter,
                    namesAfter: g.namesAfter
                  })}
                </span>
                {g.willPause.length > 0 && <span className="hint">{t('servers.admin.pay.willPause', { names: g.willPause.join(', ') })}</span>}
              </div>
            </div>
          ))}
          {ac.cut.map((c) => (
            <div key={c.uuid} className="setting admin-item">
              <div className="text">
                <b>
                  {c.name || shortId(c.uuid)} <span className="chip compact danger">{t('servers.admin.pay.cut')}</span>
                </b>
                <span>{t('servers.admin.pay.cutLine', { since: when(c.since), names: c.paused.join(', ') || '—' })}</span>
              </div>
            </div>
          ))}
        </div>
      </div>

      {closing && (
        <ConfirmModal
          title={t('servers.admin.pay.closeTitle')}
          body={t('servers.admin.pay.closeBody')}
          confirm={t('servers.admin.pay.close')}
          onConfirm={() => void act('/api/payments/store', { on: false }, 'servers.admin.pay.saved')}
          onClose={() => setClosing(false)}
        />
      )}
      {stopping && (
        <ConfirmModal
          title={t('servers.admin.pay.stopTitle')}
          body={t('servers.admin.pay.stopBody')}
          confirm={t('servers.admin.pay.stop')}
          onConfirm={() => void act('/api/payments/switch', { on: false }, 'servers.admin.pay.saved')}
          onClose={() => setStopping(false)}
        />
      )}
    </>
  )
}

function Products({ d, act, reload }: { d: Payments; act: Act; reload: () => unknown }) {
  const { toast } = useStore()
  // A product's id (its editor), or a new one.
  const [editing, setEditing] = useState<{ id: string } | 'new'>()
  const [removing, setRemoving] = useState<Product>()
  const cur = d.currency
  const rich = !!d.productIcons
  const editingProduct = editing && editing !== 'new' ? d.products.find((p) => p.id === editing.id) : undefined
  // A copy off sale (the relay leaves out its Stripe links), then its editor, like the relay dashboard.
  const duplicate = async (p: Product) => {
    try {
      const r = await post<{ made?: string }>('/api/payments/product/duplicate', { id: p.id })
      toast(t('servers.admin.pay.duplicated', { name: p.name }), 'success')
      await reload()
      setEditing(r.made ? { id: r.made } : undefined)
    } catch (e) {
      toast((e as Error).message, 'error')
    }
  }
  return (
    <>
      <div className="toolbar">
        <span className="hint grow">{t('servers.admin.pay.productsHint')}</span>
        <button className="btn sm" onClick={() => void act('/api/payments/defaults', {}, 'servers.admin.pay.saved')}>
          {t('servers.admin.pay.defaults')}
        </button>
        <button className="btn sm primary" onClick={() => setEditing('new')}>
          <Plus /> {t('servers.admin.pay.addProduct')}
        </button>
      </div>
      {d.products.length === 0 ? (
        <Empty icon={<ShoppingBag />} title={t('servers.admin.pay.noProducts')} />
      ) : (
        <div className="card admin-items">
          {d.products.map((p) => (
            <div key={p.id} className="setting admin-item pe-item">
              {rich && (
                <span
                  className={`pe-item-ic${p.highlight ? ' on' : ''}`}
                  title={isProductIcon(p.icon) ? t(`servers.admin.pay.icon.${p.icon}` as Key) : t('servers.admin.pay.iconAuto')}
                >
                  <ProductIcon icon={p.icon} kind={p.kind} />
                </span>
              )}
              <div className="text">
                <b>
                  {p.name}
                  {p.badge && <span className="rp-badge pe-badge">{p.badge}</span>}
                  {p.highlight && <span className="chip compact">{t('servers.admin.pay.highlighted')}</span>}
                  <span className="chip compact">{t(p.kind === 'pack' ? 'servers.admin.pay.pack' : 'servers.admin.pay.addon')}</span>
                  {p.on_sale ? (
                    <span className="chip compact success">{t('servers.admin.pay.onSale')}</span>
                  ) : (
                    <span className="chip compact">{t('servers.admin.pay.notOnSale')}</span>
                  )}
                  {p.stripe_product ? (
                    <span
                      className="chip compact success"
                      title={[p.stripe_product, p.stripe_price_monthly, p.stripe_price_yearly].filter(Boolean).join(' · ')}
                    >
                      {t('servers.admin.pay.inStripe')}
                    </span>
                  ) : (
                    <span className="chip compact warn">{t('servers.admin.pay.notInStripe')}</span>
                  )}
                </b>
                <span>
                  {p.id} · {gives(p.grants)}
                </span>
                <span className="hint">
                  {p.monthly ? t('servers.admin.pay.perMonth', { price: money(p.monthly, cur) }) : t('servers.admin.pay.noMonthly')} ·{' '}
                  {p.yearly ? t('servers.admin.pay.perYear', { price: money(p.yearly, cur) }) : t('servers.admin.pay.noYearly')} ·{' '}
                  {t('servers.admin.pay.activeCount', { count: p.active })}
                </span>
              </div>
              <Switch
                on={p.on_sale}
                label={t('servers.admin.pay.onSale')}
                onChange={(on) => void act('/api/payments/product/sale', { id: p.id, on }, 'servers.admin.pay.saved')}
              />
              {rich && (
                <button className="btn sm ghost icon" title={t('servers.admin.pay.duplicateHint')} aria-label={t('servers.admin.pay.duplicate')} onClick={() => void duplicate(p)}>
                  <Copy />
                </button>
              )}
              <MoreButton
                items={() => [
                  { label: t('servers.admin.pay.edit'), icon: <Pencil />, onSelect: () => setEditing({ id: p.id }) },
                  rich && { label: t('servers.admin.pay.duplicate'), icon: <Copy />, onSelect: () => void duplicate(p) },
                  d.stripe.ready && {
                    label: t('servers.admin.pay.createInStripe'),
                    icon: <CloudUpload />,
                    onSelect: () => void act('/api/payments/product/stripe', { id: p.id }, 'servers.admin.pay.saved')
                  },
                  '-',
                  { label: t('servers.admin.pay.remove'), icon: <Trash2 />, danger: true, onSelect: () => setRemoving(p) }
                ]}
              />
            </div>
          ))}
        </div>
      )}
      {(editing === 'new' || editingProduct) && (
        <ProductModal
          key={editingProduct?.id ?? 'new'}
          d={d}
          p={editingProduct}
          act={act}
          onDuplicate={(p) => {
            setEditing(undefined)
            void duplicate(p)
          }}
          onClose={() => setEditing(undefined)}
        />
      )}
      {removing && (
        <ConfirmModal
          title={t('servers.admin.pay.removeTitle', { name: removing.name })}
          body={t('servers.admin.pay.removeBody')}
          confirm={t('servers.admin.pay.remove')}
          onConfirm={() => void act('/api/payments/product/remove', { id: removing.id }, 'servers.admin.pay.saved')}
          onClose={() => setRemoving(undefined)}
        />
      )}
    </>
  )
}

/** One group of the product editor, under its own heading. */
function Group({ title, hint, children }: { title: string; hint?: string; children: ReactNode }) {
  return (
    <section className="pe-group">
      <h4>{title}</h4>
      {hint && <p className="hint">{hint}</p>}
      {children}
    </section>
  )
}

/** The product editor: every field the relay takes, grouped as on the relay dashboard, and the Store's card as it'll look. */
function ProductModal({ d, p, act, onDuplicate, onClose }: { d: Payments; p?: Product; act: Act; onDuplicate: (p: Product) => void; onClose: () => void }) {
  const s = (v?: string | number | null) => (v == null ? '' : String(v))
  // Relays from before session minutes and the card's look don't send productIcons (nor take those fields).
  const rich = !!d.productIcons
  const icons = (d.productIcons ?? []).filter(isProductIcon)
  const [f, setF] = useState({
    id: s(p?.id),
    name: s(p?.name),
    description: s(p?.description),
    kind: p?.kind ?? 'addon',
    monthly: p ? s(p.monthly / 100) : '',
    yearly: p ? s(p.yearly / 100) : '',
    order: s(p?.order ?? 0),
    on_sale: p?.on_sale ?? false,
    refundable: p?.refundable ?? true,
    badge: s(p?.badge),
    highlight: p?.highlight ?? false,
    icon: isProductIcon(p?.icon) ? (p?.icon as string) : '',
    stripe_product: s(p?.stripe_product),
    stripe_price_monthly: s(p?.stripe_price_monthly),
    stripe_price_yearly: s(p?.stripe_price_yearly)
  })
  const [g, setG] = useState<Grants>(p?.grants ?? {})
  const [busy, setBusy] = useState(false)
  const [previewYearly, setPreviewYearly] = useState(false)
  const isInf = (k: (typeof INF)[number]) => (g[k] ?? 0) >= UNLIMITED_COUNT
  const allInf = INF.every(isInf) && SWITCHES.every((k) => !!g[k])
  const setAllInf = (on: boolean) => {
    const next: Grants = { ...g }
    for (const k of INF) next[k] = on ? UNLIMITED_COUNT : 0
    for (const k of SWITCHES) next[k] = on
    setG(next)
  }
  const cur = d.currency.toUpperCase()
  const text = (k: keyof typeof f, label: string, placeholder?: string, hint?: string, maxLength?: number) => (
    <label className="field">
      <span>{label}</span>
      <input
        className="input"
        spellCheck={false}
        value={f[k] as string}
        placeholder={placeholder}
        maxLength={maxLength}
        disabled={k === 'id' && !!p}
        onChange={(e) => setF({ ...f, [k]: e.target.value })}
      />
      {hint && <em className="hint">{hint}</em>}
    </label>
  )
  const toggle = (on: boolean, label: string, onChange: (v: boolean) => void, hint?: string) => (
    <label className="row">
      <Switch on={on} label={label} onChange={onChange} />
      <span>
        {label}
        {hint && <span className="hint"> {hint}</span>}
      </span>
    </label>
  )
  const counts = COUNTS.filter((k) => rich || k !== 'session_minutes')
  const grants = (): Grants => {
    const out: Grants = {}
    for (const k of counts) out[k] = Math.max(0, Math.floor(Number(g[k]) || 0))
    for (const k of SWITCHES) out[k] = !!g[k]
    return out
  }
  const monthly = Number(f.monthly) || 0
  const yearly = Number(f.yearly) || 0
  // What the relay would refuse, said before sending (its own 400s name the field too).
  const problem = !f.name.trim()
    ? t('servers.admin.pay.err.name')
    : !p && !/^[a-z0-9][a-z0-9-]{1,39}$/.test(f.id.trim())
      ? t('servers.admin.pay.err.id')
      : f.badge.trim().length > MAX_BADGE
        ? t('servers.admin.pay.err.badge', { n: MAX_BADGE })
        : (Number(g.session_minutes) || 0) > MAX_SESSION_MINUTES
          ? t('servers.admin.pay.err.minutes', { n: MAX_SESSION_MINUTES })
          : monthly < 0 || yearly < 0
            ? t('servers.admin.pay.err.negative')
            : f.on_sale && !monthly && !yearly && !f.stripe_price_monthly.trim() && !f.stripe_price_yearly.trim()
              ? t('servers.admin.pay.err.onSale')
              : ''
  const save = async () => {
    setBusy(true)
    const ok = await act(
      '/api/payments/product',
      {
        id: p?.id ?? f.id.trim(),
        name: f.name.trim(),
        description: f.description.trim(),
        kind: f.kind,
        grants: grants(),
        monthly,
        yearly,
        order: Math.floor(Number(f.order) || 0),
        on_sale: f.on_sale,
        // Only relays with Resin Credit know it.
        ...(d.walletOn !== undefined ? { refundable: f.refundable } : {}),
        ...(rich ? { badge: f.badge.trim(), highlight: f.highlight, icon: f.icon } : {}),
        stripe_product: f.stripe_product.trim(),
        stripe_price_monthly: f.stripe_price_monthly.trim(),
        stripe_price_yearly: f.stripe_price_yearly.trim()
      },
      'servers.admin.pay.saved'
    )
    setBusy(false)
    if (ok) onClose()
  }
  // The Store page's own row, from what's typed so far.
  const previewPrice = Math.round((previewYearly ? yearly : monthly) * 100)
  const preview = (
    <aside className="pe-preview">
      <div className="row">
        <b className="grow">{t('servers.admin.pay.preview')}</b>
        <Segmented<'m' | 'y'>
          value={previewYearly ? 'y' : 'm'}
          onChange={(v) => setPreviewYearly(v === 'y')}
          options={[
            { value: 'm', label: t('servers.relayPlan.monthly') },
            { value: 'y', label: t('servers.relayPlan.yearly') }
          ]}
        />
      </div>
      <div className={`relay-plan rp-page${f.on_sale && previewPrice ? '' : ' pe-off'}`}>
        <StoreProductRow
          p={{ name: f.name.trim() || t('servers.admin.pay.untitled'), kind: f.kind, grants: grants(), badge: f.badge.trim(), highlight: f.highlight, icon: f.icon }}
          price={previewPrice}
          currency={d.currency}
          yearly={previewYearly}
          buttons={
            <>
              <button type="button" className="btn sm primary" tabIndex={-1}>
                <CreditCard /> {t('servers.relayPlan.buy')}
              </button>
              <button type="button" className="btn sm icon" tabIndex={-1} aria-label={t('servers.relayPlan.addToCart')}>
                <Plus />
              </button>
            </>
          }
        />
      </div>
      <p className="hint">
        {!previewPrice
          ? t(previewYearly ? 'servers.admin.pay.previewNoYearly' : 'servers.admin.pay.previewNoMonthly')
          : f.on_sale
            ? t('servers.admin.pay.previewOnSale')
            : t('servers.admin.pay.previewOffSale')}
      </p>
    </aside>
  )
  return (
    <Modal
      wide
      title={p ? t('servers.admin.pay.editTitle', { name: p.name }) : t('servers.admin.pay.addProduct')}
      onClose={onClose}
      footer={
        <>
          {problem && <span className="hint bad grow pe-problem">{problem}</span>}
          {p && rich && (
            <button className="btn" title={t('servers.admin.pay.duplicateHint')} onClick={() => onDuplicate(p)}>
              <Copy /> {t('servers.admin.pay.duplicate')}
            </button>
          )}
          <button className="btn" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={busy || !!problem} onClick={() => void save()}>
            <Check /> {t('servers.admin.set.save')}
          </button>
        </>
      }
    >
      <div className={rich ? 'pe' : undefined}>
        <div className="listing-fields">
          <Group title={t('servers.admin.pay.sec.basics')}>
            <div className="field-row">
              {text('id', t('servers.admin.pay.f.id'), 'addon-longer-sessions', p ? t('servers.admin.pay.f.idFixed') : t('servers.admin.pay.f.idHint'), 40)}
              <label className="field">
                <span>{t('servers.admin.pay.f.kind')}</span>
                <Segmented<'addon' | 'pack'>
                  value={f.kind}
                  onChange={(kind) => setF({ ...f, kind })}
                  options={[
                    { value: 'addon', label: t('servers.admin.pay.addon') },
                    { value: 'pack', label: t('servers.admin.pay.pack') }
                  ]}
                />
              </label>
            </div>
            {text('name', t('servers.admin.pay.f.name'), undefined, undefined, 80)}
            {text('description', t('servers.admin.pay.f.description'), undefined, undefined, 300)}
          </Group>
          <Group title={t('servers.admin.pay.sec.items')} hint={rich ? t('servers.admin.pay.itemsHint') : undefined}>
            {toggle(allInf, t('servers.admin.pay.infinite'), setAllInf, t('servers.admin.pay.infiniteHint'))}
            <div className="field-row">
              {counts.map((k) => {
                const inf = (k === 'servers' || k === 'names') && isInf(k)
                // Longer sessions mean nothing next to no session limit.
                const off = inf || (k === 'session_minutes' && !!g.no_session_limit)
                return (
                  <label key={k} className="field">
                    <span>{t(`servers.admin.pay.grant.${k}` as Key)}</span>
                    <input
                      className="input"
                      inputMode="numeric"
                      value={inf ? '' : s(g[k] ?? 0)}
                      placeholder={inf ? '∞' : undefined}
                      disabled={off}
                      onChange={(e) => setG({ ...g, [k]: e.target.value === '' ? 0 : Number(e.target.value) })}
                    />
                    {(k === 'servers' || k === 'names') && (
                      <span className="row" style={{ gap: 6, fontWeight: 'normal' }}>
                        <input type="checkbox" checked={inf} onChange={(e) => setG({ ...g, [k]: e.target.checked ? UNLIMITED_COUNT : 0 })} />
                        {t('servers.admin.pay.noLimit')}
                      </span>
                    )}
                    {k === 'session_minutes' && <em className="hint">{t('servers.admin.pay.f.minutesHint', { n: MAX_SESSION_MINUTES })}</em>}
                  </label>
                )
              })}
            </div>
            {SWITCHES.map((k) => (
              <div key={k}>{toggle(!!g[k], t(`servers.admin.pay.grant.${k}` as Key), (v) => setG({ ...g, [k]: v }))}</div>
            ))}
          </Group>
          <Group title={t('servers.admin.pay.sec.prices')} hint={t('servers.admin.pay.pricesHint', { currency: cur })}>
            <div className="field-row">
              {text('monthly', t('servers.admin.pay.f.monthly', { currency: cur }))}
              {text('yearly', t('servers.admin.pay.f.yearly', { currency: cur }))}
            </div>
          </Group>
          <Group title={t('servers.admin.pay.sec.sale')}>
            {toggle(f.on_sale, t('servers.admin.pay.onSale'), (on_sale) => setF({ ...f, on_sale }))}
            <div className="field-row">{text('order', t('servers.admin.pay.f.order'))}</div>
            {d.walletOn !== undefined &&
              toggle(f.refundable, t('servers.admin.wallet.refundable'), (refundable) => setF({ ...f, refundable }), t('servers.admin.wallet.refundableHint'))}
          </Group>
          {rich && (
            <Group title={t('servers.admin.pay.sec.display')}>
              {text('badge', t('servers.admin.pay.f.badge'), t('servers.admin.pay.f.badgePlaceholder'), t('servers.admin.pay.f.badgeHint', { n: MAX_BADGE }), MAX_BADGE)}
              {toggle(f.highlight, t('servers.admin.pay.f.highlight'), (highlight) => setF({ ...f, highlight }), t('servers.admin.pay.f.highlightHint'))}
              <div className="field">
                <span>{t('servers.admin.pay.f.icon')}</span>
                <div className="pe-icons" role="radiogroup" aria-label={t('servers.admin.pay.f.icon')}>
                  {['', ...icons].map((name) => {
                    const label = name ? t(`servers.admin.pay.icon.${name}` as Key) : t('servers.admin.pay.iconAuto')
                    return (
                      <button
                        key={name || 'auto'}
                        type="button"
                        role="radio"
                        aria-checked={f.icon === name}
                        className={`pe-icon${f.icon === name ? ' on' : ''}${name ? '' : ' auto'}`}
                        title={name ? label : t('servers.admin.pay.iconAutoHint')}
                        aria-label={label}
                        onClick={() => setF({ ...f, icon: name })}
                      >
                        <ProductIcon icon={name} kind={f.kind} />
                        {!name && <span>{label}</span>}
                      </button>
                    )
                  })}
                </div>
              </div>
            </Group>
          )}
          <Group title={t('servers.admin.pay.sec.stripe')} hint={t('servers.admin.pay.stripeHint')}>
            {text('stripe_product', t('servers.admin.pay.f.stripeProduct'), 'prod_…')}
            <div className="field-row">
              {text('stripe_price_monthly', t('servers.admin.pay.f.stripeMonthly'), 'price_…')}
              {text('stripe_price_yearly', t('servers.admin.pay.f.stripeYearly'), 'price_…')}
            </div>
          </Group>
        </div>
        {rich && preview}
      </div>
    </Modal>
  )
}

/** Ended, used-up or expired codes, folded under the main list (as on the relay dashboard). */
function EndedLog({ n, children }: { n: number; children: ReactNode }) {
  if (!n) return null
  return (
    <details className="pay-ended">
      <summary>{t('servers.admin.pay.endedLog', { n })}</summary>
      <div className="card admin-items">{children}</div>
    </details>
  )
}

function Codes({ d, act, reload }: { d: Payments; act: Act; reload: () => unknown }) {
  const { toast } = useStore()
  const [promo, setPromo] = useState(false)
  const [gift, setGift] = useState(false)
  const [made, setMade] = useState<string[]>()
  // Gift codes to send to a player (their Inbox, and email when linked).
  const [sending, setSending] = useState<string[]>()
  const cur = d.currency
  const name = (id: string) => d.products.find((p) => p.id === id)?.name ?? id
  const usablePromos = d.promos.filter((p) => p.usable)
  const endedPromos = d.promos.filter((p) => !p.usable)
  const usableGifts = d.gifts.filter((g) => g.usable)
  const endedGifts = d.gifts.filter((g) => !g.usable)
  const promoRow = (p: Promo) => (
    <div key={p.code} className="setting admin-item">
      <div className="text">
        <b>
          <code>{p.code}</code>
          <span className={`chip compact ${p.usable ? 'success' : 'danger'}`}>{t(p.usable ? 'servers.admin.pay.usable' : 'servers.admin.pay.ended')}</span>
        </b>
        <span>
          {p.percent_off
            ? t('servers.admin.pay.percentOff', { n: p.percent_off })
            : t('servers.admin.pay.amountOff', { amount: money(p.amount_off ?? 0, cur) })}{' '}
          ·{' '}
          {p.duration === 'repeating'
            ? t('servers.admin.pay.forMonths', { n: p.months ?? 0 })
            : t(p.duration === 'forever' ? 'servers.admin.pay.everyPayment' : 'servers.admin.pay.firstPayment')}{' '}
          · {p.products.length ? t('servers.admin.pay.onlyFor', { names: p.products.map(name).join(', ') }) : t('servers.admin.pay.forEverything')}
        </span>
        <span className="hint">
          {t('servers.admin.pay.used', { n: p.max_redemptions ? `${p.uses} / ${p.max_redemptions}` : p.uses })} ·{' '}
          {p.expires_at ? t('servers.admin.pay.endsAt', { when: when(p.expires_at) }) : t('servers.admin.pay.neverEnds')}
        </span>
        {!!p.below_minimum?.length && <span className="hint warn">{t('servers.admin.pay.belowMin', { names: p.below_minimum.join(', ') })}</span>}
      </div>
      {p.active && (
        <button className="btn sm" onClick={() => void act('/api/payments/promo/off', { code: p.code }, 'servers.admin.pay.saved')}>
          <Ban /> {t('servers.admin.pay.end')}
        </button>
      )}
    </div>
  )
  const giftRow = (g: GiftCode) => (
    <div key={g.code} className="setting admin-item">
      <div className="text">
        <b>
          <code>{g.code}</code>
          <span className={`chip compact ${g.usable ? 'success' : 'danger'}`}>{t(g.usable ? 'servers.admin.pay.usable' : 'servers.admin.pay.ended')}</span>
        </b>
        <span>
          {g.credit ? t('servers.admin.pay.giftCredit', { amount: money(g.credit, d.currency) }) : name(g.product)}
          {g.credit ? '' : ` · ${g.days ? t('servers.admin.pay.forDays', { n: g.days }) : t('servers.admin.pay.forGood')}`} ·{' '}
          {t('servers.admin.pay.redeemed', { n: g.max_uses ? `${g.redeemed.length} / ${g.max_uses}` : g.redeemed.length })}
          {g.expires_at ? ` · ${t('servers.admin.pay.endsAt', { when: when(g.expires_at) })}` : ''}
        </span>
        {(g.note || g.redeemed.length > 0) && <span className="hint">{[g.note, g.redeemed.map((u) => u.name).join(', ')].filter(Boolean).join(' · ')}</span>}
      </div>
      <div className="row">
        {g.usable && (
          <button className="btn sm" onClick={() => setSending([g.code])}>
            <Send /> {t('servers.admin.pay.sendCode')}
          </button>
        )}
        {g.active && (
          <button className="btn sm" onClick={() => void act('/api/payments/gift/off', { code: g.code }, 'servers.admin.pay.saved')}>
            <Ban /> {t('servers.admin.pay.end')}
          </button>
        )}
      </div>
    </div>
  )
  return (
    <>
      <div className="settings-section">
        <div className="toolbar">
          <h3 className="grow">{t('servers.admin.pay.promos')}</h3>
          <button
            className="btn sm primary"
            disabled={!d.stripe.ready}
            title={d.stripe.ready ? undefined : t('servers.admin.pay.stripeFirst')}
            onClick={() => setPromo(true)}
          >
            <Plus /> {t('servers.admin.pay.addPromo')}
          </button>
        </div>
        <p className="hint">{t('servers.admin.pay.promosHint')}</p>
        {usablePromos.length === 0 ? (
          <Empty icon={<Ticket />} title={t('servers.admin.pay.noPromos')} />
        ) : (
          <div className="card admin-items">{usablePromos.map(promoRow)}</div>
        )}
        <EndedLog n={endedPromos.length}>{endedPromos.map(promoRow)}</EndedLog>
      </div>

      <div className="settings-section">
        <div className="toolbar">
          <h3 className="grow">{t('servers.admin.pay.gifts')}</h3>
          <button className="btn sm primary" disabled={d.products.length === 0 && !d.walletOn} onClick={() => setGift(true)}>
            <Plus /> {t('servers.admin.pay.makeGifts')}
          </button>
        </div>
        <p className="hint">{t('servers.admin.pay.giftsHint')}</p>
        {made && (
          <div className="card">
            <div className="setting stack">
              <div className="row">
                <span className="hint grow">{t('servers.admin.pay.justMade')}</span>
                <button
                  className="btn sm"
                  onClick={() => {
                    void navigator.clipboard.writeText(made.join('\n'))
                    toast(t('servers.admin.pay.copied'), 'success')
                  }}
                >
                  {t('servers.admin.pay.copy')}
                </button>
                <button className="btn sm primary" onClick={() => setSending(made)}>
                  <Send /> {t('servers.admin.pay.sendToPlayer')}
                </button>
                <button className="btn sm ghost" aria-label={t('common.close')} onClick={() => setMade(undefined)}>
                  <X />
                </button>
              </div>
              <pre className="mono small">{made.join('\n')}</pre>
            </div>
          </div>
        )}
        {usableGifts.length === 0 ? (
          <Empty icon={<Gift />} title={t('servers.admin.pay.noGifts')} />
        ) : (
          <div className="card admin-items">{usableGifts.map(giftRow)}</div>
        )}
        <EndedLog n={endedGifts.length}>{endedGifts.map(giftRow)}</EndedLog>
      </div>

      {promo && (
        <PromoModal
          d={d}
          onClose={() => setPromo(false)}
          onAdd={async (body) => {
            try {
              const r = await post<Payments>('/api/payments/promo', body)
              toast(r.warning ?? t('servers.admin.pay.promoAdded', { code: String(body.code).toUpperCase() }), r.warning ? 'error' : 'success')
              await reload()
              return true
            } catch (e) {
              toast((e as Error).message, 'error')
              return false
            }
          }}
        />
      )}
      {sending && (
        <AskModal
          title={sending.length === 1 ? t('servers.admin.pay.sendTitle', { code: sending[0] }) : t('servers.admin.pay.sendTitleMany', { n: sending.length })}
          fields={[
            { key: 'player', label: t('servers.admin.pay.sendPlayer'), placeholder: 'Steve' },
            { key: 'note', label: t('servers.admin.pay.sendNote'), type: 'textarea' }
          ]}
          confirm={t('servers.admin.pay.sendToPlayer')}
          body={<p className="hint">{t('servers.admin.pay.sendHint')}</p>}
          onSubmit={(v) => act('/api/payments/gift/send', { codes: sending, player: v.player.trim(), note: v.note.trim() }, 'servers.admin.pay.sent', { name: v.player.trim() })}
          onClose={() => setSending(undefined)}
        />
      )}
      {gift && (
        <GiftModal
          d={d}
          onClose={() => setGift(false)}
          onMake={async (body, to) => {
            try {
              const r = await post<Payments>('/api/payments/gift', body)
              setMade(r.made ?? [])
              // Sent straight to a player, when the dialog names one (to their Inbox, like Send on a code).
              if (to && r.made?.length) {
                try {
                  await post('/api/payments/gift/send', { codes: r.made, player: to, note: String(body.note ?? '') })
                  toast(t('servers.admin.pay.sent', { name: to }), 'success')
                } catch (e) {
                  toast(t('servers.admin.pay.madeNotSent', { error: (e as Error).message }), 'error')
                }
              } else toast(t('servers.admin.pay.giftsMade', { n: r.made?.length ?? 0 }), 'success')
              await reload()
              return true
            } catch (e) {
              toast((e as Error).message, 'error')
              return false
            }
          }}
        />
      )}
    </>
  )
}

/** Products to pick for a code (none picked: everything). */
function PickProducts({ d, value, onChange }: { d: Payments; value: string[]; onChange: (v: string[]) => void }) {
  return (
    <div className="field">
      <span>{t('servers.admin.pay.f.onlyFor')}</span>
      <div className="row wrap">
        {d.products.map((p) => {
          const on = value.includes(p.id)
          return (
            <button
              key={p.id}
              type="button"
              className={`chip ${on ? 'success' : ''}`}
              onClick={() => onChange(on ? value.filter((x) => x !== p.id) : [...value, p.id])}
            >
              {on && <Check className="inline" />} {p.name}
            </button>
          )
        })}
      </div>
    </div>
  )
}

function FormModal({
  title,
  confirm,
  ok,
  onSubmit,
  onClose,
  children
}: {
  title: string
  confirm: string
  ok: boolean
  onSubmit: () => Promise<boolean>
  onClose: () => void
  children: ReactNode
}) {
  const [busy, setBusy] = useState(false)
  const go = async () => {
    setBusy(true)
    const done = await onSubmit()
    setBusy(false)
    if (done) onClose()
  }
  return (
    <Modal
      wide
      title={title}
      onClose={onClose}
      footer={
        <>
          <button className="btn" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={busy || !ok} onClick={() => void go()}>
            {confirm}
          </button>
        </>
      }
    >
      <div className="listing-fields">{children}</div>
    </Modal>
  )
}

function PromoModal({ d, onAdd, onClose }: { d: Payments; onAdd: (body: Record<string, unknown>) => Promise<boolean>; onClose: () => void }) {
  const [f, setF] = useState({
    code: '',
    kind: 'percent' as 'percent' | 'amount',
    value: '',
    duration: 'once' as Promo['duration'],
    months: '3',
    uses: '',
    days: ''
  })
  const [products, setProducts] = useState<string[]>([])
  const field = (k: 'code' | 'value' | 'months' | 'uses' | 'days', label: string, placeholder?: string) => (
    <label className="field">
      <span>{label}</span>
      <input
        className="input"
        spellCheck={false}
        inputMode={k === 'code' ? 'text' : 'decimal'}
        value={f[k]}
        placeholder={placeholder}
        onChange={(e) => setF({ ...f, [k]: e.target.value })}
      />
    </label>
  )
  const submit = () => {
    const body: Record<string, unknown> = { code: f.code.trim(), duration: f.duration, products }
    body[f.kind === 'percent' ? 'percent_off' : 'amount_off'] = Number(f.value)
    if (f.duration === 'repeating') body.months = Number(f.months) || 1
    if (Number(f.uses) > 0) body.max_redemptions = Number(f.uses)
    if (Number(f.days) > 0) body.expires_days = Number(f.days)
    return onAdd(body)
  }
  return (
    <FormModal
      title={t('servers.admin.pay.addPromo')}
      confirm={t('servers.admin.pay.addPromo')}
      ok={!!f.code.trim() && Number(f.value) > 0}
      onSubmit={submit}
      onClose={onClose}
    >
      <div className="field-row">
        {field('code', t('servers.admin.pay.f.code'), 'SUMMER25')}
        {field('value', t('servers.admin.pay.f.off'), '25')}
        <label className="field">
          <span>{t('servers.admin.pay.f.kind')}</span>
          <Segmented<'percent' | 'amount'>
            value={f.kind}
            onChange={(kind) => setF({ ...f, kind })}
            options={[
              { value: 'percent', label: '%' },
              { value: 'amount', label: d.currency.toUpperCase() }
            ]}
          />
        </label>
      </div>
      <label className="field">
        <span>{t('servers.admin.pay.f.appliesTo')}</span>
        <Segmented<Promo['duration']>
          value={f.duration}
          onChange={(duration) => setF({ ...f, duration })}
          options={[
            { value: 'once', label: t('servers.admin.pay.firstPayment') },
            { value: 'repeating', label: t('servers.admin.pay.someMonths') },
            { value: 'forever', label: t('servers.admin.pay.everyPayment') }
          ]}
        />
      </label>
      <div className="field-row">
        {f.duration === 'repeating' && field('months', t('servers.admin.pay.f.months'))}
        {field('uses', t('servers.admin.pay.f.uses'), t('servers.admin.pay.any'))}
        {field('days', t('servers.admin.pay.f.endsIn'), t('servers.admin.pay.never'))}
      </div>
      <PickProducts d={d} value={products} onChange={setProducts} />
      <span className="hint">{t('servers.admin.pay.promoHint')}</span>
    </FormModal>
  )
}

/** The gift form's "product" when the code gives Resin Credit. */
const CREDIT = '__credit'

function GiftModal({ d, onMake, onClose }: { d: Payments; onMake: (body: Record<string, unknown>, to: string) => Promise<boolean>; onClose: () => void }) {
  const [f, setF] = useState({ product: d.products[0]?.id ?? (d.walletOn ? CREDIT : ''), days: '30', code: '', count: '1', uses: '1', expires: '', note: '', credit: '100', to: '' })
  // Resin Credit instead of a product: the code adds that much to the balance.
  const credit = f.product === CREDIT
  const creditAmount = Math.round(Number(f.credit) * 100)
  // How long a code gives the product: a month (30 days), a year (365), for good (0) or any number of days.
  const [len, setLen] = useState<'30' | '365' | '0' | 'custom'>('30')
  const field = (k: Exclude<keyof typeof f, 'product'>, label: string, placeholder?: string) => (
    <label className="field">
      <span>{label}</span>
      <input className="input" spellCheck={false} value={f[k]} placeholder={placeholder} onChange={(e) => setF({ ...f, [k]: e.target.value })} />
    </label>
  )
  const submit = () => {
    const body: Record<string, unknown> = credit
      ? { credit: creditAmount, max_uses: Number(f.uses) || 0, note: f.note.trim() }
      : { product: f.product, days: len === 'custom' ? Math.max(1, Math.floor(Number(f.days) || 1)) : Number(len), max_uses: Number(f.uses) || 0, note: f.note.trim() }
    if (f.code.trim()) body.code = f.code.trim()
    else body.count = Math.max(1, Number(f.count) || 1)
    if (Number(f.expires) > 0) body.expires_days = Number(f.expires)
    return onMake(body, f.to.trim())
  }
  return (
    <FormModal
      title={t('servers.admin.pay.makeGifts')}
      confirm={t('servers.admin.pay.makeGifts')}
      ok={!!f.product && (!credit || (Number.isInteger(Number(f.credit)) && creditAmount >= 1000 && creditAmount <= 10_000_000))}
      onSubmit={submit}
      onClose={onClose}
    >
      <label className="field">
        <span>{t('servers.admin.pay.f.gives')}</span>
        <select className="input" value={f.product} onChange={(e) => setF({ ...f, product: e.target.value })}>
          {d.walletOn && <option value={CREDIT}>{t('servers.admin.pay.giftCreditOption')}</option>}
          {d.products.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
            </option>
          ))}
        </select>
      </label>
      {credit ? (
        <label className="field">
          <span>{t('servers.admin.pay.f.creditAmount')}</span>
          <input className="input" inputMode="numeric" value={f.credit} onChange={(e) => setF({ ...f, credit: e.target.value.replace(/[^\d]/g, '') })} />
          <span className="hint">{t('servers.admin.pay.giftCreditHint', { amount: money(creditAmount || 0, d.currency) })}</span>
        </label>
      ) : (
      <label className="field">
        <span>{t('servers.admin.pay.f.lasts')}</span>
        <Segmented<'30' | '365' | '0' | 'custom'>
          value={len}
          onChange={setLen}
          options={[
            { value: '30', label: t('servers.admin.pay.len.month') },
            { value: '365', label: t('servers.admin.pay.len.year') },
            { value: '0', label: t('servers.admin.pay.len.forever') },
            { value: 'custom', label: t('servers.admin.pay.len.custom') }
          ]}
        />
      </label>
      )}
      <div className="field-row">
        {!credit && len === 'custom' && field('days', t('servers.admin.pay.f.days'))}
        {field('uses', t('servers.admin.pay.f.perCode'))}
        {field('expires', t('servers.admin.pay.f.endsIn'), t('servers.admin.pay.never'))}
      </div>
      <div className="field-row">
        {field('code', t('servers.admin.pay.f.code'), t('servers.admin.pay.randomCodes'))}
        {!f.code.trim() && field('count', t('servers.admin.pay.f.count'))}
      </div>
      {field('note', t('servers.admin.pay.f.note'), t('servers.admin.pay.notePlaceholder'))}
      <label className="field">
        <span>{t('servers.admin.pay.f.sendTo')}</span>
        <input className="input" spellCheck={false} maxLength={36} value={f.to} placeholder={t('servers.admin.pay.f.sendToPlaceholder')} onChange={(e) => setF({ ...f, to: e.target.value })} />
        <span className="hint">{t('servers.admin.pay.f.sendToHint')}</span>
      </label>
    </FormModal>
  )
}

function Purchases({ d, act, onRefund }: { d: Payments; act: Act; onRefund: (s: Sub) => void }) {
  const [giving, setGiving] = useState(false)
  const [ending, setEnding] = useState<Sub>()
  const [noting, setNoting] = useState<Sub>()
  const [q, setQ] = useState('')
  const cur = d.currency
  const name = (id: string) => d.products.find((p) => p.id === id)?.name ?? id
  const subs = d.subs.filter((s) => !q.trim() || `${s.name} ${s.uuid} ${s.id} ${name(s.product)}`.toLowerCase().includes(q.trim().toLowerCase()))
  const how = (s: Sub) =>
    s.source === 'admin'
      ? t('servers.admin.pay.givenByAdmin')
      : `${t(s.yearly ? 'servers.admin.pay.yearly' : 'servers.admin.pay.monthly')} · ${money(s.amount, cur)}${s.method ? ` · ${s.method}` : ''}${s.source === 'stripe' ? ` · ${t('servers.admin.pay.renews')}` : ''}`
  return (
    <>
      <div className="toolbar">
        <div className="search">
          <Search />
          <input className="input" placeholder={t('servers.admin.pay.searchPurchases')} value={q} onChange={(e) => setQ(e.target.value)} />
        </div>
        <span className="grow" />
        {d.stripe.ready && (
          <button className="btn sm" onClick={() => void act('/api/payments/sync', {}, 'servers.admin.pay.synced')}>
            <RefreshCw /> {t('servers.admin.pay.syncAll')}
          </button>
        )}
        <button className="btn sm primary" disabled={d.products.length === 0} onClick={() => setGiving(true)}>
          <Gift /> {t('servers.admin.pay.giveFree')}
        </button>
      </div>
      {subs.length === 0 ? (
        <Empty icon={<ShoppingBag />} title={t('servers.admin.pay.noPurchases')} />
      ) : (
        <div className="card admin-items">
          {subs.slice(0, 300).map((s) => (
            <div key={s.id} className="setting admin-item">
              <div className="text">
                <b>
                  {s.name || shortId(s.uuid)} · {name(s.product)}
                  <span className={`chip compact ${s.active ? 'success' : 'danger'}`}>{s.status}</span>
                  {s.cancel_at_period_end && <span className="chip compact warn">{t('servers.admin.pay.endsAtEnd')}</span>}
                  {(s.refunded ?? 0) > 0 && <span className="chip compact">{t('servers.admin.wallet.refundedChip', { amount: money(s.refunded!, cur) })}</span>}
                </b>
                <span>{how(s)}</span>
                <span className="hint">
                  {s.period_end ? t('servers.admin.pay.periodEnds', { when: when(s.period_end) }) : t('servers.admin.pay.neverEnds')} · {s.id}
                  {s.credit ? ` · ${t('servers.admin.pay.creditTaken', { amount: money(s.credit, cur) })}` : ''}
                </span>
                {s.moved_from && <span className="hint">{t('servers.admin.pay.movedFrom', { name: s.moved_from, when: when(s.moved_at ?? 0) })}</span>}
                {s.note && <span className="admin-note">“{s.note}”</span>}
              </div>
              <MoreButton
                items={(): ContextEntry[] => [
                  { label: t(s.note ? 'servers.admin.pay.editNote' : 'servers.admin.pay.addNote'), icon: <Pencil />, onSelect: () => setNoting(s) },
                  // Resin Credit on: end it now and give credit back (never money), in Resin Credit with its account open.
                  ...(d.walletOn && s.uuid && !s.refunded && !s.status.startsWith('pending') && s.status !== 'incomplete'
                    ? ([{ label: t('servers.admin.wallet.refund'), icon: <Wallet />, onSelect: () => onRefund(s) }] as ContextEntry[])
                    : []),
                  '-',
                  ...(s.source === 'admin'
                    ? ([{ label: t('servers.admin.pay.takeAway'), icon: <Trash2 />, danger: true, onSelect: () => setEnding(s) }] as ContextEntry[])
                    : ([
                        s.source === 'stripe' && {
                          label: t('servers.admin.pay.sync'),
                          icon: <RefreshCw />,
                          onSelect: () => void act('/api/payments/sync', { id: s.id }, 'servers.admin.pay.synced')
                        },
                        s.source === 'stripe' &&
                          s.active &&
                          !s.cancel_at_period_end && {
                            label: t('servers.admin.pay.cancelAtEnd'),
                            icon: <X />,
                            onSelect: () => void act('/api/payments/end', { id: s.id, at_period_end: true }, 'servers.admin.pay.saved')
                          },
                        s.status !== 'canceled' && '-',
                        s.status !== 'canceled' && { label: t('servers.admin.pay.endNow'), icon: <Ban />, danger: true, onSelect: () => setEnding(s) }
                      ] as ContextEntry[]))
                ]}
              />
            </div>
          ))}
        </div>
      )}
      {giving && <GiveModal d={d} act={act} onClose={() => setGiving(false)} />}
      {noting && <NoteModal sub={noting} who={noting.name || shortId(noting.uuid)} act={act} onClose={() => setNoting(undefined)} />}
      {ending && (
        <ConfirmModal
          title={t('servers.admin.pay.endTitle', { name: `${ending.name || shortId(ending.uuid)} · ${name(ending.product)}` })}
          body={t(ending.source === 'stripe' ? 'servers.admin.pay.endStripeBody' : 'servers.admin.pay.endBody')}
          confirm={t('servers.admin.pay.endNow')}
          onConfirm={() => void act('/api/payments/end', { id: ending.id }, 'servers.admin.pay.saved')}
          onClose={() => setEnding(undefined)}
        />
      )}
    </>
  )
}

/** The admin's note to the player on one purchase or grant; empty clears it. */
function NoteModal({ sub, who, act, onClose }: { sub: Sub; who: string; act: Act; onClose: () => void }) {
  const [note, setNote] = useState(sub.note ?? '')
  return (
    <FormModal
      title={t('servers.admin.pay.noteTitle', { name: who })}
      confirm={t('common.save')}
      ok={note.trim() !== (sub.note ?? '')}
      onSubmit={() => act('/api/payments/note', { id: sub.id, note: note.trim() }, 'servers.admin.pay.saved')}
      onClose={onClose}
    >
      <label className="field">
        <span>{t('servers.admin.pay.f.note')}</span>
        <input className="input" autoFocus maxLength={200} value={note} onChange={(e) => setNote(e.target.value)} />
        <span className="hint">{t('servers.admin.pay.notePlayerHint')}</span>
      </label>
    </FormModal>
  )
}

function GiveModal({ d, act, onClose }: { d: Payments; act: Act; onClose: () => void }) {
  const [who, setWho] = useState('')
  const [product, setProduct] = useState(d.products[0]?.id ?? '')
  const [days, setDays] = useState('')
  const [note, setNote] = useState('')
  return (
    <FormModal
      title={t('servers.admin.pay.giveFree')}
      confirm={t('servers.admin.pay.give')}
      ok={!!who.trim() && !!product}
      onSubmit={() =>
        act(
          '/api/payments/grant',
          { ...account(who), product, ...(Number(days) > 0 ? { days: Number(days) } : {}), ...(note.trim() ? { note: note.trim() } : {}) },
          'servers.admin.pay.given'
        )
      }
      onClose={onClose}
    >
      <label className="field">
        <span>{t('servers.admin.pay.f.who')}</span>
        <input
          className="input"
          autoFocus
          spellCheck={false}
          value={who}
          placeholder={t('servers.admin.pay.whoPlaceholder')}
          onChange={(e) => setWho(e.target.value)}
        />
      </label>
      <div className="field-row">
        <label className="field">
          <span>{t('servers.admin.pay.f.gives')}</span>
          <select className="input" value={product} onChange={(e) => setProduct(e.target.value)}>
            {d.products.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span>{t('servers.admin.pay.f.days')}</span>
          <input
            className="input"
            inputMode="numeric"
            value={days}
            placeholder={t('servers.admin.pay.untilRemoved')}
            onChange={(e) => setDays(e.target.value)}
          />
        </label>
      </div>
      <label className="field">
        <span>{t('servers.admin.pay.f.note')}</span>
        <input className="input" maxLength={200} value={note} onChange={(e) => setNote(e.target.value)} />
        <span className="hint">{t('servers.admin.pay.notePlayerHint')}</span>
      </label>
    </FormModal>
  )
}

function TrueMoney({ d, act }: { d: Payments; act: Act }) {
  const tm = d.truemoney ?? { on: false, phone: '' }
  const [phone, setPhone] = useState('')
  const cur = d.currency
  const result = (r: string) => (r === 'ok' ? t('servers.admin.pay.angpaoPaid') : r === 'short' ? t('servers.admin.pay.angpaoShort') : r)
  return (
    <>
      <div className="settings-section">
        <h3>{t('servers.admin.pay.view.truemoney')}</h3>
        <div className="card">
          <SwitchRow
            title={t('servers.admin.pay.tmSwitch')}
            hint={tm.on ? t('servers.admin.pay.tmOn', { phone: tm.phone }) : t('servers.admin.pay.tmOff')}
            on={tm.on}
            onChange={(on) => void act('/api/payments/truemoney', phone.trim() ? { on, phone } : { on }, 'servers.admin.pay.saved')}
          />
          <div className="setting">
            <div className="text">
              <b>{t('servers.admin.pay.tmNumber')}</b>
              <span>{tm.phone ? t('servers.admin.pay.tmEnding', { phone: tm.phone }) : t('servers.admin.pay.tmNoNumber')}</span>
            </div>
            <input
              className="input"
              style={{ width: '10rem' }}
              inputMode="numeric"
              value={phone}
              placeholder="0812345678"
              onChange={(e) => setPhone(e.target.value)}
            />
            <button
              className="btn sm"
              disabled={!phone.trim()}
              onClick={() => void act('/api/payments/truemoney', { phone }, 'servers.admin.pay.saved').then((ok) => ok && setPhone(''))}
            >
              {t('servers.admin.set.save')}
            </button>
          </div>
          {!tm.fromFile && (
            <div className="setting">
              <span className="hint grow">{t('servers.admin.pay.tmOverride')}</span>
              <button className="btn sm" onClick={() => void act('/api/payments/truemoney', { reset: true }, 'servers.admin.pay.saved')}>
                {t('servers.admin.set.reset')}
              </button>
            </div>
          )}
          <div className="setting">
            <span className="hint">{t('servers.admin.pay.tmWarning')}</span>
          </div>
        </div>
      </div>
      <div className="settings-section">
        <h3>{t('servers.admin.pay.envelopes')}</h3>
        {d.angpao.length === 0 ? (
          <Empty icon={<Wallet />} title={t('servers.admin.pay.noEnvelopes')} />
        ) : (
          <div className="card admin-items">
            {d.angpao.slice(0, 200).map((r, i) => (
              <div key={`${r.at}-${i}`} className="setting admin-item">
                <div className="text">
                  <b>
                    {r.name || shortId(r.uuid)}
                    <span className={`chip compact ${r.result === 'ok' ? 'success' : r.result === 'short' ? 'warn' : 'danger'}`}>{result(r.result)}</span>
                  </b>
                  <span>
                    {t('servers.admin.pay.angpaoLine', { gave: r.amount ? money(r.amount, cur) : '—', price: money(r.price, cur) })}
                    {r.overpaid ? ` · ${t('servers.admin.pay.overpaid', { amount: money(r.overpaid, cur) })}` : ''}
                  </span>
                  <span className="hint">
                    {when(r.at)} · <code>{r.voucher}</code>
                    {r.items.length ? ` · ${r.items.map((id) => d.products.find((p) => p.id === id)?.name ?? id).join(', ')}` : ''}
                  </span>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </>
  )
}
