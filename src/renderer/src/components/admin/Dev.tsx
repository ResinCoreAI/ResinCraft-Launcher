// Admin → Dev: who may use the launcher's Dev update channel, and the Dev build (uploaded here, on the relay's Dev
// page or by release-dev.bat; relay-server's devbuilds.rs).
import { useState } from 'react'
import { FileUp, FlaskConical, Loader2, Trash2, Upload, UserPlus } from 'lucide-react'
import type { DevBuildFile } from '@shared/types'
import { api } from '../../api'
import { useStore } from '../../store'
import { t } from '../../i18n'
import { ConfirmModal } from '../ui'
import { ago, Loading, size, useAct, useAdmin } from './common'

const TARGETS = ['windows-x86_64', 'windows-aarch64', 'darwin-aarch64', 'darwin-x86_64', 'linux-x86_64', 'linux-aarch64']

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
  const { toast } = useStore()
  const [file, setFile] = useState<DevBuildFile | null>(null)
  const [version, setVersion] = useState('')
  const [target, setTarget] = useState('windows-x86_64')
  const [notes, setNotes] = useState('')
  const [uploading, setUploading] = useState(false)
  const pick = async () => {
    const f = await api.server.devPickBuild().catch((e) => void toast((e as Error).message, 'error'))
    if (!f) return
    setFile(f)
    setVersion(f.version)
    setTarget(f.target)
  }
  const preRelease = /^\d+\.\d+\.\d+-[0-9A-Za-z.-]+$/.test(version.trim())
  const upload = async () => {
    if (!file) return
    setUploading(true)
    try {
      await api.server.devUpload({ path: file.path, version: version.trim(), target, notes: notes.trim() })
      toast(t('servers.admin.dev.uploadDone', { version: version.trim() }), 'success')
      setFile(null)
      setNotes('')
      await reload()
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setUploading(false)
    }
  }
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
              <span>{t('servers.admin.dev.noBuild')}</span>
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
        <h3>{t('servers.admin.dev.upload')}</h3>
        <p className="hint">{t('servers.admin.dev.uploadHint')}</p>
        <div className="card">
          <div className="setting">
            <div className="text">
              <b>{file ? file.name : t('servers.admin.dev.noFile')}</b>
              {file && <span>{size(file.size)}</span>}
              {file && !file.signature && <div className="hint bad">{t('servers.admin.dev.noSig', { name: file.name })}</div>}
            </div>
            <button className="btn sm" disabled={uploading} onClick={() => void pick()}>
              <FileUp /> {t('servers.admin.dev.choose')}
            </button>
          </div>
          {file && (
            <div className="setting stack">
              <div className="listing-fields">
                <div className="field-row">
                  <label className="field">
                    <span>{t('servers.admin.dev.version')}</span>
                    <input className="input" value={version} maxLength={64} spellCheck={false} placeholder="1.7.0-dev.1" onChange={(e) => setVersion(e.target.value)} />
                    {!preRelease && <span className="hint">{t('servers.admin.dev.versionHint')}</span>}
                  </label>
                  <label className="field">
                    <span>{t('servers.admin.dev.target')}</span>
                    <select className="input" value={target} onChange={(e) => setTarget(e.target.value)}>
                      {TARGETS.map((x) => (
                        <option key={x} value={x}>
                          {x}
                        </option>
                      ))}
                    </select>
                  </label>
                </div>
                <label className="field">
                  <span>{t('servers.admin.dev.notes')}</span>
                  <input className="input" value={notes} maxLength={4000} onChange={(e) => setNotes(e.target.value)} />
                </label>
                <div className="row end">
                  <button className="btn sm primary" disabled={uploading || !file.signature || !preRelease} onClick={() => void upload()}>
                    {uploading ? <Loader2 className="spin" /> : <Upload />} {t(uploading ? 'servers.admin.dev.uploading' : 'servers.admin.dev.uploadButton')}
                  </button>
                </div>
              </div>
            </div>
          )}
        </div>
        {!data.uploads && <p className="hint">{t('servers.admin.dev.uploadsOff')}</p>}
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
