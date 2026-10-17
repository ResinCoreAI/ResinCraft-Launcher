import { useEffect, useState } from 'react'
import { Check, Copy, ExternalLink, Info, Loader2, LogIn, ShieldCheck, Trash2, UserPlus, Users } from 'lucide-react'
import type { DeviceCode } from '@shared/types'
import { api } from '../api'
import { Avatar, ConfirmModal, Empty, Modal } from '../components/ui'
import { useStore } from '../store'

export default function Accounts() {
  const { accounts, refreshAccounts, toast, go, settings, info } = useStore()
  const [signingIn, setSigningIn] = useState(false)
  const [offline, setOffline] = useState(false)
  const [removing, setRemoving] = useState<string>()
  const hasMsa = accounts.some((a) => a.type === 'msa')
  // Release builds need a Microsoft account first; dev builds allow testing before Mojang approval.
  const canOffline = hasMsa || !!info?.dev

  return (
    <div className="page" style={{ maxWidth: 860 }}>
      <div className="page-head">
        <div>
          <h1>Accounts</h1>
          <p>Sign in with the Microsoft account that owns Minecraft: Java Edition.</p>
        </div>
        <div className="spacer" />
        <button className="btn" disabled={!canOffline} title={canOffline ? '' : 'Add a Microsoft account first'} onClick={() => setOffline(true)}>
          <UserPlus /> Offline profile
        </button>
        <button className="btn primary" onClick={() => setSigningIn(true)}>
          <LogIn /> Sign in with Microsoft
        </button>
      </div>

      {!settings?.msClientId && (
        <div className="callout" style={{ marginBottom: 16 }}>
          <Info />
          <div>
            <b>One-time setup needed.</b> Microsoft sign-in needs an Azure application Client ID that Mojang has approved for Minecraft.{' '}
            <button className="link" onClick={() => go({ page: 'settings' })}>
              Add it in Settings
            </button>
            .
          </div>
        </div>
      )}

      {accounts.length === 0 ? (
        <div className="card">
          <Empty
            icon={<Users />}
            title="No accounts yet"
            action={
              <button className="btn primary" onClick={() => setSigningIn(true)}>
                <LogIn /> Sign in with Microsoft
              </button>
            }
          >
            Your password never touches ResinCraft. You sign in on Microsoft's own website.
          </Empty>
        </div>
      ) : (
        <div className="list">
          {accounts.map((a) => (
            <div key={a.id} className="list-row" style={{ padding: '14px 16px' }}>
              <Avatar account={a} size={44} />
              <div className="grow">
                <div className="title">{a.username}</div>
                <div className="file">{a.type === 'msa' ? 'Microsoft account' : 'Offline profile: singleplayer & offline-mode servers only'}</div>
              </div>
              {a.active ? (
                <span className="chip accent">
                  <Check /> Active
                </span>
              ) : (
                <button
                  className="btn sm"
                  onClick={async () => {
                    await api.accounts.setActive(a.id)
                    await refreshAccounts()
                  }}
                >
                  Use this account
                </button>
              )}
              <button className="btn ghost icon sm danger" title="Remove" onClick={() => setRemoving(a.id)}>
                <Trash2 />
              </button>
            </div>
          ))}
        </div>
      )}

      <div className="callout" style={{ marginTop: 16 }}>
        <ShieldCheck />
        <div>Sign-in tokens are encrypted with your operating system's secure storage and only ever sent to Microsoft and Mojang.</div>
      </div>

      {signingIn && <MicrosoftLogin onClose={() => setSigningIn(false)} />}
      {offline && <OfflineModal onClose={() => setOffline(false)} />}
      {removing && (
        <ConfirmModal
          title="Remove this account?"
          body="You can sign in again at any time."
          confirm="Remove"
          onClose={() => setRemoving(undefined)}
          onConfirm={async () => {
            await api.accounts.remove(removing)
            await refreshAccounts()
            toast('Account removed', 'info')
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
        toast(`Welcome, ${e.account?.username}!`)
        onClose()
      } else setError(e.error)
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
    <Modal title="Sign in with Microsoft" subtitle="You'll finish signing in on Microsoft's website." onClose={onClose}>
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
            <li>Click the button below. It copies your code and opens Microsoft's sign-in page.</li>
            <li>Paste the code, then sign in with the account that owns Minecraft.</li>
            <li>Come back here. ResinCraft finishes on its own.</li>
          </ol>
          <div className="device-code">{code.userCode}</div>
          <button className="btn primary lg" onClick={open}>
            {copied ? <Check /> : <Copy />} {copied ? 'Copied! Opening browser…' : 'Copy code & open browser'} <ExternalLink />
          </button>
          <div className="row hint" style={{ justifyContent: 'center' }}>
            <Loader2 className="spin" size={14} /> Waiting for you to finish in the browser…
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
      toast(`Added offline profile ${name}`)
      onClose()
    } catch (e) {
      setError((e as Error).message)
    }
  }

  return (
    <Modal
      title="Add offline profile"
      subtitle="Play singleplayer or LAN without signing in each time."
      onClose={onClose}
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            Cancel
          </button>
          <button className="btn primary" disabled={!valid} onClick={add}>
            Add profile
          </button>
        </>
      }
    >
      <div className="field">
        <label htmlFor="off-name">Username</label>
        <input
          id="off-name"
          className="input"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Steve"
          autoFocus
          onKeyDown={(e) => e.key === 'Enter' && valid && void add()}
        />
        <span className="hint">3–16 characters: letters, numbers and underscores.</span>
      </div>
      {error && <div className="callout danger">{error}</div>}
    </Modal>
  )
}
