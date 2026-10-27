import { useEffect, useState } from 'react'
import { ExternalLink, Globe, Smartphone, X } from 'lucide-react'
import type { RemoteStatus } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { t, type Key } from '../i18n'
import { Switch } from './ui'

const STATE_TEXT: Record<RemoteStatus['state'], Key> = {
  off: 'settings.remote.state.off',
  connecting: 'settings.remote.state.connecting',
  online: 'settings.remote.state.online',
  noAccount: 'settings.remote.state.noAccount',
  unavailable: 'settings.remote.state.unavailable',
  relayTooOld: 'settings.remote.state.relayTooOld',
  error: 'settings.remote.state.error'
}

/**
 * Settings → Remote control: run hosted servers from the web console (console.<relay domain>) while ResinCraft is
 * open or in the tray. The engine keeps the relay connection (remote.rs); this shows how it's doing.
 */
export function RemoteControlSection() {
  const { settings, updateSettings, toast } = useStore()
  const [status, setStatus] = useState<RemoteStatus>()
  useEffect(() => {
    void api.remote.status().then(setStatus, () => {})
    return api.on.remote((s) => setStatus((old) => ({ ...s, url: s.url ?? old?.url })))
  }, [])
  if (!settings) return null
  const on = !!settings.remoteControl
  // Just switched on: the engine's first status is on its way.
  const state = !on ? 'off' : !status || status.state === 'off' ? 'connecting' : status.state
  const line =
    state === 'online'
      ? status?.viewers
        ? t('settings.remote.viewers', { name: status.account ?? '', count: status.viewers })
        : t('settings.remote.state.online', { name: status?.account ?? '' })
      : t(STATE_TEXT[state], { name: status?.account ?? '' })

  return (
    <section className="settings-section" id="remote">
      <h3>{t('settings.remote')}</h3>
      <p>{t('settings.remote.lead')}</p>
      <div className="card">
        <div className="setting">
          <div className="text">
            <b>{t('settings.remote.switch')}</b>
            <span>{t('settings.remote.hint')}</span>
          </div>
          <Switch
            on={on}
            onChange={(v) => void updateSettings({ remoteControl: v }).catch((e) => toast((e as Error).message, 'error'))}
            label={t('settings.remote.switch')}
          />
        </div>
        {on && (
          <div className="setting">
            <div className="text">
              <b>{t('settings.remote.full')}</b>
              <span>{t('settings.remote.fullHint')}</span>
            </div>
            <Switch
              on={!!settings.remoteFullControl}
              onChange={(v) => void updateSettings({ remoteFullControl: v }).catch((e) => toast((e as Error).message, 'error'))}
              label={t('settings.remote.full')}
            />
          </div>
        )}
        {on && (
          <div className="setting">
            <div className="text">
              <b className="row" style={{ gap: 8 }}>
                <span className={`remote-dot ${state === 'online' ? 'on' : state === 'connecting' ? 'wait' : 'off'}`} aria-hidden />
                {line}
              </b>
              {state === 'error' && status?.error && <span>{status.error}</span>}
              {!settings.tray && <span>{t('settings.remote.tray')}</span>}
            </div>
            {status?.url && (
              <button className="btn" onClick={() => void api.app.openExternal(status.url!)}>
                <Globe /> {t('settings.remote.open')} <ExternalLink size={13} />
              </button>
            )}
          </div>
        )}
      </div>
    </section>
  )
}

/** The engine's remote-control status, kept current. */
function useRemoteStatus() {
  const [status, setStatus] = useState<RemoteStatus>()
  useEffect(() => {
    void api.remote.status().then(setStatus, () => {})
    return api.on.remote((s) => setStatus((old) => ({ ...s, url: s.url ?? old?.url })))
  }, [])
  return status
}

/** The web console signs in with a Microsoft account that owns Minecraft: without one there's nothing to open. */
function useHasMicrosoft() {
  const { accounts } = useStore()
  return accounts.some((a) => a.type === 'msa')
}

/** Servers page header: opens the web console while remote control is on (or asks for a Microsoft sign-in first). */
export function WebConsoleButton() {
  const { settings, go } = useStore()
  const status = useRemoteStatus()
  const microsoft = useHasMicrosoft()
  if (!settings?.remoteControl) return null
  if (!microsoft || status?.state === 'noAccount')
    return (
      <button data-pc-only className="btn lg" title={t('settings.remote.state.noAccount')} onClick={() => go({ page: 'accounts' })}>
        <Globe /> {t('servers.web.signIn')}
      </button>
    )
  if (!status?.url) return null
  return (
    <button data-pc-only className="btn lg" title={t('settings.remote.open')} onClick={() => void api.app.openExternal(status.url!)}>
      <Globe /> {t('servers.web.open')} <ExternalLink size={13} />
    </button>
  )
}

const TIP_KEY = 'resincraft.tip.webConsole'

/** Servers page: tells players about the web console while remote control is off (until they dismiss it). */
export function WebConsoleTip() {
  const { settings, go } = useStore()
  const status = useRemoteStatus()
  const microsoft = useHasMicrosoft()
  const [hidden, setHidden] = useState(() => {
    try {
      return localStorage.getItem(TIP_KEY) === 'hidden'
    } catch {
      return false
    }
  })
  // Builds without ResinCraft Relay have no console to offer.
  if (hidden || !settings || settings.remoteControl || !status || status.state === 'unavailable') return null
  const dismiss = () => {
    setHidden(true)
    try {
      localStorage.setItem(TIP_KEY, 'hidden')
    } catch {
      // Private storage: hidden for this visit.
    }
  }
  const turnOn = () => {
    go({ page: 'settings' })
    // Once the Settings page is drawn, bring its Remote control section into view.
    window.setTimeout(() => document.getElementById('remote')?.scrollIntoView({ behavior: 'smooth', block: 'start' }), 150)
  }
  return (
    <div data-pc-only className="card sv-web-tip">
      <Smartphone className="sv-web-tip-icon" />
      <div className="grow">
        <b>{t('servers.web.tipTitle')}</b>
        <p className="hint">{t(microsoft ? 'servers.web.tipBody' : 'servers.web.tipNoAccount')}</p>
      </div>
      {microsoft ? (
        <button className="btn sm primary" onClick={turnOn}>
          {t('servers.web.tipTurnOn')}
        </button>
      ) : (
        <button className="btn sm primary" onClick={() => go({ page: 'accounts' })}>
          {t('servers.web.tipSignIn')}
        </button>
      )}
      <button className="btn sm ghost icon-only" title={t('servers.web.tipDismiss')} aria-label={t('servers.web.tipDismiss')} onClick={dismiss}>
        <X />
      </button>
    </div>
  )
}

/** A small note in the launcher whenever the web console changes something. */
export function useRemoteActions() {
  const { toast } = useStore()
  useEffect(() => api.on.remoteAction(() => toast(t('settings.remote.action'), 'info')), [toast])
}
