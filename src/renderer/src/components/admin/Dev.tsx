// Admin → Dev: the relay's two invite-only update channels, each with its own list and its own build (relay-server's
// devbuilds.rs). Neither gives the other:
// - Developers (only an admin adds them) get the Dev build, what they install in their own launcher.
// - Testers (a developer or an admin invites them) get the Test build.
// Builds are uploaded here, on the relay's Dev page, or by release-dev.bat. Admins use it over the admin connection;
// developers get only this screen, over their relay session, and may not add or remove developers.
import { useState } from 'react'
import { Code2, FileUp, FlaskConical, Loader2, TestTube2, Trash2, Upload, UserPlus } from 'lucide-react'
import type { DevBuildFile } from '@shared/types'
import { api } from '../../api'
import { useStore } from '../../store'
import { t, type Key } from '../../i18n'
import { ConfirmModal, Segmented } from '../ui'
import { ago, Loading, size, useAct, useAdmin, type Caller } from './common'

const TARGETS = ['windows-x86_64', 'windows-aarch64', 'darwin-aarch64', 'darwin-x86_64', 'linux-x86_64', 'linux-aarch64']

type Lane = 'dev' | 'test'
type List = 'developers' | 'testers'

interface DevInvite {
  uuid: string
  name: string
  addedAt: number
  addedBy: string
  developer?: boolean
}
interface Build {
  version: string
  notes: string
  uploadedAt: number
  platforms: { target: string; size: number; uploadedAt: number }[]
}
interface DevData {
  developers?: DevInvite[]
  testers?: DevInvite[]
  /** A relay from before the two lists: everyone in one list, developers flagged. */
  invites?: DevInvite[]
  build: Build | null
  testBuild?: Build | null
  /** `[dev_builds] upload_tokens` is set, so release-dev.bat can upload. */
  uploads: boolean
  /** A developer's own account (their calls only). */
  you?: { uuid: string; name: string }
}

const devCall: Caller = (method, path, body) => api.server.devCall(method, path, body)

const LANE_ICON = { dev: <Code2 className="inline" />, test: <TestTube2 className="inline" /> }

export function DevAdmin({ developer = false }: { developer?: boolean }) {
  const caller = developer ? devCall : undefined
  const { data, error, reload } = useAdmin<DevData>('/api/devs', undefined, caller)
  const act = useAct(reload, caller)
  const [removing, setRemoving] = useState<Lane | null>(null)
  const { toast } = useStore()
  const [file, setFile] = useState<DevBuildFile | null>(null)
  const [lane, setLane] = useState<Lane>('dev')
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
      await api.server.devUpload({ path: file.path, version: version.trim(), target, notes: notes.trim(), developer, lane })
      toast(t(lane === 'test' ? 'servers.admin.dev.uploadDoneTest' : 'servers.admin.dev.uploadDone', { version: version.trim() }), 'success')
      setFile(null)
      setNotes('')
      await reload()
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setUploading(false)
    }
  }
  if (!data) return <Loading error={error} />
  // A relay from before the split sends only `invites`.
  const developers = data.developers ?? (data.invites ?? []).filter((i) => i.developer)
  const testers = data.testers ?? (data.invites ?? []).filter((i) => !i.developer)
  const split = !!data.testers
  const builds: { lane: Lane; build: Build | null | undefined }[] = [
    { lane: 'dev', build: data.build },
    { lane: 'test', build: split ? (data.testBuild ?? null) : undefined }
  ]
  const removingBuild = removing && (removing === 'dev' ? data.build : data.testBuild)
  return (
    <>
      <p className="hint">{t('servers.admin.dev.leadLanes')}</p>
      <p className="hint">{t(developer ? 'servers.admin.dev.leadDeveloper' : 'servers.admin.dev.roles')}</p>
      {!split && <p className="hint bad">{t('servers.admin.dev.oldRelay')}</p>}

      <div className="card" style={{ marginBottom: 18 }}>
        {builds
          .filter((x) => x.build !== undefined)
          .map(({ lane: l, build: b }) => (
            <div key={l} className="setting">
              <div className="text">
                <b>
                  {LANE_ICON[l]}{' '}
                  {b
                    ? t(l === 'test' ? 'servers.admin.dev.testBuild' : 'servers.admin.dev.build', { version: b.version })
                    : t(l === 'test' ? 'servers.admin.dev.noTestBuildTitle' : 'servers.admin.dev.noBuildTitle')}
                </b>
                <span>{t(l === 'test' ? 'servers.admin.dev.testBuildFor' : 'servers.admin.dev.devBuildFor')}</span>
                {b ? (
                  <>
                    <div className="hint">
                      {t('servers.admin.dev.uploaded', { when: ago(b.uploadedAt) })} · {b.platforms.map((p) => `${p.target} (${size(p.size)})`).join(', ')}
                    </div>
                    {b.notes && <div className="hint">{b.notes}</div>}
                  </>
                ) : (
                  <div className="hint">{t('servers.admin.dev.noBuild')}</div>
                )}
              </div>
              {b && (
                <button className="btn sm danger" onClick={() => setRemoving(l)}>
                  <Trash2 /> {t('servers.admin.dev.removeBuild')}
                </button>
              )}
            </div>
          ))}
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
                {split && (
                  <div className="field">
                    <span>{t('servers.admin.dev.uploadAs')}</span>
                    <Segmented
                      value={lane}
                      onChange={setLane}
                      options={[
                        { value: 'dev', label: t('servers.admin.dev.laneDev') },
                        { value: 'test', label: t('servers.admin.dev.laneTest') }
                      ]}
                    />
                  </div>
                )}
                <div className="field-row">
                  <label className="field">
                    <span>{t('servers.admin.dev.version')}</span>
                    <input
                      className="input"
                      value={version}
                      maxLength={64}
                      spellCheck={false}
                      placeholder={lane === 'test' ? '2.0.1-test.1' : '2.0.1-dev.1'}
                      onChange={(e) => setVersion(e.target.value)}
                    />
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
                  <span>{t(lane === 'test' ? 'servers.admin.dev.notesTest' : 'servers.admin.dev.notes')}</span>
                  <input className="input" value={notes} maxLength={4000} onChange={(e) => setNotes(e.target.value)} />
                </label>
                <div className="row end">
                  <button className="btn sm primary" disabled={uploading || !file.signature || !preRelease} onClick={() => void upload()}>
                    {uploading ? <Loader2 className="spin" /> : <Upload />}{' '}
                    {t(uploading ? 'servers.admin.dev.uploading' : lane === 'test' ? 'servers.admin.dev.uploadTestButton' : 'servers.admin.dev.uploadButton')}
                  </button>
                </div>
              </div>
            </div>
          )}
        </div>
        {!data.uploads && !developer && <p className="hint">{t('servers.admin.dev.uploadsOff')}</p>}
      </div>

      <InviteList
        list="developers"
        people={developers}
        canEdit={!developer}
        act={act}
        title="servers.admin.dev.developersList"
        lead="servers.admin.dev.developersLead"
        none="servers.admin.dev.noDevelopers"
        split={split}
      />
      <InviteList
        list="testers"
        people={testers}
        canEdit
        act={act}
        title="servers.admin.dev.testersList"
        lead="servers.admin.dev.testersLead"
        none="servers.admin.dev.none"
        split={split}
      />

      {removing && removingBuild && (
        <ConfirmModal
          title={t('servers.admin.dev.removeBuild')}
          body={t(removing === 'test' ? 'servers.admin.dev.removeTestBuildConfirm' : 'servers.admin.dev.removeBuildConfirm', { version: removingBuild.version })}
          confirm={t('servers.admin.dev.removeBuild')}
          onConfirm={async () =>
            void (await act('/api/devs/build/remove', { lane: removing }, removing === 'test' ? 'servers.admin.dev.testBuildRemoved' : 'servers.admin.dev.buildRemoved'))
          }
          onClose={() => setRemoving(null)}
        />
      )}
    </>
  )
}

/** One of the two lists: who's on it, and (when `canEdit`) a form to add someone and a button to take them off. */
function InviteList({
  list,
  people,
  canEdit,
  act,
  title,
  lead,
  none,
  split
}: {
  list: List
  people: DevInvite[]
  canEdit: boolean
  act: ReturnType<typeof useAct>
  title: Key
  lead: Key
  none: Key
  split: boolean
}) {
  const [who, setWho] = useState('')
  const add = async () => {
    const name = who.trim()
    // An older relay knows only one list, with developers flagged.
    const body = split ? { who: name, list } : { who: name, developer: list === 'developers' }
    if (await act('/api/devs/add', body, list === 'developers' ? 'servers.admin.dev.addedDeveloper' : 'servers.admin.dev.invited', { name })) setWho('')
  }
  return (
    <div className="settings-section">
      <h3>
        {list === 'developers' ? <Code2 className="inline" /> : <FlaskConical className="inline" />} {t(title, { count: people.length })}
      </h3>
      <p className="hint">{t(lead)}</p>
      {canEdit && (
        <form
          className="card"
          style={{ marginBottom: 10 }}
          onSubmit={(e) => {
            e.preventDefault()
            if (who.trim()) void add()
          }}
        >
          <div className="setting">
            <label className="field" style={{ flex: 1 }}>
              <span>{t('servers.admin.dev.inviteHint')}</span>
              <input className="input" value={who} maxLength={36} spellCheck={false} placeholder="Notch / 069a79f4-44e9-…" onChange={(e) => setWho(e.target.value)} />
            </label>
            <button className="btn sm primary" type="submit" disabled={!who.trim()}>
              <UserPlus /> {t(list === 'developers' ? 'servers.admin.dev.addDeveloper' : 'servers.admin.dev.inviteButton')}
            </button>
          </div>
        </form>
      )}
      {people.length === 0 ? (
        <p className="hint">{t(none)}</p>
      ) : (
        <div className="card admin-items">
          {people.map((a) => (
            <div key={a.uuid} className="setting">
              <div className="text">
                <b>{a.name || a.uuid}</b>
                <span>{t('servers.admin.dev.invitedBy', { when: ago(a.addedAt), by: a.addedBy || '—' })}</span>
                <div className="hint mono">{a.uuid}</div>
              </div>
              {canEdit && (
                <button className="btn sm" onClick={() => void act('/api/devs/remove', split ? { uuid: a.uuid, list } : { uuid: a.uuid }, 'servers.admin.dev.uninvited')}>
                  {t(list === 'developers' ? 'servers.admin.dev.removeDeveloper' : 'servers.admin.dev.uninvite')}
                </button>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
