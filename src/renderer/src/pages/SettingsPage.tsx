import { useEffect, useState } from 'react'
import { Coffee, Flame, Download, FolderSearch, Gamepad2, Info, KeyRound, Monitor, Moon, Palette, RefreshCw, Sun } from 'lucide-react'
import type { JavaInstall, Settings } from '@shared/types'
import { api } from '../api'
import { Logo, Segmented, Switch } from '../components/ui'
import { useStore } from '../store'

const SECTIONS = [
  { id: 'appearance', label: 'Appearance', icon: <Palette size={16} /> },
  { id: 'game', label: 'Game', icon: <Gamepad2 size={16} /> },
  { id: 'java', label: 'Java', icon: <Coffee size={16} /> },
  { id: 'downloads', label: 'Downloads', icon: <Download size={16} /> },
  { id: 'microsoft', label: 'Microsoft sign-in', icon: <KeyRound size={16} /> },
  { id: 'curseforge', label: 'CurseForge', icon: <Flame size={16} /> },
  { id: 'about', label: 'About', icon: <Info size={16} /> }
]

export default function SettingsPage() {
  const { settings: s, updateSettings, info } = useStore()
  const [javas, setJavas] = useState<JavaInstall[]>()
  const [scanning, setScanning] = useState(false)
  const [mem, setMem] = useState(s?.memoryMb ?? 4096)
  const [jvm, setJvm] = useState(s?.jvmArgs ?? '')
  const [clientId, setClientId] = useState(s?.msClientId ?? '')
  const [cfKey, setCfKey] = useState(s?.curseforgeKey ?? '')
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

  return (
    <div className="page">
      <div className="page-head">
        <div>
          <h1>Settings</h1>
          <p>Defaults for every instance. Each instance can override these in its own Settings tab.</p>
        </div>
      </div>

      <div className="settings-grid">
        <nav className="settings-nav">
          {SECTIONS.map((x) => (
            <button key={x.id} className="nav-item" onClick={() => document.getElementById(x.id)?.scrollIntoView({ behavior: 'smooth' })}>
              {x.icon} {x.label}
            </button>
          ))}
        </nav>

        <div>
          <section className="settings-section" id="appearance">
            <h3>Appearance</h3>
            <p>Make ResinCraft feel like home.</p>
            <div className="card">
              <div className="setting">
                <div className="text">
                  <b>Theme</b>
                  <span>System follows your OS light/dark setting.</span>
                </div>
                <Segmented
                  value={s.theme}
                  onChange={(theme) => set({ theme })}
                  options={[
                    {
                      value: 'dark',
                      label: (
                        <>
                          <Moon size={14} /> Dark
                        </>
                      )
                    },
                    {
                      value: 'light',
                      label: (
                        <>
                          <Sun size={14} /> Light
                        </>
                      )
                    },
                    {
                      value: 'system',
                      label: (
                        <>
                          <Monitor size={14} /> System
                        </>
                      )
                    }
                  ]}
                />
              </div>
            </div>
          </section>

          <section className="settings-section" id="game">
            <h3>Game</h3>
            <p>How Minecraft starts.</p>
            <div className="card">
              <div className="setting stack">
                <div className="row">
                  <div className="text grow">
                    <b>Memory</b>
                    <span>Most modpacks are happy with 4–6 GB. Your computer has {(totalMem / 1024).toFixed(0)} GB.</span>
                  </div>
                  <span className="mem-value">{(mem / 1024).toFixed(1)} GB</span>
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
                    <span>1 GB</span>
                    <span>{(maxMem / 1024).toFixed(0)} GB</span>
                  </div>
                </div>
              </div>
              <div className="setting">
                <div className="text">
                  <b>Window size</b>
                  <span>Starting resolution of the game window.</span>
                </div>
                <div className="row" style={{ width: 220 }}>
                  <input
                    className="input"
                    type="number"
                    min={640}
                    value={size.w}
                    onChange={(e) => setSize({ ...size, w: Number(e.target.value) })}
                    onBlur={() => set({ gameWidth: size.w })}
                    aria-label="Width"
                  />
                  <span className="faint">×</span>
                  <input
                    className="input"
                    type="number"
                    min={480}
                    value={size.h}
                    onChange={(e) => setSize({ ...size, h: Number(e.target.value) })}
                    onBlur={() => set({ gameHeight: size.h })}
                    aria-label="Height"
                  />
                </div>
              </div>
              <div className="setting">
                <div className="text">
                  <b>When the game starts</b>
                  <span>What the launcher window does while you play.</span>
                </div>
                <select
                  className="select"
                  style={{ width: 220 }}
                  value={s.onLaunch}
                  onChange={(e) => set({ onLaunch: e.target.value as Settings['onLaunch'] })}
                >
                  <option value="minimize">Minimize</option>
                  <option value="hide">Hide until the game closes</option>
                  <option value="keep">Stay open</option>
                </select>
              </div>
              <div className="setting">
                <div className="text">
                  <b>Show snapshots</b>
                  <span>Include test builds when creating instances.</span>
                </div>
                <Switch on={s.showSnapshots} onChange={(v) => set({ showSnapshots: v })} label="Show snapshots" />
              </div>
            </div>
          </section>

          <section className="settings-section" id="java">
            <h3>Java</h3>
            <p>Minecraft runs on Java. ResinCraft picks the right version for each game and can download it for you.</p>
            <div className="card">
              <div className="setting">
                <div className="text">
                  <b>Download Java automatically</b>
                  <span>Installs Eclipse Temurin when no suitable Java is found.</span>
                </div>
                <Switch on={s.autoJava} onChange={(v) => set({ autoJava: v })} label="Download Java automatically" />
              </div>
              <div className="setting stack">
                <div className="row">
                  <div className="text grow">
                    <b>Java to use</b>
                    <span>Automatic is best unless you know you need a specific one.</span>
                  </div>
                  <button className="btn sm" onClick={scan} disabled={scanning}>
                    <RefreshCw className={scanning ? 'spin' : ''} /> Rescan
                  </button>
                  <button
                    className="btn sm"
                    onClick={async () => {
                      const p = await api.java.browse()
                      if (p) set({ javaPath: p })
                    }}
                  >
                    <FolderSearch /> Browse
                  </button>
                </div>
                <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
                  <div className={`java-row ${!s.javaPath ? 'on' : ''}`} onClick={() => set({ javaPath: '' })}>
                    <span className="radio" />
                    <div className="grow">
                      <b>Automatic</b>{' '}
                      <span className="chip accent" style={{ marginLeft: 6 }}>
                        Recommended
                      </span>
                      <div className="hint">Matches Java to each Minecraft version.</div>
                    </div>
                  </div>
                  {javas?.map((j) => (
                    <div key={j.path} className={`java-row ${s.javaPath === j.path ? 'on' : ''}`} onClick={() => set({ javaPath: j.path })}>
                      <span className="radio" />
                      <div className="grow" style={{ minWidth: 0 }}>
                        <b>Java {j.major}</b> <span className="faint">{j.version}</span>
                        {j.managed && (
                          <span className="chip" style={{ marginLeft: 6 }}>
                            Installed by ResinCraft
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
                  {javas?.length === 0 && <div className="hint">No Java found yet. That's fine: ResinCraft will download one when you first play.</div>}
                </div>
              </div>
              <div className="setting stack">
                <div className="text">
                  <b>Default JVM arguments</b>
                  <span>Advanced: extra flags passed to Java for every instance.</span>
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
            <h3>Downloads</h3>
            <p>Lower this if your connection struggles when preparing games.</p>
            <div className="card">
              <div className="setting">
                <div className="text">
                  <b>Parallel downloads</b>
                  <span>Files fetched at the same time.</span>
                </div>
                <select className="select" style={{ width: 120 }} value={s.concurrency} onChange={(e) => set({ concurrency: Number(e.target.value) })}>
                  {[4, 8, 16, 24, 32].map((n) => (
                    <option key={n} value={n}>
                      {n}
                    </option>
                  ))}
                </select>
              </div>
            </div>
          </section>

          <section className="settings-section" id="microsoft">
            <h3>Microsoft sign-in</h3>
            <p>
              ResinCraft signs in through your own Azure app registration. Create one (personal Microsoft accounts, public client flows enabled), then{' '}
              <a href="https://aka.ms/mce-reviewappid" target="_blank" rel="noreferrer">
                ask Mojang to approve it
              </a>{' '}
              for Minecraft. Details are in the README.
            </p>
            <div className="card">
              <div className="setting">
                <div className="text">
                  <b>Azure Client ID</b>
                  <span>Looks like 00000000-0000-0000-0000-000000000000</span>
                </div>
                <div className="control">
                  <input
                    className="input mono"
                    value={clientId}
                    placeholder="Application (client) ID"
                    onChange={(e) => setClientId(e.target.value)}
                    onBlur={() => set({ msClientId: clientId.trim() })}
                  />
                </div>
              </div>
            </div>
          </section>

          <section className="settings-section" id="curseforge">
            <h3>CurseForge</h3>
            <p>
              Searching and downloading from CurseForge needs a free API key. Sign in at{' '}
              <a href="https://console.curseforge.com/" target="_blank" rel="noreferrer">
                console.curseforge.com
              </a>
              , request an API key for your app, and paste it here. Modrinth and FTB work without one.
            </p>
            <div className="card">
              <div className="setting">
                <div className="text">
                  <b>API key</b>
                  <span>{s.curseforgeKey ? 'Saved. CurseForge appears in Discover.' : 'Not set'}</span>
                </div>
                <div className="control">
                  <input
                    className="input mono"
                    type="password"
                    autoComplete="off"
                    value={cfKey}
                    placeholder="$2a$10$…"
                    onChange={(e) => setCfKey(e.target.value)}
                    onBlur={() => set({ curseforgeKey: cfKey.trim() })}
                  />
                </div>
              </div>
            </div>
          </section>

          <section className="settings-section" id="about">
            <h3>About</h3>
            <div className="card pad row" style={{ gap: 16 }}>
              <Logo className="about-logo" />
              <div className="grow">
                <b style={{ fontSize: 16 }}>ResinCraft Launcher</b> <span className="faint">v{info?.version}</span>
                <div className="muted">Made by Puirkle. Its feature set is inspired by Prism Launcher; all code is original.</div>
                <div className="hint mono ellipsis" style={{ marginTop: 6 }}>
                  Data folder: {info?.dataDir}
                </div>
              </div>
            </div>
          </section>
        </div>
      </div>
    </div>
  )
}
