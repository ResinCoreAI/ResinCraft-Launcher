// The relay's limits (LimitsConfig): which are whole numbers, decimals or a list, and their minimum values. Labels
// are servers.admin.relay.limit.<key>.
export type LimitKind = 'int' | 'float' | 'list'

export const LIMITS: { key: string; kind: LimitKind; min?: number }[] = [
  { key: 'tunnels_per_account', kind: 'int', min: 1 },
  { key: 'names_per_account', kind: 'int', min: 1 },
  { key: 'connections_per_tunnel', kind: 'int', min: 1 },
  { key: 'connections_per_ip', kind: 'int', min: 0 },
  { key: 'agents_per_ip', kind: 'int', min: 0 },
  { key: 'monthly_gb_per_account', kind: 'float', min: 0 },
  { key: 'handshake_timeout_secs', kind: 'int', min: 1 },
  { key: 'idle_timeout_secs', kind: 'int', min: 10 },
  { key: 'logins_per_ip_per_hour', kind: 'int', min: 1 },
  { key: 'name_expiry_days', kind: 'int', min: 1 },
  { key: 'session_days', kind: 'int', min: 1 },
  { key: 'max_pending_handshakes', kind: 'int', min: 1 },
  { key: 'session_minutes', kind: 'float', min: 0 },
  { key: 'session_warn_minutes', kind: 'list' },
  { key: 'session_continue_minutes', kind: 'float', min: 0 },
  { key: 'topup_gb', kind: 'float', min: 0 },
  { key: 'topups_per_month', kind: 'int', min: 0 },
  { key: 'topup_low_gb', kind: 'float', min: 0 },
  { key: 'max_domains', kind: 'int', min: 0 },
  { key: 'join_requests_per_ip', kind: 'int', min: 0 },
  { key: 'join_requests_per_server', kind: 'int', min: 0 }
]

export const kindOf = (key: string): LimitKind => LIMITS.find((l) => l.key === key)?.kind ?? 'float'

/** A limit's value as text for a field ("15, 5, 1" for lists). */
export function show(v: unknown): string {
  if (v == null) return ''
  if (Array.isArray(v)) return v.join(', ')
  return String(v)
}

/** A field's text back to the relay's value: null when empty (the relay's own value), "unlimited", a number or a list. */
export function parse(key: string, text: string): unknown {
  const s = text.trim()
  if (!s) return null
  if (s.toLowerCase() === 'unlimited') return 'unlimited'
  const kind = kindOf(key)
  if (kind === 'list') return s.split(/[,\s]+/).filter(Boolean).map(Number)
  const n = Number(s)
  return kind === 'int' ? Math.round(n) : n
}
