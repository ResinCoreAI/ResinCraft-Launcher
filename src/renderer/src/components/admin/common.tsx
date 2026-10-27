// Shared bits of the Admin page's relay screens (pages/Admin.tsx): calls over the relay connection, polling, and
// small formatting helpers. The relay's admin API is documented in relay-server/docs/admin-api.md.
import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react'
import { Loader2 } from 'lucide-react'
import { api } from '../../api'
import { useStore } from '../../store'
import { currentLanguage, t, type Key } from '../../i18n'
import { timeAgo } from '../../util'
import { Modal } from '../ui'

export const get = <T,>(path: string) => api.server.adminCall('GET', path) as Promise<T>
export const post = <T = unknown,>(path: string, body: unknown = {}) => api.server.adminCall('POST', path, body) as Promise<T>

/** A route's answer, loaded now and again every `pollMs` (while the screen is open). */
export function useAdmin<T>(path: string, pollMs?: number) {
  const [data, setData] = useState<T>()
  const [error, setError] = useState<string>()
  const live = useRef(true)
  const reload = useCallback(async () => {
    try {
      const v = await get<T>(path)
      if (live.current) {
        setData(v)
        setError(undefined)
      }
    } catch (e) {
      if (live.current) setError((e as Error).message)
    }
  }, [path])
  useEffect(() => {
    live.current = true
    void reload()
    const h = pollMs ? setInterval(() => void reload(), pollMs) : undefined
    return () => {
      live.current = false
      if (h) clearInterval(h)
    }
  }, [reload, pollMs])
  return { data, error, reload, setData }
}

/** A change: posts, toasts `done` (or the relay's error), reloads. Resolves true when it worked. */
export function useAct(reload: () => unknown) {
  const { toast } = useStore()
  return useCallback(
    async (path: string, body: unknown = {}, done?: Key, params?: Record<string, string | number>) => {
      try {
        await post(path, body)
        if (done) toast(t(done, params), 'success')
        await reload()
        return true
      } catch (e) {
        toast((e as Error).message, 'error')
        return false
      }
    },
    [reload, toast]
  )
}

/** Unix seconds → "3 minutes ago" (empty for none). */
export const ago = (secs?: number | null) => (secs ? timeAgo(secs * 1000, true) : '—')
export const when = (secs?: number | null) => (secs ? new Date(secs * 1000).toLocaleString(currentLanguage(), { dateStyle: 'medium', timeStyle: 'short' }) : '—')
/** Bytes up to TB, in 1000s like the relay's GB limits (the launcher's own helper stops at MB). */
export function size(n?: number | null): string {
  if (!n) return '0 B'
  const u = ['B', 'KB', 'MB', 'GB', 'TB']
  let i = 0
  let v = n
  while (v >= 1000 && i < u.length - 1) {
    v /= 1000
    i++
  }
  return `${Intl.NumberFormat(currentLanguage(), { maximumFractionDigits: i >= 3 ? 1 : 0 }).format(v)} ${u[i]}`
}
/** "1 h 20 min" for a number of seconds. */
export function duration(secs?: number | null): string {
  if (secs == null) return '—'
  const s = Math.max(0, Math.round(secs))
  const d = Math.floor(s / 86400)
  const h = Math.floor((s % 86400) / 3600)
  const m = Math.floor((s % 3600) / 60)
  if (d) return `${d} d ${h} h`
  if (h) return `${h} h ${m} min`
  return `${m} min`
}
export const shortId = (uuid?: string | null) => (uuid ? uuid.slice(0, 8) : '')

export function Stat({ label, value, tone }: { label: ReactNode; value: ReactNode; tone?: 'ok' | 'warn' | 'bad' }) {
  return (
    <div className={`stat admin-stat ${tone ?? ''}`}>
      <b>{value}</b>
      <span>{label}</span>
    </div>
  )
}

export function Loading({ error }: { error?: string }) {
  return error ? <p className="hint bad">{error}</p> : <Loader2 className="spin" />
}

export type Field = { key: string; label: string; type?: 'text' | 'number' | 'password' | 'textarea'; placeholder?: string; value?: string; hint?: string }

/** A small form in a dialog (a reason, an amount, a name…); `onSubmit` resolves true to close. */
export function AskModal({
  title,
  fields,
  confirm,
  danger,
  body,
  onSubmit,
  onClose
}: {
  title: string
  fields: Field[]
  confirm: string
  danger?: boolean
  body?: ReactNode
  onSubmit: (values: Record<string, string>) => Promise<boolean>
  onClose: () => void
}) {
  const [values, setValues] = useState<Record<string, string>>(() => Object.fromEntries(fields.map((f) => [f.key, f.value ?? ''])))
  const [busy, setBusy] = useState(false)
  const go = async () => {
    setBusy(true)
    const ok = await onSubmit(values)
    setBusy(false)
    if (ok) onClose()
  }
  return (
    <Modal
      title={title}
      onClose={onClose}
      footer={
        <>
          <button className="btn" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className={`btn ${danger ? 'danger' : 'primary'}`} disabled={busy} onClick={() => void go()}>
            {busy && <Loader2 className="spin" />} {confirm}
          </button>
        </>
      }
    >
      {body}
      <form
        className="listing-fields"
        onSubmit={(e) => {
          e.preventDefault()
          void go()
        }}
      >
        {fields.map((f, i) => (
          <label key={f.key} className="field">
            <span>{f.label}</span>
            {f.type === 'textarea' ? (
              <textarea className="input" rows={3} autoFocus={i === 0} value={values[f.key]} placeholder={f.placeholder} onChange={(e) => setValues({ ...values, [f.key]: e.target.value })} />
            ) : (
              <input
                className="input"
                type={f.type ?? 'text'}
                autoFocus={i === 0}
                value={values[f.key]}
                placeholder={f.placeholder}
                spellCheck={false}
                onChange={(e) => setValues({ ...values, [f.key]: e.target.value })}
              />
            )}
            {f.hint && <span className="hint">{f.hint}</span>}
          </label>
        ))}
        <button type="submit" hidden />
      </form>
    </Modal>
  )
}

/** An account field the relay takes: a UUID (dashes optional) or a Minecraft name it has seen. */
export const account = (who: string) => (/^[0-9a-f]{8}-?[0-9a-f]{4}-?[0-9a-f]{4}-?[0-9a-f]{4}-?[0-9a-f]{12}$/i.test(who.trim()) ? { uuid: who.trim() } : { name: who.trim() })
