import { ChevronRight, ChevronsUpDown, Compass, Home as HomeIcon, Layers, Server, Settings as SettingsIcon, Smartphone, Users, Zap } from 'lucide-react'
import { lazy, Suspense, type ReactNode } from 'react'
import type { AccountView, Instance } from '@shared/types'
import { ACCOUNT_KIND_SHORT, Avatar, BlockArt, ContextMenuHost, Dock, Logo, openContextMenu, type ContextEntry } from './components/ui'
import DropImport from './components/DropImport'
import WindowControls from './components/WindowControls'
import { UpdateBanner, UpdateTitleButton } from './components/Updates'
import { useRemoteActions } from './components/RemoteControl'
import ThemeDecor, { SidebarPumpkin } from './components/ThemeDecor'
import EasterEgg from './components/EasterEgg'
import CrashFixer from './components/CrashFixer'
import { PackUpdatesBanner } from './components/PackUpdates'
import { RelayWatcher } from './components/ServerRelay'
import { isTauri } from './api'
import NewInstanceModal from './components/NewInstanceModal'
import Accounts from './pages/Accounts'
import Bedrock from './pages/Bedrock'
import Discover from './pages/Discover'
import Home from './pages/Home'
import InstancePage from './pages/InstancePage'
import Library, { useInstanceMenu } from './pages/Library'
import QuickJoin from './pages/QuickJoin'
import SettingsPage from './pages/SettingsPage'
import { useStore, type Route } from './store'
import { t, type Key } from './i18n'
import { hasBedrockApp } from './platform'

// Server hosting is self-contained and fairly large; load it on first visit.
const Servers = lazy(() => import('./pages/Servers'))

const NAV: { page: Route['page']; label: Key; icon: ReactNode }[] = [
  { page: 'home', label: 'nav.home', icon: <HomeIcon /> },
  { page: 'library', label: 'nav.library', icon: <Layers /> },
  { page: 'quickjoin', label: 'quickjoin.nav', icon: <Zap /> },
  { page: 'bedrock', label: 'nav.bedrock', icon: <Smartphone /> },
  { page: 'discover', label: 'nav.discover', icon: <Compass /> },
  { page: 'servers', label: 'servers.nav', icon: <Server /> },
  { page: 'accounts', label: 'nav.accounts', icon: <Users /> },
  { page: 'settings', label: 'nav.settings', icon: <SettingsIcon /> }
]

/** The sidebar's line under the account name. */
const ACCOUNT_KIND: Record<AccountView['type'], Key> = { msa: 'app.microsoftAccount', bedrock: 'app.bedrockProfile', offline: 'app.offlineProfile' }

const TITLES: Record<Route['page'], Key> = {
  home: 'nav.home',
  library: 'nav.library',
  quickjoin: 'quickjoin.nav',
  bedrock: 'nav.bedrock',
  instance: 'nav.library',
  discover: 'nav.discover',
  servers: 'servers.nav',
  accounts: 'nav.accounts',
  settings: 'nav.settings'
}

/** An instance under "Jump back in"; right-click for the same menu as its Library card. */
function RecentItem({ inst, active, running }: { inst: Instance; active: boolean; running: boolean }) {
  const { go } = useStore()
  const menu = useInstanceMenu(inst)
  return (
    <>
      <button className={`nav-item ${active ? 'active' : ''}`} onClick={() => go({ page: 'instance', id: inst.id })} onContextMenu={menu.open}>
        <span className="nav-dot">{inst.iconUrl ? <img src={inst.iconUrl} alt="" /> : <BlockArt seed={inst.id} color={inst.color} />}</span>
        <span className="ellipsis">{inst.name}</span>
        {running && <span className="live" title={t('app.running')} />}
      </button>
      {menu.dialogs}
    </>
  )
}

/** The sidebar account chip's menu: switch the active account right here, or go manage them. */
function accountMenu(accounts: AccountView[], pick: (a: AccountView) => void, manage: () => void): ContextEntry[] {
  return [
    ...accounts.map((a) => ({
      label: a.username,
      icon: <Avatar account={a} size={22} />,
      hint: t(ACCOUNT_KIND_SHORT[a.type]),
      checked: a.active,
      onSelect: () => !a.active && pick(a)
    })),
    '-',
    { label: t('account.manage'), icon: <Users />, onSelect: manage }
  ]
}

export default function App() {
  const { route, go, instances, games, accounts, activeAccount, switchAccount, toast, newInstanceOpen, setNewInstanceOpen } = useStore()
  useRemoteActions()
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
          {NAV.filter((n) => n.page !== 'bedrock' || hasBedrockApp).map((n) => (
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
            <RecentItem key={i.id} inst={i} active={route.page === 'instance' && route.id === i.id} running={games[i.id] === 'running'} />
          ))}
        </div>

        <SidebarPumpkin />
        <button
          className="account-chip"
          title={accounts.length ? t('account.switch') : undefined}
          aria-haspopup={accounts.length ? 'menu' : undefined}
          onClick={(e) => {
            if (!accounts.length) return go({ page: 'accounts' })
            // Opens upward from the chip's top-left (it sits at the bottom of the sidebar).
            const r = e.currentTarget.getBoundingClientRect()
            const pick = (a: AccountView) =>
              void switchAccount(a.id).then((ok) => ok && toast(t('account.switched', { name: a.username }), 'info'))
            openContextMenu(
              { x: r.left, y: r.top - 4, keyboard: e.detail === 0, origin: e.currentTarget },
              accountMenu(accounts, pick, () => go({ page: 'accounts' })),
              t('account.switch')
            )
          }}
        >
          <Avatar account={activeAccount} size={34} />
          <div className="who">
            <b>{activeAccount?.username ?? t('app.notSignedIn')}</b>
            <span>{t(activeAccount ? ACCOUNT_KIND[activeAccount.type] : 'app.clickToSignIn')}</span>
          </div>
          {accounts.length > 0 && <ChevronsUpDown className="chip-caret" />}
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
          {route.page === 'quickjoin' && <QuickJoin />}
          {route.page === 'instance' && <InstancePage key={route.id} id={route.id} tab={route.tab} />}
          {route.page === 'discover' && (
            <Discover key={`${route.instanceId ?? ''}${route.kind ?? ''}`} initialKind={route.kind} instanceId={route.instanceId} build={route.build} />
          )}
          {route.page === 'accounts' && <Accounts />}
          {route.page === 'bedrock' && <Bedrock />}
          {route.page === 'settings' && <SettingsPage />}
          {route.page === 'servers' && (
            <Suspense fallback={null}>
              <Servers key={`${route.id ?? ''}${route.fromInstance ?? ''}`} id={route.id} fromInstance={route.fromInstance} />
            </Suspense>
          )}
        </div>
      </main>

      {newInstanceOpen && <NewInstanceModal onClose={() => setNewInstanceOpen(false)} />}
      <Dock />
      <UpdateBanner />
      <CrashFixer />
      <EasterEgg />
      <ThemeDecor />
      <PackUpdatesBanner />
      <RelayWatcher />
      <ContextMenuHost />
      <DropImport />
      {isTauri ? (
        <WindowControls />
      ) : (
        <div className="win-controls">
          <UpdateTitleButton />
        </div>
      )}
    </div>
  )
}
