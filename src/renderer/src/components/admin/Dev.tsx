// Admin → Dev: who may use the launcher's Dev update channel, and the Dev build release-dev.bat uploaded (the relay's
// Dev page; relay-server's devbuilds.rs).
import { useState } from 'react'
import { FlaskConical, Trash2, UserPlus } from 'lucide-react'
import { t } from '../../i18n'
import { ConfirmModal } from '../ui'
import { ago, Loading, size, useAct, useAdmin } from './common'

interface DevInvite {
  uuid: string
  name: string
  addedAt: number
  addedBy: string
}
interface DevData {
  invites: DevInvite[]
  build: { version: string; notes: string; uploadedAt: number; platforms: { target: string; size: number; uploadedAt: number }[] } | null
  /** `[dev_builds] upload_tokens` is set, so release-dev.bat can upload. */
  uploads: boolean
}

export function DevAdmin() {
  const { data, error, reload } = useAdmin<DevData>('/api/devs')
  const act = useAct(reload)
  const [who, setWho] = useState('')
  const [removing, setRemoving] = useState(false)
  const invite = async () => {
    const name = who.trim()
    if (await act('/api/devs/add', { who: name }, 'servers.admin.dev.invited', { name })) setWho('')
  }
  if (!data) return <Loading error={error} />
  const b = data.build
  return (
    <>
      <p className="hint">{t('servers.admin.dev.lead')}</p>
      <div className="card" style={{ marginBottom: 18 }}>
        <div className="setting">
          <div className="text">
            <b>
              <FlaskConical className="inline" /> {b ? t('servers.admin.dev.build', { version: b.version }) : t('servers.admin.dev.noBuildTitle')}
            </b>
            {b ? (
              <>
                <span>
                  {t('servers.admin.dev.uploaded', { when: ago(b.uploadedAt) })} · {b.platforms.map((p) => `${p.target} (${size(p.size)})`).join(', ')}
                </span>
                {b.notes && <div className="hint">{b.notes}</div>}
              </>
            ) : (
              <span>{data.uploads ? t('servers.admin.dev.noBuild') : t('servers.admin.dev.uploadsOff')}</span>
            )}
          </div>
          {b && (
            <button className="btn sm danger" onClick={() => setRemoving(true)}>
              <Trash2 /> {t('servers.admin.dev.removeBuild')}
            </button>
          )}
        </div>
      </div>

      <div className="settings-section">
        <h3>{t('servers.admin.dev.invite')}</h3>
        <form
          className="card"
          onSubmit={(e) => {
            e.preventDefault()
            if (who.trim()) void invite()
          }}
        >
          <div className="setting">
            <label className="field" style={{ flex: 1 }}>
              <span>{t('servers.admin.dev.inviteHint')}</span>
              <input className="input" value={who} maxLength={36} spellCheck={false} placeholder="Notch / 069a79f4-44e9-…" onChange={(e) => setWho(e.target.value)} />
            </label>
            <button className="btn sm primary" type="submit" disabled={!who.trim()}>
              <UserPlus /> {t('servers.admin.dev.inviteButton')}
            </button>
          </div>
        </form>
      </div>

      <div className="settings-section">
        <h3>{t('servers.admin.dev.list', { count: data.invites.length })}</h3>
        {data.invites.length === 0 ? (
          <p className="hint">{t('servers.admin.dev.none')}</p>
        ) : (
          <div className="card admin-items">
            {data.invites.map((a) => (
              <div key={a.uuid} className="setting">
                <div className="text">
                  <b>{a.name || a.uuid}</b>
                  <span>{t('servers.admin.dev.invitedBy', { when: ago(a.addedAt), by: a.addedBy || '—' })}</span>
                  <div className="hint mono">{a.uuid}</div>
                </div>
                <button className="btn sm" onClick={() => void act('/api/devs/remove', { uuid: a.uuid }, 'servers.admin.dev.uninvited')}>
                  {t('servers.admin.dev.uninvite')}
                </button>
              </div>
            ))}
          </div>
        )}
      </div>

      {removing && b && (
        <ConfirmModal
          title={t('servers.admin.dev.removeBuild')}
          body={t('servers.admin.dev.removeBuildConfirm', { version: b.version })}
          confirm={t('servers.admin.dev.removeBuild')}
          onConfirm={async () => void (await act('/api/devs/build/remove', {}, 'servers.admin.dev.buildRemoved'))}
          onClose={() => setRemoving(false)}
        />
      )}
    </>
  )
}
