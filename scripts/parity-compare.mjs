// Compares ts.json and rust.json from the parity scripts: shapes (keys + types) everywhere, and exact
// values for local, deterministic answers.   node scripts/parity-compare.mjs ts.json rust.json
import { readFileSync } from 'node:fs'
const [a, b] = process.argv.slice(2).map((f) => JSON.parse(readFileSync(f, 'utf8')))
const EXACT = ['settings:get', 'instances:list', 'accounts:list', 'content:exportPreview', 'worlds:list', 'servers:list', 'screenshots:list', 'notes:get', 'java:list', 'content:list']
const kind = (v) => (v === null ? 'null' : Array.isArray(v) ? 'array' : typeof v)
function diff(x, y, path, out) {
  if (kind(x) !== kind(y)) return out.push(`${path}: ${kind(x)} (ts) vs ${kind(y)} (rust)`)
  if (kind(x) === 'array') {
    if (x.length && y.length) diff(x[0], y[0], `${path}[0]`, out)
    else if (x.length !== y.length) out.push(`${path}: ${x.length} items (ts) vs ${y.length} (rust)`)
  } else if (kind(x) === 'object') {
    for (const k of new Set([...Object.keys(x), ...Object.keys(y)])) {
      if (!(k in y)) out.push(`${path}.${k}: missing in rust`)
      else if (!(k in x)) out.push(`${path}.${k}: extra in rust`)
      else diff(x[k], y[k], `${path}.${k}`, out)
    }
  }
}
const canon = (v) => JSON.stringify(v, (_k, val) => (val && typeof val === 'object' && !Array.isArray(val) ? Object.fromEntries(Object.entries(val).sort()) : val))
let bad = 0
for (const name of Object.keys(a)) {
  const out = []
  diff(a[name], b[name], name, out)
  let note = out.length ? out.join('\n    ') : 'shape OK'
  if (!out.length && EXACT.includes(name)) note += canon(a[name]) === canon(b[name]) ? ', values identical' : ', VALUES DIFFER'
  if (out.length || note.includes('DIFFER')) bad++
  console.log(`${out.length || note.includes('DIFFER') ? 'DIFF' : 'OK  '} ${name}: ${note}`)
}
console.log(bad ? `${bad} channel(s) differ` : 'ALL CHANNELS MATCH')
