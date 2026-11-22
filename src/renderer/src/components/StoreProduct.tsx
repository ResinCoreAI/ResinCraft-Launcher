// A Store product as players see it: its icon, badge and what it gives. Shared by the Store (RelayPlan.tsx) and the
// live preview in Admin → Payments' product editor, so the preview is the Store's own card.
import type { ReactNode } from 'react'
import { Clock, Database, Gift, Globe, Infinity as InfinityIcon, Package, Puzzle, Server, Shield, Star, Tag, Zap, type LucideIcon } from 'lucide-react'
import { PRODUCT_ICONS, type PlanGrants, type ProductIcon as IconName } from '@shared/types'
import { t } from '../i18n'
import { UNLIMITED_COUNT } from '../relayCount'
import { money } from '../storeStatus'

/** The relay's product icons (PRODUCT_ICONS), drawn with lucide. */
export const ICONS: Record<IconName, LucideIcon> = {
  clock: Clock,
  database: Database,
  server: Server,
  tag: Tag,
  infinity: InfinityIcon,
  globe: Globe,
  star: Star,
  gift: Gift,
  shield: Shield,
  bolt: Zap
}

export const isProductIcon = (name?: string): name is IconName => !!name && (PRODUCT_ICONS as readonly string[]).includes(name)

/** A product's icon: the admin's pick, or the launcher's own for an add-on or a pack. */
export function ProductIcon({ icon, kind }: { icon?: string; kind: string }) {
  const Icon = isProductIcon(icon) ? ICONS[icon] : kind === 'pack' ? Package : Puzzle
  return <Icon />
}

/** What a product gives, as the Store words it. */
export function givesParts(g: Partial<PlanGrants>): string[] {
  const parts: string[] = []
  const servers = g.servers ?? 0
  const names = g.names ?? 0
  if (servers >= UNLIMITED_COUNT) parts.push(t('servers.relayPlan.giveServersInf'))
  else if (servers) parts.push(t('servers.relayPlan.giveServers', { count: servers }))
  if (names >= UNLIMITED_COUNT) parts.push(t('servers.relayPlan.giveNamesInf'))
  else if (names) parts.push(t('servers.relayPlan.giveNames', { count: names }))
  if (g.traffic_gb) parts.push(t('servers.relayPlan.giveTraffic', { amount: `${g.traffic_gb} GB` }))
  // Longer sessions mean nothing next to no session limit.
  if (g.session_minutes && !g.no_session_limit) parts.push(t('servers.relayPlan.giveSessionMinutes', { minutes: g.session_minutes }))
  if (g.unlimited_traffic) parts.push(t('servers.relayPlan.trafficUnlimited'))
  if (g.no_session_limit) parts.push(t('servers.relayPlan.sessionUnlimited'))
  if (g.whitelist_page) parts.push(t('servers.relayPlan.whitelistOn'))
  return parts
}

/** What the Store page shows of one product: one row, so products can be compared down the page. */
export interface RowProduct {
  name: string
  kind: string
  grants: Partial<PlanGrants>
  badge?: string
  highlight?: boolean
  icon?: string
  owned?: boolean
}

/** The Store page's product row; `notes` go under what it gives, `buttons` at its end. */
export function StoreProductRow({
  p,
  price,
  currency,
  yearly,
  notes,
  buttons
}: {
  p: RowProduct
  price: number
  currency: string
  yearly: boolean
  notes?: ReactNode
  buttons?: ReactNode
}) {
  return (
    <div className={`rp-row${p.owned ? ' owned' : ''}${p.highlight ? ' highlight' : ''}`}>
      <span className="rp-row-ic">
        <ProductIcon icon={p.icon} kind={p.kind} />
      </span>
      <div className="rp-row-text">
        <b>
          {p.name}
          {p.badge && <span className="rp-badge">{p.badge}</span>}
        </b>
        <span className="hint">{givesParts(p.grants).join(' · ')}</span>
        {notes}
      </div>
      <div className="rp-row-price">
        {money(price, currency)}
        <span className="hint"> / {yearly ? t('servers.relayPlan.year') : t('servers.relayPlan.month')}</span>
      </div>
      {buttons && <div className="rp-row-buttons">{buttons}</div>}
    </div>
  )
}
