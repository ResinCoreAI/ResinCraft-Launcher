// Generates src/renderer/src/i18n/en/engine.ts from the tr!/err_tr!/bail_tr! calls in src-tauri/src, so the
// engine's English and the UI's English source never drift apart.
//   npx tsx tools/engine-strings.ts          write the file
//   npx tsx tools/engine-strings.ts --check  fail if it's out of date (or a key has two different texts)
import { readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs'
import { join, relative, resolve } from 'node:path'

const root = resolve(__dirname, '..')
const srcDir = join(root, 'src-tauri', 'src')
const outFile = join(root, 'src', 'renderer', 'src', 'i18n', 'en', 'engine.ts')

function rustFiles(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const p = join(dir, name)
    return statSync(p).isDirectory() ? rustFiles(p) : p.endsWith('.rs') ? [p] : []
  })
}

/** Reads a Rust string literal starting at `i` (the opening quote); returns its value and the index after it. */
function readString(src: string, i: number): [string, number] {
  let out = ''
  i++
  while (i < src.length) {
    const c = src[i]
    if (c === '"') return [out, i + 1]
    if (c === '\\') {
      const n = src[i + 1]
      if (n === '\n' || n === '\r') {
        // Line continuation: skip the newline and the next line's leading whitespace.
        i += 2
        while (/\s/.test(src[i])) i++
        continue
      }
      out += n === 'n' ? '\n' : n === 't' ? '\t' : n === '0' ? '\0' : n
      i += 2
      continue
    }
    out += c
    i++
  }
  throw new Error('Unterminated string')
}

const strings = new Map<string, { text: string; where: string }>()
const problems: string[] = []
const call = /\b(?:tr|err_tr|bail_tr)!\s*\(\s*"/g

for (const file of rustFiles(srcDir)) {
  const src = readFileSync(file, 'utf8')
  for (const m of src.matchAll(call)) {
    const before = src.slice(0, m.index)
    if (before.slice(before.lastIndexOf('\n') + 1).trimStart().startsWith('//')) continue // examples in comments
    const where = `${relative(root, file)}:${before.split('\n').length}`
    let [key, i] = readString(src, m.index! + m[0].length - 1)
    while (/\s/.test(src[i])) i++
    if (src[i] !== ',') continue // the macro definitions themselves
    i++
    while (/\s/.test(src[i])) i++
    if (src[i] !== '"') {
      problems.push(`${where}: the English text must be a string literal`)
      continue
    }
    // `{{` and `}}` are literal braces in Rust format strings.
    const text = readString(src, i)[0].replace(/\{\{/g, '{').replace(/\}\}/g, '}')
    if (key.startsWith('test.')) continue
    if (/\{\s*(:[^}]*)?\}|\{\d+\}/.test(text)) problems.push(`${where}: use named placeholders like {name}, not {} or {0}`)
    const seen = strings.get(key)
    if (seen && seen.text !== text) problems.push(`${where}: "${key}" already means "${seen.text}" (${seen.where})`)
    else if (!seen) strings.set(key, { text, where })
  }
}

const quote = (s: string) => (s.includes("'") ? JSON.stringify(s) : `'${JSON.stringify(s).slice(1, -1).replace(/\\"/g, '"')}'`)
const body = [...strings.keys()]
  .sort()
  .map((k) => `  ${quote(`engine.${k}`)}: ${quote(strings.get(k)!.text)}`)
  .join(',\n')
const output = `// Generated from the tr!() calls in src-tauri/src by \`npx tsx tools/engine-strings.ts\`. Don't edit by hand:
// change the English in the Rust source and run the script again.
export default {
${body}
}
`

if (problems.length) {
  console.error(problems.join('\n'))
  process.exit(1)
}
const current = (() => {
  try {
    return readFileSync(outFile, 'utf8')
  } catch {
    return ''
  }
})()
if (process.argv.includes('--check')) {
  if (current !== output) {
    console.error('src/renderer/src/i18n/en/engine.ts is out of date: run npx tsx tools/engine-strings.ts')
    process.exit(1)
  }
  console.log(`engine strings: ok (${strings.size})`)
} else {
  writeFileSync(outFile, output)
  console.log(`wrote ${strings.size} engine strings to ${relative(root, outFile)}`)
}
