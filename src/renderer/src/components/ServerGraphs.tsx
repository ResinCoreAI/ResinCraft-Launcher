// A running server's performance (the Performance tab): CPU, RAM, players and TPS as small line charts drawn with plain
// SVG, and a spark lag report. The engine keeps the history (server/metrics.rs); it ends when the server stops.
import { useEffect, useState } from 'react'
import { Activity, ExternalLink, Gauge, Loader2 } from 'lucide-react'
import type { ServerInfo, ServerMetrics, ServerSample, SparkReport, SparkStatus } from '@shared/types'
import { api, isConsole } from '../api'
import { t } from '../i18n'
import { remotePc } from '../remotePc'
import { useStore } from '../store'
import { bytes, fixed } from '../util'
import { Empty, Modal } from './ui'
import '../server-graphs.css'

const SAMPLE_MS = 5000
const MINUTE = 60_000
/** Shortest stretch of time a chart shows, so the first readings don't fill it. */
const MIN_SPAN = 5 * MINUTE

/** A round top for a chart's scale. */
function niceMax(n: number, steps: number[]): number {
  return steps.find((s) => n <= s) ?? Math.ceil(n / steps[steps.length - 1]) * steps[steps.length - 1]
}

/** The line, broken where readings are missing, in a 100 × 100 box (the SVG stretches to fit). */
function linePath(points: { at: number; v?: number | null }[], from: number, span: number, top: number, gapMs: number): string {
  let d = ''
  let last: number | undefined
  for (const p of points) {
    if (p.v == null) {
      last = undefined
      continue
    }
    const x = ((p.at - from) / span) * 100
    const y = 100 - (Math.min(p.v, top) / top) * 100
    d += `${last === undefined || p.at - last > gapMs ? 'M' : 'L'}${fixed(x, 2)} ${fixed(y, 2)}`
    last = p.at
  }
  return d
}

function Chart({
  label,
  hint,
  current,
  points,
  top,
  tick,
  note
}: {
  label: string
  hint: string
  current: string
  points: { at: number; v?: number | null }[]
  top: number
  tick: (v: number) => string
  note?: string
}) {
  const end = points.length ? points[points.length - 1].at : 0
  const span = Math.max(MIN_SPAN, end - (points[0]?.at ?? 0))
  const d = linePath(points, end - span, span, top, SAMPLE_MS * 3)
  return (
    <div className="card graph">
      <div className="graph-head">
        <div>
          <b>{label}</b>
          <small className="muted">{hint}</small>
        </div>
        <span className="graph-now">{current}</span>
      </div>
      <div className="graph-plot">
        <div className="graph-y" aria-hidden>
          <span>{tick(top)}</span>
          <span>{tick(top / 2)}</span>
          <span>{tick(0)}</span>
        </div>
        <div className="graph-area">
          <svg viewBox="0 0 100 100" preserveAspectRatio="none" role="img" aria-label={`${label}: ${current}`}>
            <line className="graph-grid" x1="0" x2="100" y1="50" y2="50" />
            <path className="graph-line" d={d} />
          </svg>
          {note && <span className="graph-note muted">{note}</span>}
        </div>
      </div>
      <div className="graph-x muted" aria-hidden>
        <span>{t('servers.graphs.minAgo', { n: Math.round(span / MINUTE) })}</span>
        <span>{t('servers.graphs.now')}</span>
      </div>
    </div>
  )
}

function useMetrics(id: string, running: boolean): { data?: ServerMetrics; failed: boolean } {
  const [data, setData] = useState<ServerMetrics>()
  const [failed, setFailed] = useState(false)
  useEffect(() => {
    setData(undefined)
    setFailed(false)
    if (!running) return
    let live = true
    const tick = () =>
      void api.server
        .metrics(id)
        .then((m) => live && setData(m))
        .catch(() => live && setFailed(true))
    tick()
    const h = setInterval(tick, SAMPLE_MS)
    return () => {
      live = false
      clearInterval(h)
    }
  }, [id, running])
  return { data, failed }
}

const pick = (samples: ServerSample[], f: (s: ServerSample) => number | null | undefined) => samples.map((s) => ({ at: s.at, v: f(s) }))

/** The newest reading that has a value. */
function latest(samples: ServerSample[], f: (s: ServerSample) => number | null | undefined): number | undefined {
  for (let i = samples.length - 1; i >= 0; i--) {
    const v = f(samples[i])
    if (v != null) return v
  }
  return undefined
}

function Charts({ m }: { m: ServerMetrics }) {
  const { samples } = m
  const cpu = latest(samples, (s) => s.cpu)
  const ram = latest(samples, (s) => s.bytes)
  const players = samples[samples.length - 1]?.players ?? 0
  const tps = samples[samples.length - 1]?.tps
  const cpuTop = niceMax(Math.max(...samples.map((s) => s.cpu ?? 0), 1), [5, 10, 25, 50, 100])
  const seenBytes = Math.max(...samples.map((s) => s.bytes ?? 0), 1)
  const ramTop = m.maxBytes ? Math.max(m.maxBytes, seenBytes) : niceMax(seenBytes / 2 ** 30, [1, 2, 4, 8, 16, 32]) * 2 ** 30
  const playersTop = niceMax(Math.max(...samples.map((s) => s.players), 1), [4, 10, 20, 50, 100, 200, 500])
  return (
    <div className="graph-grid-wrap">
      <Chart label={t('servers.graphs.cpu')} hint={t('servers.graphs.cpuHint')} current={cpu == null ? '–' : `${fixed(cpu, 1)}%`} points={pick(samples, (s) => s.cpu)} top={cpuTop} tick={(v) => `${fixed(v, v < 10 ? 1 : 0)}%`} />
      <Chart
        label={t('servers.graphs.ram')}
        hint={t('servers.graphs.ramHint')}
        current={ram == null ? '–' : t('common.size.gb', { n: fixed(ram / 2 ** 30, 2) })}
        points={pick(samples, (s) => s.bytes)}
        top={ramTop}
        tick={(v) => t('common.size.gb', { n: fixed(v / 2 ** 30, 1) })}
      />
      <Chart label={t('servers.graphs.players')} hint={t('servers.graphs.playersHint')} current={String(players)} points={pick(samples, (s) => s.players)} top={playersTop} tick={(v) => fixed(v, v % 1 ? 1 : 0)} />
      <Chart
        label={t('servers.graphs.tps')}
        hint={t('servers.graphs.tpsHint')}
        current={m.tpsAvailable && tps != null ? fixed(tps, 1) : t('servers.graphs.notAvailable')}
        points={m.tpsAvailable ? pick(samples, (s) => s.tps) : []}
        top={20}
        tick={(v) => fixed(v, 0)}
        note={!m.tpsAvailable ? t('servers.graphs.tpsNone') : latest(samples, (s) => s.tps) == null ? t('servers.graphs.tpsWaiting') : undefined}
      />
    </div>
  )
}

function ReportProgress({ report }: { report: SparkReport }) {
  const [now, setNow] = useState(Date.now())
  useEffect(() => {
    const h = setInterval(() => setNow(Date.now()), 1000)
    return () => clearInterval(h)
  }, [])
  const left = Math.max(0, report.seconds - Math.round((now - report.startedAt) / 1000))
  const share = Math.min(1, 1 - left / report.seconds)
  const text = report.state === 'profiling' ? t('servers.graphs.lag.profiling', { left }) : t(report.state === 'uploading' ? 'servers.graphs.lag.uploading' : 'servers.graphs.lag.starting')
  return (
    <div className="lag-progress">
      <span className="muted">
        <Loader2 className="spin" size={14} /> {text}
      </span>
      {report.state === 'profiling' ? (
        <div className="bar" role="progressbar" aria-valuenow={Math.round(share * 100)} aria-valuemin={0} aria-valuemax={100}>
          <i style={{ width: `${Math.round(share * 100)}%` }} />
        </div>
      ) : (
        <div className="bar indeterminate" role="progressbar">
          <i />
        </div>
      )}
    </div>
  )
}

const REPORT_SECONDS = 60
const active = (r?: SparkReport | null) => !!r && r.state !== 'done' && r.state !== 'failed'

/** spark on this server: hidden where it can't run, and from the web console and other PCs (installing and uploading stay with the owner). */
function LagReport({ s, running }: { s: ServerInfo; running: boolean }) {
  const { toast } = useStore()
  const [spark, setSpark] = useState<SparkStatus>()
  const [asking, setAsking] = useState(false)
  const [busy, setBusy] = useState(false)
  const reportActive = active(spark?.report)
  const here = !isConsole && !remotePc()

  useEffect(() => {
    if (!here) return
    let live = true
    const tick = () =>
      void api.server
        .spark(s.id)
        .then((v) => live && setSpark(v))
        .catch(() => {})
    tick()
    const h = reportActive ? setInterval(tick, 1000) : undefined
    return () => {
      live = false
      clearInterval(h)
    }
  }, [s.id, here, running, reportActive])

  if (!here || !spark?.available) return null
  const { report } = spark
  const go = async () => {
    setBusy(true)
    try {
      setSpark(spark.needsInstall ? await api.server.sparkInstall(s.id) : await api.server.sparkRun(s.id))
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(false)
      setAsking(false)
    }
  }
  const blocked = spark.needsRestart || (!running && !spark.needsInstall)
  return (
    <div className="card lag">
      <div className="lag-head">
        <Gauge />
        <div className="grow">
          <b>{t('servers.graphs.lag.title')}</b>
          <small className="muted">
            {spark.needsRestart
              ? running
                ? t('servers.graphs.lag.restart')
                : t('servers.graphs.lag.installed')
              : !running && !spark.needsInstall
                ? t('servers.graphs.lag.startFirst')
                : t('servers.graphs.lag.hint', { seconds: report?.seconds ?? REPORT_SECONDS })}
          </small>
        </div>
        <button className="btn primary" disabled={blocked || busy || reportActive} onClick={() => setAsking(true)}>
          <Activity /> {t('servers.graphs.lag.run')}
        </button>
      </div>
      {report && active(report) && <ReportProgress report={report} />}
      {report?.state === 'failed' && <p className="lag-error">{t(`servers.graphs.lag.error.${report.error ?? 'failed'}`)}</p>}
      {report?.url && (
        <div className="lag-done">
          <span>{t('servers.graphs.lag.done')}</span>
          <code>{report.url}</code>
          <button className="btn sm" onClick={() => void api.app.openExternal(report.url!)}>
            <ExternalLink /> {t('servers.graphs.lag.open')}
          </button>
        </div>
      )}
      {asking && (
        <Modal
          title={t('servers.graphs.lag.title')}
          onClose={() => setAsking(false)}
          footer={
            <>
              <button className="btn ghost" onClick={() => setAsking(false)}>
                {t('common.cancel')}
              </button>
              <button className="btn primary" disabled={busy} onClick={() => void go()}>
                {t(spark.needsInstall ? 'servers.graphs.lag.installConfirm' : 'servers.graphs.lag.confirm')}
              </button>
            </>
          }
        >
          {spark.needsInstall && (
            <p>
              {t('servers.graphs.lag.installBody')} {t('servers.graphs.lag.installRestart')}
            </p>
          )}
          <p className="muted">{t('servers.graphs.lag.confirmBody', { seconds: REPORT_SECONDS })}</p>
        </Modal>
      )}
    </div>
  )
}

export default function ServerGraphs({ s }: { s: ServerInfo }) {
  const running = s.state === 'running' || s.state === 'starting'
  const { data, failed } = useMetrics(s.id, running)
  return (
    <div className="graphs">
      <LagReport s={s} running={s.state === 'running'} />
      {!running ? (
        <Empty icon={<Activity />} title={t('servers.graphs.off')}>
          {t('servers.graphs.offBody')}
        </Empty>
      ) : failed ? (
        <p className="muted">{t('servers.graphs.unavailable')}</p>
      ) : data && data.samples.length > 0 ? (
        <Charts m={data} />
      ) : (
        <p className="muted">{t('servers.graphs.collecting')}</p>
      )}
    </div>
  )
}
