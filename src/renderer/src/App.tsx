import { ChevronRight, Compass, Home as HomeIcon, Layers, Settings as SettingsIcon, Users } from 'lucide-react'
import type { ReactNode } from 'react'
import { Avatar, BlockArt, Dock, Logo } from './components/ui'
import DropImport from './components/DropImport'
import NewInstanceModal from './components/NewInstanceModal'
import Accounts from './pages/Accounts'
import Discover from './pages/Discover'
import Home from './pages/Home'
import InstancePage from './pages/InstancePage'
import Library from './pages/Library'
import SettingsPage from './pages/SettingsPage'
import { useStore, type Route } from './store'

const NAV: { page: Route['page']; label: string; icon: ReactNode }[] = [
  { page: 'home', label: 'Home', icon: <HomeIcon /> },
  { page: 'library', label: 'Library', icon: <Layers /> },
  { page: 'discover', label: 'Discover', icon: <Compass /> },
  { page: 'accounts', label: 'Accounts', icon: <Users /> },
  { page: 'settings', label: 'Settings', icon: <SettingsIcon /> }
]

const TITLES: Record<Route['page'], string> = {
  home: 'Home',
  library: 'Library',
  instance: 'Library',
  discover: 'Discover',
  accounts: 'Accounts',
  settings: 'Settings'
}

export default function App() {
  const { route, go, instances, games, activeAccount, newInstanceOpen, setNewInstanceOpen } = useStore()
  const current = route.page === 'instance' ? 'library' : route.page
  const instance = route.page === 'instance' ? instances.find((i) => i.id === route.id) : undefined
  const recent = instances.filter((i) => i.lastPlayed || games[i.id]).slice(0, 6)

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="brand">
          <Logo />
          <div className="brand-name">
            ResinCraft
            <small>Launcher by Puirkle</small>
          </div>
        </div>

        <nav className="nav">
          {NAV.map((n) => (
            <button key={n.page} className={`nav-item ${current === n.page ? 'active' : ''}`} onClick={() => go({ page: n.page } as Route)}>
              {n.icon}
              {n.label}
              {n.page === 'library' && instances.length > 0 && <span className="count">{instances.length}</span>}
            </button>
          ))}
        </nav>

        <div className="nav-label">Jump back in</div>
        <div className="nav nav-recent" style={{ marginTop: 0 }}>
          {recent.length === 0 && (
            <div className="hint" style={{ padding: '0 12px' }}>
              Instances you play show up here.
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
              {games[i.id] === 'running' && <span className="live" title="Running" />}
            </button>
          ))}
        </div>

        <button className="account-chip" onClick={() => go({ page: 'accounts' })}>
          <Avatar account={activeAccount} size={34} />
          <div className="who">
            <b>{activeAccount?.username ?? 'Not signed in'}</b>
            <span>{activeAccount ? (activeAccount.type === 'msa' ? 'Microsoft account' : 'Offline profile') : 'Click to sign in'}</span>
          </div>
        </button>
      </aside>

      <main className="main">
        <header className="topbar">
          <div className="crumb">
            {instance ? (
              <>
                <button onClick={() => go({ page: 'library' })}>Library</button>
                <ChevronRight size={14} />
                <b>{instance.name}</b>
              </>
            ) : (
              <b>{TITLES[route.page]}</b>
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
    </div>
  )
}
