// Admin → Users: the relay's dashboard users (each one an admin, linked to a Minecraft account), their 2FA,
// Microsoft sign-in, passwords and remembered launchers; register more admins.
import { useEffect, useState } from 'react'
import { useStore } from '../../store'
import { Copy, Download, KeyRound, Link2, Monitor, Plus, ShieldCheck, ShieldOff, Trash2, UserCog } from 'lucide-react'
import { t } from '../../i18n'
import { ConfirmModal, Modal, MoreButton, useCopy } from '../ui'
import { api } from '../../api'
import { ago, AskModal, Loading, post, shortId, useAct, useAdmin, when } from './common'

interface Launcher {
  id: string
  device: string
  account: string
  createdAt: number
  lastUsed?: number | null
}
interface User {
  name: string
  minecraftUuid?: string | null
  minecraftName?: string
  createdAt: number
  createdBy: string
  lastLogin?: number | null
  you: boolean
  twoFactor: boolean
  recoveryLeft: number
  microsoft: boolean
  microsoftOff: boolean
  launchers: Launcher[]
}
interface Users {
  users: User[]
  legacy: { uuid: string; name?: string; source: 'config' | 'store'; addedBy: string }[]
  you: string
  passwordSet: boolean
  require2fa: boolean
  microsoft: boolean
}

type Ask =
  | { kind: 'link'; u: User }
  | { kind: 'password'; u: User }
  | { kind: 'remove'; u: User }
  | { kind: 'disable2fa'; u: User }
  | { kind: 'register' }
  | { kind: 'legacyUser'; uuid: string; name?: string }

export function UsersAdmin() {
  const { toast } = useStore()
  const { data, error, reload, setData } = useAdmin<Users>('/api/users')
  const act = useAct(reload)
  const [ask, setAsk] = useState<Ask>()
  const [setup, setSetup] = useState(false)
  if (!data) return <Loading error={error} />
  const users = data.users
  return (
    <>
      <div className="toolbar">
        <p className="hint grow">{t(data.require2fa ? 'servers.admin.relay.require2faOn' : 'servers.admin.relay.usersHint')}</p>
        <button className="btn sm primary" onClick={() => setAsk({ kind: 'register' })}>
          <Plus /> {t('servers.admin.relay.register')}
        </button>
      </div>
      <div className="card admin-items">
        {users.map((u) => (
          <div key={u.name} className="setting admin-item">
            <div className="text">
              <b>
                {u.name}
                {u.you && <span className="chip compact accent">{t('servers.admin.relay.you')}</span>}
                <span className={`chip compact ${u.twoFactor ? 'success' : data.require2fa ? 'danger' : ''}`}>
                  {u.twoFactor ? t('servers.admin.relay.twoFactorOn', { left: u.recoveryLeft }) : t('servers.admin.relay.noTwoFactor')}
                </span>
                {data.microsoft && u.minecraftUuid && (
                  <span className={`chip compact ${u.microsoft ? 'success' : ''}`}>{t(u.microsoft ? 'servers.admin.relay.microsoftOn' : 'servers.admin.relay.microsoftOff')}</span>
                )}
              </b>
              <span>
                {u.minecraftUuid ? `${u.minecraftName || shortId(u.minecraftUuid)} (${shortId(u.minecraftUuid)})` : t('servers.admin.relay.notLinked')} ·{' '}
                {t('servers.admin.relay.addedBy', { when: ago(u.createdAt), by: u.createdBy })} · {t('servers.admin.relay.lastSignIn', { when: ago(u.lastLogin) })}
              </span>
              {u.launchers.length > 0 && (
                <span className="hint">
                  <Monitor className="inline" /> {u.launchers.map((l) => l.device).join(', ')}
                </span>
              )}
            </div>
            <MoreButton
              items={() => [
                { label: t(u.minecraftUuid ? 'servers.admin.relay.relink' : 'servers.admin.relay.link'), icon: <Link2 />, onSelect: () => setAsk({ kind: 'link', u }) },
                u.you && !u.twoFactor && { label: t('servers.admin.relay.setup2fa'), icon: <ShieldCheck />, onSelect: () => setSetup(true) },
                u.twoFactor && { label: t('servers.admin.relay.disable2fa'), icon: <ShieldOff />, onSelect: () => setAsk({ kind: 'disable2fa', u }) },
                data.microsoft &&
                  !!u.minecraftUuid && {
                    label: t(u.microsoftOff ? 'servers.admin.relay.microsoftTurnOn' : 'servers.admin.relay.microsoftTurnOff'),
                    icon: <UserCog />,
                    onSelect: () => void act('/api/users/microsoft', { name: u.name, on: u.microsoftOff }, 'servers.admin.relay.done')
                  },
                { label: t(u.you ? 'servers.admin.relay.changePassword' : 'servers.admin.relay.resetPassword'), icon: <KeyRound />, onSelect: () => setAsk({ kind: 'password', u }) },
                u.launchers.length > 0 && { label: t('servers.admin.relay.revokeAll'), icon: <Monitor />, onSelect: () => void act('/api/users/revoke-launchers', { name: u.name }, 'servers.admin.relay.done') },
                '-',
                !u.you && users.length > 1 && { label: t('servers.admin.relay.removeUser'), icon: <Trash2 />, danger: true, onSelect: () => setAsk({ kind: 'remove', u }) }
              ]}
            />
          </div>
        ))}
      </div>

      {users.some((u) => u.launchers.length > 0) && (
        <div className="settings-section">
          <h3>{t('servers.admin.relay.remembered')}</h3>
          <div className="card admin-items">
            {users.flatMap((u) =>
              u.launchers.map((l) => (
                <div key={l.id} className="setting">
                  <div className="text">
                    <b>
                      <Monitor className="inline" /> {l.device} · {u.name}
                    </b>
                    <span>
                      {t('servers.admin.relay.rememberedAgo', { when: ago(l.createdAt) })} · {t('servers.admin.relay.lastUsed', { when: ago(l.lastUsed) })}
                    </span>
                  </div>
                  <button className="btn sm" onClick={() => void act('/api/users/revoke-launcher', { id: l.id }, 'servers.admin.relay.done')}>
                    {t('servers.admin.relay.revoke')}
                  </button>
                </div>
              ))
            )}
          </div>
        </div>
      )}

      {data.legacy.length > 0 && (
        <div className="settings-section">
          <h3>{t('servers.admin.relay.legacy')}</h3>
          <div className="card admin-items">
            {data.legacy.map((l) => (
              <div key={l.uuid} className="setting">
                <div className="text">
                  <b>{l.name || shortId(l.uuid)}</b>
                  <span>{l.source === 'config' ? 'relay.toml' : l.addedBy}</span>
                </div>
                <div className="row">
                  <button className="btn sm" onClick={() => setAsk({ kind: 'legacyUser', uuid: l.uuid, name: l.name })}>
                    {t('servers.admin.relay.setPassword')}
                  </button>
                  {l.source !== 'config' && (
                    <button className="btn sm ghost" onClick={() => void act('/api/admins/remove', { uuid: l.uuid }, 'servers.admin.relay.done')}>
                      {t('servers.admin.remove')}
                    </button>
                  )}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {ask?.kind === 'register' && (
        <AskModal
          title={t('servers.admin.relay.register')}
          fields={[
            { key: 'name', label: t('servers.admin.relay.username'), hint: t('servers.admin.relay.usernameRule') },
            { key: 'minecraft', label: t('servers.admin.relay.linkAccount'), placeholder: t('servers.admin.relay.accountPlaceholder') },
            { key: 'password', label: t('servers.admin.relay.password'), type: 'password', hint: t('servers.admin.relay.passwordRule') },
            { key: 'again', label: t('servers.admin.relay.passwordAgain'), type: 'password' }
          ]}
          confirm={t('servers.admin.relay.registerSend')}
          onSubmit={async (v) => {
            if (v.password !== v.again) return mismatch()
            return act('/api/users/add', { name: v.name.trim(), password: v.password, minecraft: v.minecraft.trim() || undefined }, 'servers.admin.relay.registered', { name: v.name.trim() })
          }}
          onClose={() => setAsk(undefined)}
        />
      )}
      {ask?.kind === 'legacyUser' && (
        <AskModal
          title={t('servers.admin.relay.setPassword')}
          fields={[
            { key: 'name', label: t('servers.admin.relay.username'), value: ask.name ?? '', hint: t('servers.admin.relay.usernameRule') },
            { key: 'password', label: t('servers.admin.relay.password'), type: 'password', hint: t('servers.admin.relay.passwordRule') },
            { key: 'again', label: t('servers.admin.relay.passwordAgain'), type: 'password' }
          ]}
          confirm={t('servers.admin.relay.registerSend')}
          onSubmit={async (v) => {
            if (v.password !== v.again) return mismatch()
            return act('/api/users/add', { name: v.name.trim(), password: v.password, minecraft: ask.uuid }, 'servers.admin.relay.registered', { name: v.name.trim() })
          }}
          onClose={() => setAsk(undefined)}
        />
      )}
      {ask?.kind === 'link' && (
        <AskModal
          title={t('servers.admin.relay.linkTitle', { name: ask.u.name })}
          fields={[{ key: 'minecraft', label: t('servers.admin.relay.linkAccount'), value: ask.u.minecraftName ?? '', hint: t('servers.admin.relay.linkHint') }]}
          confirm={t('servers.admin.relay.link')}
          onSubmit={(v) => act('/api/users/link', { name: ask.u.name, minecraft: v.minecraft.trim() }, 'servers.admin.relay.done')}
          onClose={() => setAsk(undefined)}
        />
      )}
      {ask?.kind === 'password' && (
        <AskModal
          title={t(ask.u.you ? 'servers.admin.relay.changePassword' : 'servers.admin.relay.resetPasswordFor', { name: ask.u.name })}
          fields={[
            ...(ask.u.you ? [{ key: 'current', label: t('servers.admin.relay.currentPassword'), type: 'password' as const }] : []),
            { key: 'password', label: t('servers.admin.relay.newPassword'), type: 'password', hint: t('servers.admin.relay.passwordRule') },
            { key: 'again', label: t('servers.admin.relay.passwordAgain'), type: 'password' }
          ]}
          body={<p className="hint">{t('servers.admin.relay.passwordForgets')}</p>}
          confirm={t('servers.admin.set.save')}
          onSubmit={async (v) => {
            if (v.password !== v.again) return mismatch()
            return act('/api/users/password', ask.u.you ? { password: v.password, current: v.current } : { name: ask.u.name, password: v.password }, 'servers.admin.relay.done')
          }}
          onClose={() => setAsk(undefined)}
        />
      )}
      {ask?.kind === 'disable2fa' &&
        (ask.u.you ? (
          <AskModal
            title={t('servers.admin.relay.disable2fa')}
            fields={[{ key: 'code', label: t('servers.admin.code'), hint: t('servers.admin.codeHint') }]}
            confirm={t('servers.admin.relay.disable2fa')}
            danger
            onSubmit={(v) => act('/api/users/2fa/disable', { code: v.code.trim() }, 'servers.admin.relay.done')}
            onClose={() => setAsk(undefined)}
          />
        ) : (
          <ConfirmModal
            title={t('servers.admin.relay.disable2faFor', { name: ask.u.name })}
            body={t('servers.admin.relay.disable2faBody')}
            confirm={t('servers.admin.relay.disable2fa')}
            onConfirm={async () => void (await act('/api/users/2fa/disable', { name: ask.u.name }, 'servers.admin.relay.done'))}
            onClose={() => setAsk(undefined)}
          />
        ))}
      {ask?.kind === 'remove' && (
        <ConfirmModal
          title={t('servers.admin.relay.removeUserTitle', { name: ask.u.name })}
          body={t('servers.admin.relay.removeUserBody')}
          confirm={t('servers.admin.relay.removeUser')}
          onConfirm={async () => void (await act('/api/users/remove', { name: ask.u.name }, 'servers.admin.relay.done'))}
          onClose={() => setAsk(undefined)}
        />
      )}
      {setup && <TwoFactorSetup onClose={() => setSetup(false)} onDone={(u) => setData(u)} />}
    </>
  )

  /** The two password fields differ: say so and keep the dialog open. */
  function mismatch() {
    toast(t('servers.admin.relay.passwordsDiffer'), 'error')
    return false
  }
}

/** 2FA setup: the key and otpauth link for an authenticator app, then a code to confirm, then the recovery codes. */
export function TwoFactorSetup({ onClose, onDone }: { onClose: () => void; onDone?: (users: Users) => void }) {
  const [secret, setSecret] = useState<{ secret: string; uri: string }>()
  const [error, setError] = useState<string>()
  const [code, setCode] = useState('')
  const [recovery, setRecovery] = useState<string[]>()
  const [busy, setBusy] = useState(false)
  const [qr, setQr] = useState<string>()
  const { toast } = useStore()
  const copy = useCopy()
  useEffect(() => {
    post<{ secret: string; uri: string }>('/api/users/2fa/setup').then(
      (s) => {
        setSecret(s)
        // Drawn on this PC: the key never goes to a QR website.
        api.app.qrSvg(s.uri).then(setQr, () => {})
      },
      (e) => setError((e as Error).message)
    )
  }, [])
  /** The recovery codes as a text file the player keeps. */
  const download = async (codes: string[]) => {
    const text = [t('servers.admin.relay.codesFileTitle'), '', ...codes, '', t('servers.admin.relay.codesFileHint')].join('\r\n')
    try {
      const path = await api.app.saveText('ResinCraft Relay recovery codes.txt', text)
      if (path) toast(t('servers.admin.relay.codesSaved', { path }), 'success')
    } catch (e) {
      toast((e as Error).message, 'error')
    }
  }
  const confirm = async () => {
    setBusy(true)
    setError(undefined)
    try {
      const res = await post<Users & { recoveryCodes: string[] }>('/api/users/2fa/confirm', { code: code.trim() })
      setRecovery(res.recoveryCodes)
      onDone?.(res)
    } catch (e) {
      setError((e as Error).message)
    } finally {
      setBusy(false)
    }
  }
  return (
    <Modal
      title={t('servers.admin.relay.setup2fa')}
      subtitle={recovery ? t('servers.admin.relay.recoveryHint') : t('servers.admin.relay.setup2faHint')}
      onClose={onClose}
      footer={
        recovery ? (
          <button className="btn primary" onClick={onClose}>
            {t('servers.admin.relay.savedCodes')}
          </button>
        ) : (
          <>
            <button className="btn" onClick={onClose}>
              {t('common.cancel')}
            </button>
            <button className="btn primary" disabled={!secret || code.trim().length < 6 || busy} onClick={() => void confirm()}>
              {t('servers.admin.relay.turnOn2fa')}
            </button>
          </>
        )
      }
    >
      {recovery ? (
        <>
          <div className="recovery-codes mono">
            {recovery.map((c) => (
              <span key={c}>{c}</span>
            ))}
          </div>
          <div className="row end recovery-actions">
            <button className="btn sm" onClick={() => void copy(recovery.join('\n'), t('servers.admin.relay.codesCopied'))}>
              <Copy /> {t('servers.admin.relay.copyCodes')}
            </button>
            <button className="btn sm primary" onClick={() => void download(recovery)}>
              <Download /> {t('servers.admin.relay.downloadCodes')}
            </button>
          </div>
        </>
      ) : !secret ? (
        <Loading error={error} />
      ) : (
        <div className="listing-fields">
          {qr && (
            <div className="twofa-qr">
              <img src={`data:image/svg+xml;charset=utf-8,${encodeURIComponent(qr)}`} alt={t('servers.admin.relay.qrAlt')} width={200} height={200} />
              <span className="hint">{t('servers.admin.relay.qrHint')}</span>
            </div>
          )}
          <label className="field">
            <span>{t('servers.admin.relay.setupKey')}</span>
            <code className="secret-key">{secret.secret.replace(/(.{4})/g, '$1 ').trim()}</code>
          </label>
          <label className="field">
            <span>{t('servers.admin.relay.setupLink')}</span>
            <input className="input mono" readOnly value={secret.uri} onFocus={(e) => e.currentTarget.select()} />
          </label>
          <label className="field">
            <span>{t('servers.admin.code')}</span>
            <input className="input" inputMode="numeric" autoComplete="one-time-code" maxLength={6} autoFocus value={code} placeholder="123456" onChange={(e) => setCode(e.target.value.replace(/\D/g, ''))} />
          </label>
          {error && <p className="hint bad">{error}</p>}
        </div>
      )}
    </Modal>
  )
}
