// A server's Logs tab: old logs and crash reports, opened in place (gzipped ones too) and searched (server/logs.rs).
// On this PC only: logs hold player IPs and tokens, so remote.rs doesn't let these calls out and Servers.tsx doesn't
// show this in the web console or for another PC.
import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react'
import { ArrowLeft, Download, FolderOpen, RotateCw, ScrollText, Search } from 'lucide-react'
import type { ServerInfo } from '@shared/types'
import type { LogFile, LogFileLine, LogHit, LogPage, LogSearch } from '@shared/serverlogs'
import { api } from '../api'
import { useStore } from '../store'
import { t, type Key } from '../i18n'
import { bytes, timeAgo } from '../util'
import { Empty, Segmented, Switch } from './ui'
import '../server-logs.css'

/** Lines in one page: PAGE_LINES in logs.rs. */
const PAGE = 2000
/** How far above a search result its file opens. */
const LEAD = 100
const DEBOUNCE_MS = 300

/** Marks every match in a line. Like the engine: plain text, any case, in any alphabet. */
function pattern(query: string): RegExp | undefined {
  return query.trim() ? new RegExp(query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), 'giu') : undefined
}

function Marked({ text, re }: { text: string; re?: RegExp }) {
  if (!re) return <>{text}</>
  const parts: ReactNode[] = []
  let at = 0
  for (const m of text.matchAll(re)) {
    parts.push(text.slice(at, m.index), <mark key={m.index}>{m[0]}</mark>)
    at = m.index + m[0].length
  }
  parts.push(text.slice(at))
  return <>{parts}</>
}

function FileRow({ file, on, pick }: { file: LogFile; on: boolean; pick: (f: LogFile) => void }) {
  return (
    <button className={on ? 'on' : ''} aria-current={on} onClick={() => pick(file)}>
      <b>{file.name}</b>
      <small>
        {bytes(file.size)} · {timeAgo(file.modified, true)}
      </small>
    </button>
  )
}

function Row({ l, re, target }: { l: LogFileLine; re?: RegExp; target: boolean }) {
  return (
    <div className={`sv-logs-row ${l.level} ${target ? 'target' : ''}`} data-target={target || undefined}>
      <span className="sv-logs-n">{l.line}</span>
      <span>
        <Marked text={l.text} re={re} />
      </span>
    </div>
  )
}

function Hits({ found, re, many, open }: { found: LogSearch; re?: RegExp; many: boolean; open: (hit: LogHit) => void }) {
  return (
    <>
      {found.hits.map((h, i) => (
        <div key={`${h.path}:${h.line}`}>
          {many && h.path !== found.hits[i - 1]?.path && <div className="sv-logs-file">{h.path}</div>}
          <button className={`sv-logs-row ${h.level}`} onClick={() => open(h)}>
            <span className="sv-logs-n">{h.line}</span>
            <span>
              <Marked text={h.text} re={re} />
            </span>
          </button>
        </div>
      ))}
    </>
  )
}

export default function ServerLogs({ s }: { s: ServerInfo }) {
  const { toast } = useStore()
  const [files, setFiles] = useState<LogFile[]>()
  const [path, setPath] = useState<string>()
  /** The line the page starts at; 0 is the end of the file. */
  const [from, setFrom] = useState(0)
  const [page, setPage] = useState<LogPage>()
  const [query, setQuery] = useState('')
  const [problems, setProblems] = useState(false)
  const [everywhere, setEverywhere] = useState(false)
  const [found, setFound] = useState<LogSearch>()
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string>()
  /** A search result opened in its file: the search stays, to go back to. */
  const [peek, setPeek] = useState(false)
  const [target, setTarget] = useState<number>()
  const [refresh, setRefresh] = useState(0)
  const box = useRef<HTMLDivElement>(null)

  const searching = !peek && (query.trim() !== '' || problems)
  const current = files?.find((f) => f.path === path)
  const re = useMemo(() => pattern(query), [query])

  useEffect(() => {
    let live = true
    api.server.logFiles(s.id).then(
      (list) => {
        if (!live) return
        setFiles(list)
        const gone = !list.some((f) => f.path === path)
        if (gone) {
          const first = list.find((f) => f.path === 'logs/latest.log') ?? list[0]
          setPath(first?.path)
          setFrom(first?.crash ? 1 : 0)
        }
      },
      (e) => live && setError((e as Error).message)
    )
    return () => {
      live = false
    }
    // Only when asked: `path` changes by picking from this list.
  }, [s.id, refresh])

  // The open file.
  useEffect(() => {
    if (searching || !path) return
    let live = true
    setError(undefined)
    api.server.logRead(s.id, path, from).then(
      (p) => live && setPage(p),
      (e) => {
        if (!live) return
        setPage(undefined)
        setError((e as Error).message)
      }
    )
    return () => {
      live = false
    }
  }, [s.id, path, from, searching, refresh])

  // Nothing here stops an older search: the engine ends it when the next one starts.
  useEffect(() => {
    if (!searching) return
    let live = true
    setBusy(true)
    const timer = setTimeout(() => {
      setError(undefined)
      api.server.logSearch(s.id, { path: everywhere ? undefined : path, query, problems }).then(
        (r) => {
          if (!live) return
          setFound(r)
          setBusy(false)
        },
        (e) => {
          if (!live) return
          setFound(undefined)
          setBusy(false)
          setError((e as Error).message)
        }
      )
    }, DEBOUNCE_MS)
    return () => {
      live = false
      clearTimeout(timer)
    }
  }, [s.id, path, query, problems, everywhere, searching, refresh])

  useEffect(() => {
    const el = box.current
    if (!el || searching || !page) return
    const line = el.querySelector('[data-target]')
    if (line) line.scrollIntoView({ block: 'center' })
    else el.scrollTop = from === 0 ? el.scrollHeight : 0
  }, [page, searching, from])

  const jump = (line: number) => {
    setTarget(undefined)
    setFrom(line)
  }
  const pick = (f: LogFile) => {
    setPath(f.path)
    setFrom(f.crash ? 1 : 0)
    setTarget(undefined)
    setPeek(false)
    setEverywhere(false)
    setPage(undefined)
  }
  const openHit = (h: LogHit) => {
    setPath(h.path)
    setFrom(Math.max(1, h.line - LEAD))
    setTarget(h.line)
    setPeek(true)
    setPage(undefined)
  }
  const save = () => {
    if (!path) return
    api.server.logSave(s.id, path).then(
      (to) => to && toast(t('servers.logs.saved', { file: to.split(/[\\/]/).pop() ?? to }), 'info'),
      (e) => toast((e as Error).message, 'error')
    )
  }

  if (files?.length === 0) {
    return (
      <Empty icon={<ScrollText />} title={t('servers.logs.empty')}>
        {t('servers.logs.emptyHint')}
      </Empty>
    )
  }

  const first = page?.lines[0]?.line ?? 1
  const last = page?.lines[page.lines.length - 1]?.line ?? first
  const status = (): string => {
    if (busy) return t('servers.logs.searching')
    if (!found) return ''
    const count = found.hits.length
    return found.more ? t('servers.logs.firstMatches', { count }) : t('servers.logs.matches', { count })
  }
  const group = (key: Key, list: LogFile[]) =>
    list.length > 0 && (
      <>
        <h4>{t(key)}</h4>
        {list.map((f) => (
          <FileRow key={f.path} file={f} on={f.path === path} pick={pick} />
        ))}
      </>
    )
  const folder = current?.crash ? 'servers.logs.openCrashes' : 'servers.console.openLogs'

  return (
    <div className="sv-logs">
      <div className="card sv-logs-side">
        {group('servers.logs.group.logs', files?.filter((f) => !f.crash) ?? [])}
        {group('servers.logs.group.crashes', files?.filter((f) => f.crash) ?? [])}
        <span className="hint">{t('servers.logs.privacy')}</span>
      </div>
      <div className="sv-logs-main">
        <div className="toolbar">
          <div className="search">
            <Search />
            <input
              className="input"
              placeholder={t('servers.logs.search')}
              value={query}
              maxLength={500}
              spellCheck={false}
              onChange={(e) => {
                setQuery(e.target.value)
                setPeek(false)
              }}
            />
          </div>
          <Segmented
            value={everywhere ? 'all' : 'file'}
            options={[
              { value: 'file', label: t('servers.logs.scope.file') },
              { value: 'all', label: t('servers.logs.scope.all') }
            ]}
            onChange={(v) => setEverywhere(v === 'all')}
          />
          <span className="hint">{t('servers.logs.problems')}</span>
          <Switch
            on={problems}
            label={t('servers.logs.problems')}
            onChange={(on) => {
              setProblems(on)
              setPeek(false)
            }}
          />
          <button className="btn icon" title={t('servers.logs.refresh')} aria-label={t('servers.logs.refresh')} onClick={() => setRefresh((n) => n + 1)}>
            <RotateCw />
          </button>
          <button className="btn icon" title={t('servers.logs.save')} aria-label={t('servers.logs.save')} disabled={!path} onClick={save}>
            <Download />
          </button>
          <button className="btn icon" title={t(folder)} aria-label={t(folder)} onClick={() => void api.server.openFolder(s.id, current?.crash ? 'crash-reports' : 'logs')}>
            <FolderOpen />
          </button>
        </div>
        <div className="console" ref={box}>
          {error ? (
            <div className="empty-log">{error}</div>
          ) : searching ? (
            <>
              {found?.capped && <div className="sv-logs-note">{t('servers.logs.capped')}</div>}
              {!!found?.damaged.length && <div className="sv-logs-note">{t('servers.logs.damagedSome', { count: found.damaged.length })}</div>}
              {found && <Hits found={found} re={re} many={everywhere} open={openHit} />}
              {!busy && found?.hits.length === 0 && <div className="empty-log">{t('servers.logs.noMatches')}</div>}
            </>
          ) : page ? (
            <>
              {page.capped && <div className="sv-logs-note">{t('servers.logs.capped')}</div>}
              {page.damaged && <div className="sv-logs-note">{t('servers.logs.damaged')}</div>}
              {page.lines.map((l) => (
                <Row key={l.line} l={l} re={re} target={l.line === target} />
              ))}
              {page.lines.length === 0 && <div className="empty-log">{t('servers.logs.emptyFile')}</div>}
            </>
          ) : (
            <div className="empty-log">{t('servers.logs.loading')}</div>
          )}
        </div>
        <div className="sv-logs-bar">
          {searching ? (
            <span className="hint">{status()}</span>
          ) : (
            <span className="row">
              {peek && (
                <button className="btn sm" onClick={() => setPeek(false)}>
                  <ArrowLeft /> {t('servers.logs.back')}
                </button>
              )}
              <span className="hint">{current?.name}</span>
            </span>
          )}
          {!searching && page && (
            <span className="row">
              <button className="btn sm" disabled={first <= 1} onClick={() => jump(1)}>
                {t('servers.logs.start')}
              </button>
              <button className="btn sm" disabled={first <= 1} onClick={() => jump(Math.max(1, first - PAGE))}>
                {t('servers.logs.earlier')}
              </button>
              <button className="btn sm" disabled={!page.more} onClick={() => jump(last + 1)}>
                {t('servers.logs.later')}
              </button>
              <button className="btn sm" disabled={!page.more} onClick={() => jump(0)}>
                {t('servers.logs.end')}
              </button>
            </span>
          )}
        </div>
      </div>
    </div>
  )
}
