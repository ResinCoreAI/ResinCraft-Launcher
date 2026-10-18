import { useEffect, useState } from 'react'
import { Check, Copy, ExternalLink, Fingerprint, Info, Loader2, LogIn, Shirt, ShieldCheck, Trash2, UserCheck, UserPlus, Users } from 'lucide-react'
import type { AccountView, DeviceCode } from '@shared/types'
import { api } from '../api'
import { Avatar, ConfirmModal, Empty, Modal, openContextMenu, useCopy } from '../components/ui'
import { useStore } from '../store'
import ProfileManager from '../components/ProfileManager'
import { rich, t, tx } from '../i18n'

export default function Accounts() {
  const { accounts, refreshAccounts, toast, go, settings, info } = useStore()
  const [signingIn, setSigningIn] = useState(false)
  const [offline, setOffline] = useState(false)
  const [removing, setRemoving] = useState<string>()
  const [managing, setManaging] = useState<AccountView>()
  const copy = useCopy()
  return (
    <div className="page" style={{ maxWidth: 860 }}>
      <div className="page-head">
        <div>
          <h1>{t('accounts.title')}</h1>
          <p>{t('accounts.subtitle')}</p>
        </div>
        <div className="spacer" />
        <button className="btn" onClick={() => setOffline(true)}>
          <UserPlus /> {t('accounts.offlineProfile')}
        </button>
        <button className="btn primary" onClick={() => setSigningIn(true)}>
          <LogIn /> {t('accounts.signIn')}
        </button>
      </div>

      {info && !info.microsoft && (
        <div className="callout" style={{ marginBottom: 16 }}>
          <Info />
          <div>
            {rich('accounts.noMicrosoft', { b: (s) => <b>{s}</b> })}
          </div>
        </div>
      )}

      {accounts.length === 0 ? (
        <div className="card">
          <Empty
            icon={<Users />}
            title={t('accounts.empty.title')}
            action={
              <button className="btn primary" onClick={() => setSigningIn(true)}>
                <LogIn /> {t('accounts.signIn')}
              </button>
            }
          >
            {t('accounts.empty.body')}
          </Empty>
        </div>
      ) : (
        <div className="list">
          {accounts.map((a) => (
            <div
              key={a.id}
              className="list-row"
              style={{ padding: '14px 16px' }}
              onContextMenu={(e) =>
                openContextMenu(e, [
                  {
                    label: t('accounts.use'),
                    icon: <UserCheck />,
                    checked: a.active,
                    disabled: a.active,
                    onSelect: async () => {
                      await api.accounts.setActive(a.id)
                      await refreshAccounts()
                    }
                  },
                  a.type === 'msa' && { label: t('menu.account.manage'), icon: <Shirt />, onSelect: () => setManaging(a) },
                  '-',
                  { label: t('menu.account.copyName'), icon: <Copy />, onSelect: () => void copy(a.username) },
                  { label: t('menu.account.copyUuid'), icon: <Fingerprint />, onSelect: () => void copy(a.uuid) },
                  '-',
                  { label: t('common.remove'), icon: <Trash2 />, danger: true, onSelect: () => setRemoving(a.id) }
                ])
              }
            >
              <Avatar account={a} size={44} />
              <div className="grow">
                <div className="title">{a.username}</div>
                <div className="file">{t(a.type === 'msa' ? 'accounts.kind.msa' : 'accounts.kind.offline')}</div>
              </div>
              {a.type === 'msa' && (
                <button className="btn sm" onClick={() => setManaging(a)}>
                  <Shirt /> {t('accounts.manage')}
                </button>
              )}
              {a.active ? (
                <span className="chip accent">
                  <Check /> {t('accounts.active')}
                </span>
              ) : (
                <button
                  className="btn sm"
                  onClick={async () => {
                    await api.accounts.setActive(a.id)
                    await refreshAccounts()
                  }}
                >
                  {t('accounts.use')}
                </button>
              )}
              <button className="btn ghost icon sm danger" title={t('common.remove')} onClick={() => setRemoving(a.id)}>
                <Trash2 />
              </button>
            </div>
          ))}
        </div>
      )}

      <div className="callout" style={{ marginTop: 16 }}>
        <ShieldCheck />
        <div>{t('accounts.security')}</div>
      </div>

      {signingIn && <MicrosoftLogin onClose={() => setSigningIn(false)} />}
      {offline && <OfflineModal onClose={() => setOffline(false)} />}
      {managing && <ProfileManager account={managing} onClose={() => setManaging(undefined)} />}
      {removing && (
        <ConfirmModal
          title={t('accounts.remove.title')}
          body={t('accounts.remove.body')}
          confirm={t('common.remove')}
          onClose={() => setRemoving(undefined)}
          onConfirm={async () => {
            await api.accounts.remove(removing)
            await refreshAccounts()
            toast(t('accounts.removed'), 'info')
          }}
        />
      )}
    </div>
  )
}

function MicrosoftLogin({ onClose }: { onClose: () => void }) {
  const { refreshAccounts, toast } = useStore()
  const [code, setCode] = useState<DeviceCode>()
  const [error, setError] = useState<string>()
  const [copied, setCopied] = useState(false)

  useEffect(() => {
    const off = api.on.login(async (e) => {
      if (e.state === 'success') {
        await refreshAccounts()
        toast(t('accounts.login.welcome', { name: e.account?.username ?? '' }))
        onClose()
      } else setError(tx(e.errorText, e.error))
    })
    api.accounts.beginMicrosoft().then(setCode, (e) => setError((e as Error).message))
    return () => {
      off()
      void api.accounts.cancelMicrosoft()
    }
  }, [])

  const open = async () => {
    if (!code) return
    await navigator.clipboard.writeText(code.userCode)
    setCopied(true)
    await api.app.openExternal(code.verificationUri)
  }

  return (
    <Modal title={t('accounts.login.title')} subtitle={t('accounts.login.subtitle')} onClose={onClose}>
      {error ? (
        <div className="callout danger">
          <Info />
          <div>{error}</div>
        </div>
      ) : !code ? (
        <div className="empty" style={{ padding: 30 }}>
          <Loader2 className="spin" />
        </div>
      ) : (
        <>
          <ol className="muted" style={{ margin: 0, paddingLeft: 18, lineHeight: 1.9 }}>
            <li>{t('accounts.login.step1')}</li>
            <li>{t('accounts.login.step2')}</li>
            <li>{t('accounts.login.step3')}</li>
          </ol>
          <div className="device-code">{code.userCode}</div>
          <button className="btn primary lg" onClick={open}>
            {copied ? <Check /> : <Copy />} {t(copied ? 'accounts.login.opening' : 'accounts.login.open')} <ExternalLink />
          </button>
          <div className="row hint" style={{ justifyContent: 'center' }}>
            <Loader2 className="spin" size={14} /> {t('accounts.login.waiting')}
          </div>
        </>
      )}
    </Modal>
  )
}

function OfflineModal({ onClose }: { onClose: () => void }) {
  const { refreshAccounts, toast } = useStore()
  const [name, setName] = useState('')
  const [error, setError] = useState<string>()
  const valid = /^[A-Za-z0-9_]{3,16}$/.test(name)

  const add = async () => {
    try {
      await api.accounts.addOffline(name)
      await refreshAccounts()
      toast(t('accounts.offline.added', { name }))
      onClose()
    } catch (e) {
      setError((e as Error).message)
    }
  }

  return (
    <Modal
      title={t('accounts.offline.title')}
      subtitle={t('accounts.offline.subtitle')}
      onClose={onClose}
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={!valid} onClick={add}>
            {t('accounts.offline.add')}
          </button>
        </>
      }
    >
      <div className="field">
        <label htmlFor="off-name">{t('accounts.offline.username')}</label>
        <input
          id="off-name"
          className="input"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Steve"
          autoFocus
          onKeyDown={(e) => e.key === 'Enter' && valid && void add()}
        />
        <span className="hint">{t('accounts.offline.rules')}</span>
      </div>
      {error && <div className="callout danger">{error}</div>}
    </Modal>
  )
}
