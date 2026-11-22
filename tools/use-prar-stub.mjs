// Builds ResinCraft from source without access to PuirkleRAR (a private repository): points the engine at the
// stand-in crate in src-tauri/prar-stub and updates Cargo.lock to match. Backups are then zip files only.
// Run once before building: node tools/use-prar-stub.mjs   (CI does this for pull requests from forks.)
import { readFileSync, writeFileSync } from 'node:fs'
import { execFileSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'

const manifest = fileURLToPath(new URL('../src-tauri/Cargo.toml', import.meta.url))
const text = readFileSync(manifest, 'utf8')
const dep = /^puirklerar = \{[^\r\n]*\}/m
const stub = 'puirklerar = { path = "prar-stub" }'
const found = text.match(dep)
if (!found) throw new Error('No puirklerar dependency line in src-tauri/Cargo.toml')
if (found[0] === stub) {
  console.log('Already using the PuirkleRAR stand-in.')
} else {
  writeFileSync(manifest, text.replace(dep, stub))
  console.log('Using the PuirkleRAR stand-in (zip backups only).')
}
// Resolves again without fetching PuirkleRAR, so `cargo … --locked` works afterwards.
execFileSync('cargo', ['metadata', '--format-version', '1', '--manifest-path', manifest], { stdio: ['ignore', 'ignore', 'inherit'] })
