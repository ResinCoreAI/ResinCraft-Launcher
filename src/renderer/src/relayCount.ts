// Servers or names from this count up mean no limit (∞): the relay's UNLIMITED_COUNT, like an account's "unlimited".
export const UNLIMITED_COUNT = 1_000_000

/** A count of servers or names as players read it: ∞ when it has no limit. */
export const countText = (n: number): string => (n >= UNLIMITED_COUNT ? '∞' : String(n))
