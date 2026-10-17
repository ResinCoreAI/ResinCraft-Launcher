import { ChevronRight, Compass, Home as HomeIcon, Layers, Settings as SettingsIcon, Users } from 'lucide-react'
import type { ReactNode } from 'react'
import { Avatar, BlockArt, Dock, Logo } from './components/ui'
import DropImport from './components/DropImport'
import WindowControls from './components/WindowControls'
import { isTauri } from './api'
import NewInstanceModal from './components/NewInstanceModal'
import Accounts from './pages/Accounts'
import Discover from './pages/Discover'
import Home from './pages/Home'
import InstancePage from './pages/InstancePage'
import Library from './pages/Library'
import SettingsPage from './pages/SettingsPage'
import { useStore, type Route } from './store'
import { t, type Key } from './i18n'

const NAV: { page: Route['page']; label: Key; icon: ReactNode }[] = [
  { page: 'home', label: 'nav.home', icon: <HomeIcon /> },
  { page: 'library', label: 'nav.library', icon: <Layers /> },
  { page: 'discover', label: 'nav.discover', icon: <Compass /> },
  { page: 'accounts', label: 'nav.accounts', icon: <Users /> },
  { page: 'settings', label: 'nav.settings', icon: <SettingsIcon /> }
]

const TITLES: Record<Route['page'], Key> = {
  home: 'nav.home',
  library: 'nav.library',
  instance: 'nav.library',
  discover: 'nav.discover',
  accounts: 'nav.accounts',
  settings: 'nav.settings'
}

export default function App() {
  const { route, go, instances, games, activeAccount, newInstanceOpen, setNewInstanceOpen } = useStore()
  const current = route.page === 'instance' ? 'library' : route.page
  const instance = route.page === 'instance' ? instances.find((i) => i.id === route.id) : undefined
  const recent = instances.filter((i) => i.lastPlayed || games[i.id]).slice(0, 6)

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="brand" data-tauri-drag-region>
          <Logo />
          <div className="brand-name">
            ResinCraft
            <small>{t('app.tagline')}</small>
          </div>
        </div>

        <nav className="nav">
          {NAV.map((n) => (
            <button key={n.page} className={`nav-item ${current === n.page ? 'active' : ''}`} onClick={() => go({ page: n.page } as Route)}>
              {n.icon}
              <span className="ellipsis">{t(n.label)}</span>
              {n.page === 'library' && instances.length > 0 && <span className="count">{instances.length}</span>}
            </button>
          ))}
        </nav>

        <div className="nav-label">{t('app.jumpBackIn')}</div>
        <div className="nav nav-recent" style={{ marginTop: 0 }}>
          {recent.length === 0 && (
            <div className="hint" style={{ padding: '0 12px' }}>
              {t('app.recentEmpty')}
            </div>
          )}
          {recent.map((i) => (
            <button
              key={i.id}
              className={`nav-item ${route.page === 'instance' && route.id === i.id ? 'active' : ''}`}
              onClick={() => go({ page: 'instance', id: i.id })}
            >
              <span className="nav-dot">{i.iconUrl ? <img src={i.iconUrl} alt="" /> : <BlockArt seed={i.id} color={i.color} />}</span>
              <span className="ellipsis">{i.name}</span>
              {games[i.id] === 'running' && <span className="live" title={t('app.running')} />}
            </button>
          ))}
        </div>

        <button className="account-chip" onClick={() => go({ page: 'accounts' })}>
          <Avatar account={activeAccount} size={34} />
          <div className="who">
            <b>{activeAccount?.username ?? t('app.notSignedIn')}</b>
            <span>{t(activeAccount ? (activeAccount.type === 'msa' ? 'app.microsoftAccount' : 'app.offlineProfile') : 'app.clickToSignIn')}</span>
          </div>
        </button>
      </aside>

      <main className="main">
        <header className="topbar" data-tauri-drag-region>
          <div className="crumb">
            {instance ? (
              <>
                <button onClick={() => go({ page: 'library' })}>{t('nav.library')}</button>
                <ChevronRight size={14} />
                <b>{instance.name}</b>
              </>
            ) : (
              <b>{t(TITLES[route.page])}</b>
            )}
          </div>
        </header>
        <div className="content">
          {route.page === 'home' && <Home />}
          {route.page === 'library' && <Library />}
          {route.page === 'instance' && <InstancePage key={route.id} id={route.id} tab={route.tab} />}
          {route.page === 'discover' && (
            <Discover key={`${route.instanceId ?? ''}${route.kind ?? ''}`} initialKind={route.kind} instanceId={route.instanceId} />
          )}
          {route.page === 'accounts' && <Accounts />}
          {route.page === 'settings' && <SettingsPage />}
        </div>
      </main>

      {newInstanceOpen && <NewInstanceModal onClose={() => setNewInstanceOpen(false)} />}
      <Dock />
      <DropImport />
      {isTauri && <WindowControls />}
    </div>
  )
}
