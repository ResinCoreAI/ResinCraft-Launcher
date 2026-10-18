'use client'

// Everything on the page that depends on the newest GitHub release. Each part renders a sensible
// static fallback first, so the page works without JavaScript or when GitHub can't be reached.

import { Icon } from './Icons'
import { LATEST } from '@/lib/site'
import { headlineOf, installerOf, longDate, megabytes, notesOf, useReleases, windowsFiles } from '@/lib/release'

/** A Download link that points straight at the newest installer once it's known. */
export function DownloadLink({ className = 'btn primary', children }) {
  const releases = useReleases()
  const installer = installerOf(releases?.[0])
  return (
    <a className={className} href={installer?.url || LATEST}>
      <Icon name="download" /> {children}
    </a>
  )
}

export function HeroBadge() {
  const latest = useReleases()?.[0]
  const headline = headlineOf(latest)
  return (
    <a className="hero-badge" href="#whats-new">
      <span className="dot" aria-hidden="true" />
      <span className="ver">{latest?.tag ?? 'Latest'}</span>
      <span className="badge-text">
        {headline ? (
          <>
            New update: <b>{headline}</b>
          </>
        ) : (
          <b>See what's new</b>
        )}
      </span>
      <Icon name="arrow" small />
    </a>
  )
}

export function ReleaseLine() {
  const latest = useReleases()?.[0]
  return (
    <p className="muted">
      {latest ? (
        <>
          Version {latest.tag.replace(/^v/, '')} · released {longDate(latest.date)}
        </>
      ) : (
        'The newest version is always on GitHub.'
      )}
    </p>
  )
}

export function ForYou() {
  const installer = installerOf(useReleases()?.[0])
  return (
    <div className="for-you card reveal">
      <span className="f-ico">
        <Icon name="windows" />
      </span>
      <div className="grow">
        <span className="kicker">For your system</span>
        <p className="file">
          <b className="mono">{installer?.name ?? 'ResinCraft Launcher for Windows'}</b>{' '}
          {installer && <span className="muted">{megabytes(installer.size)}</span>}
        </p>
      </div>
      <a className="btn primary" href={installer?.url || LATEST}>
        <Icon name="download" /> Download
      </a>
    </div>
  )
}

export function WindowsFiles() {
  const files = windowsFiles(useReleases()?.[0])
  return (
    <ul className="files">
      {files.length ? (
        files.map((f) => (
          <li key={f.name}>
            <a href={f.url}>{f.name}</a>
            <span className="size">{megabytes(f.size)}</span>
          </li>
        ))
      ) : (
        <li>
          <a href={LATEST}>Latest installer on GitHub</a>
        </li>
      )}
    </ul>
  )
}

export function WhatsNew() {
  const latest = useReleases()?.[0]
  const notes = notesOf(latest)
  if (!notes.length) return <span id="whats-new" />
  return (
    <section className="notes card" id="whats-new" aria-labelledby="notes-title">
      <h3 id="notes-title">
        What's new in <span className="tag">{latest.tag.replace(/^v/, '')}</span>
      </h3>
      <ul>
        {notes.map((html, i) => (
          // notesOf() escapes everything and only adds <b>, <code> and https links.
          <li key={i} dangerouslySetInnerHTML={{ __html: html }} />
        ))}
      </ul>
      <p className="small" style={{ marginTop: 14 }}>
        <a href={latest.url}>Full release notes on GitHub</a>
      </p>
    </section>
  )
}

export function History() {
  const older = useReleases()?.slice(1) ?? []
  if (!older.length) return null
  return (
    <section className="history card" aria-labelledby="history-title">
      <h3 id="history-title">Previous versions</h3>
      <ul>
        {older.map((r) => (
          <li key={r.tag}>
            <a href={r.url}>{r.tag.replace(/^v/, '')}</a>
            <time dateTime={r.date}>{longDate(r.date)}</time>
          </li>
        ))}
      </ul>
    </section>
  )
}
