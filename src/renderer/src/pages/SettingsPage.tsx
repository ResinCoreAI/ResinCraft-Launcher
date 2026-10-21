import { useEffect, useState } from 'react'
import { ArrowLeftRight, Bug, Coffee, Download, FolderSearch, Gamepad2, HardDrive, Info, Monitor, Moon, Palette, RefreshCw, Sun } from 'lucide-react'
import type { JavaInstall, ModSourcePreference, Settings } from '@shared/types'
import { api } from '../api'
import { Logo, Segmented, Switch } from '../components/ui'
import { StorageSection } from '../components/Storage'
import { BackupSection } from '../components/Backup'
import { UpdatesSection } from '../components/Updates'
import { PackUpdatesSetting } from '../components/PackUpdates'
import { useStore } from '../store'
import { LANGUAGES, matchLanguage, t, type Key } from '../i18n'
import { fixed } from '../util'

/** A new GitHub issue, with the version and system filled in (nothing is sent until the player submits it there). */
function issueUrl(version?: string, platform?: string): string {
  const body = [
    '**What happened?**',
    '',
    '',
    '**What did you expect to happen?**',
    '',
    '',
    '**Steps to make it happen again**',
    '1. ',
    '',
    '---',
    `ResinCraft Launcher ${version ? `v${version}` : '(version unknown)'} · ${platform ?? navigator.userAgent}`
  ].join('\n')
  return `https://github.com/ResinCoreAI/ResinCraft-Launcher/issues/new?body=${encodeURIComponent(body)}`
}

/** Mojang's usage guidelines ask for this line word for word, so it isn't translated. */
const DISCLAIMER = 'NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.'

const SECTIONS: { id: string; label: Key; icon: React.ReactNode }[] = [
  { id: 'appearance', label: 'settings.nav.appearance', icon: <Palette size={16} /> },
  { id: 'game', label: 'settings.nav.game', icon: <Gamepad2 size={16} /> },
  { id: 'java', label: 'settings.nav.java', icon: <Coffee size={16} /> },
  { id: 'downloads', label: 'settings.nav.downloads', icon: <Download size={16} /> },
  { id: 'storage', label: 'storage.nav', icon: <HardDrive size={16} /> },
  { id: 'backup', label: 'backup.nav', icon: <ArrowLeftRight size={16} /> },
  { id: 'updates', label: 'updates.nav', icon: <RefreshCw size={16} /> },
  { id: 'about', label: 'settings.nav.about', icon: <Info size={16} /> }
]

export default function SettingsPage() {
  const { settings: s, updateSettings, info, toast } = useStore()
  const [javas, setJavas] = useState<JavaInstall[]>()
  const [scanning, setScanning] = useState(false)
  const [mem, setMem] = useState(s?.memoryMb ?? 4096)
  const [jvm, setJvm] = useState(s?.jvmArgs ?? '')
  const [size, setSize] = useState({ w: s?.gameWidth ?? 1280, h: s?.gameHeight ?? 720 })

  const scan = async () => {
    setScanning(true)
    setJavas(await api.java.list().finally(() => setScanning(false)))
  }
  useEffect(() => {
    void scan()
  }, [])

  if (!s) return null
  const set = (p: Partial<Settings>) => void updateSettings(p)
  const totalMem = info?.totalMemMb ?? 8192
  const maxMem = Math.max(4096, Math.floor((totalMem * 0.75) / 512) * 512)
  const systemLanguage = matchLanguage(navigator.languages ?? [navigator.language])

  return (
    <div className="page">
      <div className="page-head">
        <div>
          <h1>{t('settings.title')}</h1>
          <p>{t('settings.subtitle')}</p>
        </div>
      </div>

      <div className="settings-grid">
        <nav className="settings-nav">
          {SECTIONS.map((x) => (
            <button key={x.id} className="nav-item" onClick={() => document.getElementById(x.id)?.scrollIntoView({ behavior: 'smooth' })}>
              {x.icon} <span className="ellipsis">{t(x.label)}</span>
            </button>
          ))}
        </nav>

        <div>
          <section className="settings-section" id="appearance">
            <h3>{t('settings.nav.appearance')}</h3>
            <p>{t('settings.appearance.lead')}</p>
            <div className="card">
              <div className="setting">
                <div className="text">
                  <b>{t('settings.theme')}</b>
                  <span>{t('settings.theme.hint')}</span>
                </div>
                <Segmented
                  value={s.theme}
                  onChange={(theme) => set({ theme })}
                  options={[
                    {
                      value: 'dark',
                      label: (
                        <>
                          <Moon size={14} /> {t('settings.theme.dark')}
                        </>
                      )
                    },
                    {
                      value: 'light',
                      label: (
                        <>
                          <Sun size={14} /> {t('settings.theme.light')}
                        </>
                      )
                    },
                    {
                      value: 'system',
                      label: (
                        <>
                          <Monitor size={14} /> {t('settings.theme.system')}
                        </>
                      )
                    }
                  ]}
                />
              </div>
              <div className="setting">
                <div className="text">
                  <b>{t('settings.language')}</b>
                  <span>{t('settings.language.hint')}</span>
                </div>
                <select className="select" style={{ width: 220 }} value={s.language || 'system'} onChange={(e) => set({ language: e.target.value })}>
                  <option value="system">
                    {t('settings.language.system', { name: LANGUAGES.find((l) => l.code === systemLanguage)?.name ?? 'English' })}
                  </option>
                  {LANGUAGES.map((l) => (
                    <option key={l.code} value={l.code} lang={l.code}>
                      {l.name}
                    </option>
                  ))}
                </select>
              </div>
            </div>
          </section>

          <section className="settings-section" id="game">
            <h3>{t('settings.nav.game')}</h3>
            <p>{t('settings.game.lead')}</p>
            <div className="card">
              <div className="setting stack">
                <div className="row">
                  <div className="text grow">
                    <b>{t('settings.memory')}</b>
                    <span>{t('settings.memory.hint', { total: Math.round(totalMem / 1024) })}</span>
                  </div>
                  <span className="mem-value">{t('common.size.gb', { n: fixed(mem / 1024, 1) })}</span>
                </div>
                <div>
                  <input
                    type="range"
                    min={1024}
                    max={maxMem}
                    step={512}
                    value={mem}
                    onChange={(e) => setMem(Number(e.target.value))}
                    onMouseUp={() => set({ memoryMb: mem })}
                    onKeyUp={() => set({ memoryMb: mem })}
                  />
                  <div className="mem-scale">
                    <span>{t('common.size.gb', { n: 1 })}</span>
                    <span>{t('common.size.gb', { n: Math.round(maxMem / 1024) })}</span>
                  </div>
                </div>
              </div>
              <div className="setting">
                <div className="text">
                  <b>{t('settings.windowSize')}</b>
                  <span>{t('settings.windowSize.hint')}</span>
                </div>
                <div className="row" style={{ width: 220 }}>
                  <input
                    className="input"
                    type="number"
                    min={640}
                    value={size.w}
                    onChange={(e) => setSize({ ...size, w: Number(e.target.value) })}
                    onBlur={() => set({ gameWidth: size.w })}
                    aria-label={t('settings.width')}
                  />
                  <span className="faint">×</span>
                  <input
                    className="input"
                    type="number"
                    min={480}
                    value={size.h}
                    onChange={(e) => setSize({ ...size, h: Number(e.target.value) })}
                    onBlur={() => set({ gameHeight: size.h })}
                    aria-label={t('settings.height')}
                  />
                </div>
              </div>
              <div className="setting">
                <div className="text">
                  <b>{t('settings.onLaunch')}</b>
                  <span>{t('settings.onLaunch.hint')}</span>
                </div>
                <select
                  className="select"
                  style={{ width: 220 }}
                  value={s.onLaunch}
                  onChange={(e) => set({ onLaunch: e.target.value as Settings['onLaunch'] })}
                >
                  <option value="minimize">{t('settings.onLaunch.minimize')}</option>
                  <option value="hide">{t('settings.onLaunch.hide')}</option>
                  <option value="keep">{t('settings.onLaunch.keep')}</option>
                </select>
              </div>
              <div className="setting">
                <div className="text">
                  <b>{t('settings.snapshots')}</b>
                  <span>{t('settings.snapshots.hint')}</span>
                </div>
                <Switch on={s.showSnapshots} onChange={(v) => set({ showSnapshots: v })} label={t('settings.snapshots')} />
              </div>
            </div>
          </section>

          <section className="settings-section" id="java">
            <h3>{t('settings.nav.java')}</h3>
            <p>{t('settings.java.lead')}</p>
            <div className="card">
              <div className="setting">
                <div className="text">
                  <b>{t('settings.autoJava')}</b>
                  <span>{t('settings.autoJava.hint')}</span>
                </div>
                <Switch on={s.autoJava} onChange={(v) => set({ autoJava: v })} label={t('settings.autoJava')} />
              </div>
              <div className="setting stack">
                <div className="row">
                  <div className="text grow">
                    <b>{t('settings.javaToUse')}</b>
                    <span>{t('settings.javaToUse.hint')}</span>
                  </div>
                  <button className="btn sm" onClick={scan} disabled={scanning}>
                    <RefreshCw className={scanning ? 'spin' : ''} /> {t('settings.rescan')}
                  </button>
                  <button
                    className="btn sm"
                    onClick={async () => {
                      const p = await api.java.browse()
                      if (p) set({ javaPath: p })
                    }}
                  >
                    <FolderSearch /> {t('common.browse')}
                  </button>
                </div>
                <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
                  <div className={`java-row ${!s.javaPath ? 'on' : ''}`} onClick={() => set({ javaPath: '' })}>
                    <span className="radio" />
                    <div className="grow">
                      <b>{t('settings.javaAuto')}</b>{' '}
                      <span className="chip accent" style={{ marginLeft: 6 }}>
                        {t('common.recommended')}
                      </span>
                      <div className="hint">{t('settings.javaAuto.hint')}</div>
                    </div>
                  </div>
                  {javas?.map((j) => (
                    <div key={j.path} className={`java-row ${s.javaPath === j.path ? 'on' : ''}`} onClick={() => set({ javaPath: j.path })}>
                      <span className="radio" />
                      <div className="grow" style={{ minWidth: 0 }}>
                        <b>{t('settings.javaVersion', { major: j.major })}</b> <span className="faint">{j.version}</span>
                        {j.managed && (
                          <span className="chip" style={{ marginLeft: 6 }}>
                            {t('settings.javaManaged')}
                          </span>
                        )}
                        <div className="hint ellipsis mono">{j.path}</div>
                      </div>
                    </div>
                  ))}
                  {s.javaPath && !javas?.some((j) => j.path === s.javaPath) && (
                    <div className="java-row on">
                      <span className="radio" />
                      <div className="grow hint mono ellipsis">{s.javaPath}</div>
                    </div>
                  )}
                  {javas?.length === 0 && <div className="hint">{t('settings.noJava')}</div>}
                </div>
              </div>
              <div className="setting stack">
                <div className="text">
                  <b>{t('settings.jvmArgs')}</b>
                  <span>{t('settings.jvmArgs.hint')}</span>
                </div>
                <textarea
                  className="input"
                  rows={2}
                  value={jvm}
                  onChange={(e) => setJvm(e.target.value)}
                  onBlur={() => set({ jvmArgs: jvm })}
                  placeholder="-XX:+UseG1GC -XX:MaxGCPauseMillis=50"
                />
              </div>
            </div>
          </section>

          <section className="settings-section" id="downloads">
            <h3>{t('settings.nav.downloads')}</h3>
            <p>{t('settings.downloads.lead')}</p>
            <div className="card">
              <div className="setting">
                <div className="text">
                  <b>{t('settings.parallel')}</b>
                  <span>{t('settings.parallel.hint')}</span>
                </div>
                <select className="select" style={{ width: 120 }} value={s.concurrency} onChange={(e) => set({ concurrency: Number(e.target.value) })}>
                  {[4, 8, 16, 24, 32].map((n) => (
                    <option key={n} value={n}>
                      {n}
                    </option>
                  ))}
                </select>
              </div>
              <div className="setting">
                <div className="text">
                  <b>{t('settings.preferredSource')}</b>
                  <span>{t('settings.preferredSource.hint')}</span>
                  {s.preferredModSource && s.preferredModSource !== 'ask' && (
                    <button
                      className="link"
                      style={{ alignSelf: 'flex-start' }}
                      onClick={() => {
                        set({ preferredModSource: 'ask' })
                        toast(t('settings.preferredSource.resetDone'), 'info')
                      }}
                    >
                      {t('settings.preferredSource.reset')}
                    </button>
                  )}
                </div>
                <select
                  className="select"
                  style={{ width: 220 }}
                  value={s.preferredModSource ?? 'ask'}
                  onChange={(e) => set({ preferredModSource: e.target.value as ModSourcePreference })}
                >
                  <option value="ask">{t('settings.preferredSource.ask')}</option>
                  <option value="modrinth">Modrinth</option>
                  <option value="curseforge">CurseForge</option>
                </select>
              </div>
            </div>
          </section>

          <StorageSection />

          <BackupSection />

          <UpdatesSection />

          <section className="settings-section" id="pack-updates">
            <h3>{t('packUpdates.section')}</h3>
            <div className="card">
              <PackUpdatesSetting />
            </div>
          </section>

          <section className="settings-section" id="about">
            <h3>{t('settings.nav.about')}</h3>
            <div className="card pad row" style={{ gap: 16 }}>
              <Logo className="about-logo" />
              <div className="grow">
                <b style={{ fontSize: 16 }}>ResinCraft Launcher</b> <span className="faint">v{info?.version}</span>
                <div className="muted">{t('settings.about.credit')}</div>
                <div className="hint mono ellipsis" style={{ marginTop: 6 }}>
                  {t('settings.about.dataFolder', { path: info?.dataDir ?? '' })}
                </div>
                <div className="hint" lang="en" style={{ marginTop: 6 }}>
                  {DISCLAIMER}
                </div>
              </div>
              <button className="btn" style={{ alignSelf: 'flex-start' }} onClick={() => void api.app.openExternal(issueUrl(info?.version, info?.platform))}>
                <Bug /> {t('settings.about.report')}
              </button>
            </div>
          </section>
        </div>
      </div>
    </div>
  )
}
