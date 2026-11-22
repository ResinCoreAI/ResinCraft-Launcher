// The email address ResinCraft mails the active Microsoft account at: add it, confirm it with the 6-digit code the
// relay emails, change or remove it. Mail by email only ever goes to a confirmed address. (Accounts page, and the
// Inbox page's Email button.)
import { useEffect, useState } from 'react'
import { Check, Loader2, MailCheck, RefreshCw, Trash2 } from 'lucide-react'
import type { InboxEmail } from '@shared/types'
import { inbox } from '../inboxState'
import { useStore } from '../store'
import { currentLanguage, t } from '../i18n'
import { ConfirmModal } from './ui'

export default function EmailAddress({ compact }: { compact?: boolean }) {
  const { toast, activeAccount } = useStore()
  const [state, setState] = useState<InboxEmail>()
  const [error, setError] = useState('')
  const [editing, setEditing] = useState(false)
  const [address, setAddress] = useState('')
  const [code, setCode] = useState('')
  const [busy, setBusy] = useState(false)
  const [removing, setRemoving] = useState(false)
  const [, setTick] = useState(0)

  useEffect(() => {
    let live = true
    setState(undefined)
    setError('')
    inbox.email().then(
      (s) => live && setState(s),
      (e) => live && setError((e as Error).message)
    )
    return () => {
      live = false
    }
  }, [activeAccount?.id])
  // "Send again in 40 s" counts down.
  const resendIn = state?.pending ? Math.max(0, state.pending.resendAt - Math.floor(Date.now() / 1000)) : 0
  useEffect(() => {
    if (resendIn <= 0) return
    const timer = window.setTimeout(() => setTick((n) => n + 1), 1000)
    return () => window.clearTimeout(timer)
  })

  const run = async (work: () => Promise<InboxEmail>, done?: string) => {
    setBusy(true)
    try {
      const s = await work()
      setState(s)
      if (done) toast(done, 'success')
      return s
    } catch (e) {
      toast((e as Error).message, 'error')
      return undefined
    } finally {
      setBusy(false)
    }
  }
  const send = async (to: string) => {
    const s = await run(() => inbox.startEmail(to.trim()), t('inbox.email.sent', { address: to.trim() }))
    if (s) {
      setEditing(false)
      setCode('')
    }
  }

  if (error) return <p className="hint">{error}</p>
  if (!state)
    return (
      <p className="hint">
        <Loader2 className="spin inline" /> {t('common.loading')}
      </p>
    )
  if (!state.on && !state.address) return <p className="hint">{t('inbox.email.off')}</p>

  const asking = editing || (!state.address && !state.pending)
  const date = (s: number) => new Date(s * 1000).toLocaleDateString(currentLanguage(), { dateStyle: 'medium' })
  return (
    <div className={`email-box ${compact ? 'compact' : ''}`}>
      {state.address && !editing && (
        <div className="email-current">
          <MailCheck className="email-ok" aria-hidden />
          <div className="grow">
            <b>{state.address}</b>
            <span className="hint">{t('inbox.email.confirmed', { date: date(state.verifiedAt ?? 0) })}</span>
          </div>
          {state.on && (
            <button className="btn sm" onClick={() => (setEditing(true), setAddress(''))} disabled={busy}>
              {t('inbox.email.change')}
            </button>
          )}
          <button className="btn sm ghost danger" onClick={() => setRemoving(true)} disabled={busy} title={t('inbox.email.remove')}>
            <Trash2 /> {t('common.remove')}
          </button>
        </div>
      )}

      {state.pending && !editing && (
        <form
          className="email-code"
          onSubmit={(e) => {
            e.preventDefault()
            void run(() => inbox.confirmEmail(code), t('inbox.email.done')).then((s) => s && setCode(''))
          }}
        >
          <p>{t('inbox.email.codeSent', { address: state.pending.address })}</p>
          <div className="row">
            <input
              className="input email-code-input mono"
              inputMode="numeric"
              autoComplete="one-time-code"
              maxLength={7}
              placeholder="123456"
              aria-label={t('inbox.email.code')}
              value={code}
              onChange={(e) => setCode(e.target.value.replace(/[^\d ]/g, ''))}
              autoFocus
            />
            <button className="btn primary" type="submit" disabled={busy || code.replace(/\D/g, '').length !== 6}>
              {busy ? <Loader2 className="spin" /> : <Check />} {t('inbox.email.confirm')}
            </button>
            <button type="button" className="btn ghost" disabled={busy || resendIn > 0} onClick={() => void send(state.pending!.address)}>
              <RefreshCw /> {resendIn > 0 ? t('inbox.email.resendIn', { secs: resendIn }) : t('inbox.email.resend')}
            </button>
          </div>
          <button type="button" className="text-btn" onClick={() => (setEditing(true), setAddress(''))}>
            {t('inbox.email.other')}
          </button>
        </form>
      )}

      {asking && state.on && (
        <form
          className="email-ask"
          onSubmit={(e) => {
            e.preventDefault()
            void send(address)
          }}
        >
          {!compact && <p className="hint">{t('inbox.email.why')}</p>}
          <div className="row">
            <input
              className="input grow"
              type="email"
              autoComplete="email"
              placeholder={t('inbox.email.placeholder')}
              aria-label={t('inbox.email.label')}
              value={address}
              onChange={(e) => setAddress(e.target.value)}
              autoFocus={editing}
            />
            <button className="btn primary" type="submit" disabled={busy || !/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(address.trim())}>
              {busy ? <Loader2 className="spin" /> : null} {t('inbox.email.send')}
            </button>
            {editing && (
              <button type="button" className="btn ghost" onClick={() => setEditing(false)}>
                {t('common.cancel')}
              </button>
            )}
          </div>
        </form>
      )}

      {removing && (
        <ConfirmModal
          title={t('inbox.email.removeTitle')}
          body={t('inbox.email.removeBody', { address: state.address ?? '' })}
          confirm={t('common.remove')}
          onClose={() => setRemoving(false)}
          onConfirm={async () => {
            await run(() => inbox.removeEmail(), t('inbox.email.removed'))
          }}
        />
      )}
    </div>
  )
}
