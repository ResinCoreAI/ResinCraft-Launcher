'use client'

// Release data straight from GitHub, so the site never needs editing for a new version.
// One request per visit (cached in sessionStorage for 10 minutes); on any failure the
// components keep their static fallbacks, and every Download link still opens /releases/latest.

import { useEffect, useState } from 'react'
import { REPO } from './site'

const API = `https://api.github.com/repos/${REPO}/releases?per_page=20`
const CACHE_KEY = 'resincraft.releases'
const CACHE_MS = 10 * 60 * 1000

let pending

function readCache() {
  try {
    const hit = JSON.parse(sessionStorage.getItem(CACHE_KEY) || 'null')
    if (hit && Date.now() - hit.at < CACHE_MS) return hit.releases
  } catch {
    // Storage blocked or corrupt: just fetch again.
  }
  return null
}

function writeCache(releases) {
  try {
    sessionStorage.setItem(CACHE_KEY, JSON.stringify({ at: Date.now(), releases }))
  } catch {
    // Not important.
  }
}

/** Published releases, newest first, trimmed to what the site shows. Null if GitHub can't be reached. */
export function fetchReleases() {
  pending ??= (async () => {
    const cached = readCache()
    if (cached) return cached
    const res = await fetch(API, { headers: { Accept: 'application/vnd.github+json' } })
    if (!res.ok) throw new Error(`GitHub answered ${res.status}`)
    const releases = (await res.json())
      .filter((r) => !r.draft && !r.prerelease)
      .map((r) => ({
        tag: r.tag_name,
        name: r.name,
        url: r.html_url,
        date: r.published_at,
        body: r.body || '',
        assets: r.assets.map((a) => ({ name: a.name, size: a.size, url: a.browser_download_url })),
      }))
    writeCache(releases)
    return releases
  })().catch(() => null)
  return pending
}

/** undefined while loading, null on failure, otherwise the list of releases. */
export function useReleases() {
  const [releases, setReleases] = useState(undefined)
  useEffect(() => {
    let alive = true
    fetchReleases().then((r) => alive && setReleases(r))
    return () => {
      alive = false
    }
  }, [])
  return releases
}

/** The Windows installer of a release (the updater's .sig and latest.json are skipped). */
export function installerOf(release) {
  return release?.assets.find((a) => /-setup\.exe$/i.test(a.name)) || release?.assets.find((a) => /\.msi$/i.test(a.name))
}

export function windowsFiles(release) {
  return release?.assets.filter((a) => /\.(exe|msi)$/i.test(a.name)) ?? []
}

export function megabytes(bytes) {
  return `${(bytes / 1048576).toFixed(1)} MB`
}

export function longDate(iso) {
  return new Date(iso).toLocaleDateString('en-GB', { day: 'numeric', month: 'long', year: 'numeric' })
}

/** The bullets under "## What's new" (or every bullet when there's no such heading). */
function bulletLines(body) {
  const lines = body.split(/\r?\n/)
  const start = lines.findIndex((l) => /^#+\s*what.?s new/i.test(l))
  const section = start >= 0 ? lines.slice(start + 1) : lines
  const end = section.findIndex((l) => /^#+\s/.test(l))
  return (end >= 0 ? section.slice(0, end) : section).filter((l) => /^\s*[-*]\s+/.test(l)).map((l) => l.replace(/^\s*[-*]\s+/, ''))
}

function escapeHtml(s) {
  return s.replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c])
}

/** A tiny, safe Markdown subset: **bold**, `code` and https links. Everything else is escaped. */
function inlineMarkdown(text) {
  return escapeHtml(text)
    .replace(/\*\*(.+?)\*\*/g, '<b>$1</b>')
    .replace(/`(.+?)`/g, '<code>$1</code>')
    .replace(/\[([^\]]+)\]\((https:\/\/[^)\s]+)\)/g, '<a href="$2">$1</a>')
}

/** "What's new" bullets as safe HTML strings. */
export function notesOf(release) {
  return release ? bulletLines(release.body).map(inlineMarkdown) : []
}

/** A short headline for the hero badge: the first bold phrase of the first bullet. */
export function headlineOf(release) {
  const first = release ? bulletLines(release.body)[0] : null
  const bold = first?.match(/\*\*(.+?)\*\*/)
  return bold ? bold[1].replace(/[:.]$/, '') : null
}
