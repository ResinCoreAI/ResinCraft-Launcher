// Checks every translation against English: same keys, same {placeholders} and <tags>, and plural forms for each
// category the language uses. Run: npx tsx tools/check-i18n.ts [code…]
import en from '../src/renderer/src/i18n/en'
import { LANGUAGES, type Msg } from '../src/renderer/src/i18n'

const tokens = (s: string) => ({
  params: [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort(),
  tags: [...s.matchAll(/<(\/?\w+)>/g)].map((m) => m[1]).sort()
})
const forms = (m: Msg): string[] => (typeof m === 'string' ? [m] : Object.values(m).filter((v): v is string => typeof v === 'string'))

/** Key prefixes that fall back to English when a translation lacks them (see NewKey in i18n/index.tsx). */
const FALLBACK = ['servers.']

async function check(code: string): Promise<number> {
  const dict = (await import(`../src/renderer/src/i18n/locales/${code}.ts`)).default as Record<string, Msg>
  // Plural categories that real counts (0–10000) reach; e.g. French "many" only applies to millions.
  const rules = new Intl.PluralRules(code)
  const categories = [...new Set(Array.from({ length: 10001 }, (_, n) => rules.select(n)))]
  const problems: string[] = []
  let untranslated = 0
  let fallback = 0
  for (const [key, source] of Object.entries(en) as [string, Msg][]) {
    const msg = dict[key]
    if (msg == null) {
      // New areas (Dict's NewKey) may be missing: they show in English until translated.
      if (FALLBACK.some((p) => key.startsWith(p))) fallback++
      else problems.push(`${key}: missing`)
      continue
    }
    if (JSON.stringify(msg) === JSON.stringify(source)) untranslated++
    const want = new Set(forms(source).flatMap((s) => tokens(s).params))
    const wantTags = JSON.stringify(tokens(forms(source)[0]).tags)
    for (const text of forms(msg)) {
      const got = tokens(text)
      for (const p of got.params) if (!want.has(p)) problems.push(`${key}: unknown placeholder {${p}}`)
      if (JSON.stringify(got.tags) !== wantTags) problems.push(`${key}: tags differ from English`)
    }
    // A plural message must keep {count} somewhere and cover the language's categories (or at least "other").
    if (typeof source !== 'string') {
      if (typeof msg === 'string') problems.push(`${key}: should be a plural object`)
      else {
        const missing = categories.filter((c) => c !== 'zero' && !(c in msg) && c !== 'other')
        if (!('other' in msg)) problems.push(`${key}: plural needs "other"`)
        if (missing.length) problems.push(`${key}: plural lacks ${missing.join(', ')}`)
      }
    } else if (typeof msg !== 'string') problems.push(`${key}: should be a plain string`)
    for (const p of want) if (!forms(msg).some((s) => s.includes(`{${p}}`)) && p !== 'count') problems.push(`${key}: drops {${p}}`)
  }
  for (const key of Object.keys(dict)) if (!(key in en)) problems.push(`${key}: not an English key`)
  const total = Object.keys(en).length
  console.log(`${code}: ${problems.length ? `${problems.length} problem(s)` : 'ok'}, ${untranslated}/${total} identical to English${fallback ? `, ${fallback} new (shown in English)` : ''}`)
  for (const p of problems) console.log(`  ${p}`)
  return problems.length
}

const codes = process.argv.slice(2).length ? process.argv.slice(2) : LANGUAGES.map((l) => l.code).filter((c) => c !== 'en')
void (async () => {
  let bad = 0
  for (const c of codes) bad += await check(c)
  process.exit(bad ? 1 : 0)
})()
