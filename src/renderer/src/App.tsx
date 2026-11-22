import {
  ChevronDown,
  ChevronRight,
  Compass,
  Home as HomeIcon,
  Layers,
  Radar,
  Server,
  Settings as SettingsIcon,
  Shield,
  ShoppingBag,
  Smartphone,
  Tv,
  Users,
  Zap
} from 'lucide-react'
import { OpenLinks } from './components/OpenLinks'
import { useGamepad } from './gamepad'
import { bigScreenPrefs, setBigScreen, toggleBigScreen, useBigScreen } from './bigScreen'
import { lazy, Suspense, useEffect, type ComponentType, type ReactNode } from 'react'
import type { AccountView } from '@shared/types'
import { ACCOUNT_KIND_SHORT, Avatar, ContextMenuHost, Dock, Logo, openMenuBelow, type ContextEntry } from './components/ui'
import DropImport from './components/DropImport'
import OpenFiles from './components/OpenFiles'
import WindowControls from './components/WindowControls'
import QuickSearch from './components/QuickSearch'
import InstanceDock from './components/InstanceDock'
import LaunchScreen from './components/LaunchScreen'
import PageGuide from './components/PageGuide'
import HeaderNav from './components/HeaderNav'
import { UpdateBanner, UpdateTitleButton } from './components/Updates'
import { useRemoteActions } from './components/RemoteControl'
import ThemeDecor, { SidebarPumpkin } from './components/ThemeDecor'
import EasterEgg from './components/EasterEgg'
import ThemeAmbient from './components/ThemeAmbient'
import CrashFixer from './components/CrashFixer'
import { NativeBuildHost } from './components/HorizonNative'
import { PackUpdatesBanner } from './components/PackUpdates'
import { JoinAnswersNotice } from './components/ServerList'
import { RelayWatcher } from './components/ServerRelay'
import { api, isTauri } from './api'
import NewInstanceModal from './components/NewInstanceModal'
import Home from './pages/Home'
import Library from './pages/Library'
import { useStore, type Route } from './store'
import { t, type Key } from './i18n'
import { hasBedrockApp } from './platform'
import { useAdminStatus } from './adminStatus'
import { useStoreOpen } from './storeStatus'
import { decideWelcome, useWelcome } from './welcomeState'

/**
 * A page whose code loads apart from the launcher's start: on its first visit, or before that with `preload`. Once
 * its code is in, the page shows at once; a page still loading would show only after React's 300 ms wait.
 */
function page<P extends object>(load: () => Promise<{ default: ComponentType<P> }>) {
  let loaded: ComponentType<P> | undefined
  const get = () => load().then((m) => ((loaded = m.default), m))
  const Lazy = lazy(get)
  const Page = (props: P) => {
    const Shown = loaded ?? Lazy
    return <Shown {...props} />
  }
  Page.preload = () => void get().catch(() => {})
  return Page
}

// Home and the Library are where the launcher opens; every other page (and what only it uses, like the theme
// editor in Settings) is read a moment after the start (`preloadPages`), so starting up parses less and a page
// still opens at once.
const Servers = page(() => import('./pages/Servers'))
const BigScreen = page(() => import('./components/BigScreen'))
const InstancePage = page(() => import('./pages/InstancePage'))
const Discover = page(() => import('./pages/Discover'))
const QuickJoin = page(() => import('./pages/QuickJoin'))
const Accounts = page(() => import('./pages/Accounts'))
const Bedrock = page(() => import('./pages/Bedrock'))
const Horizon = lazy(() => import('./pages/Horizon'))
const SettingsPage = page(() => import('./pages/SettingsPage'))
const StorePage = lazy(() => import('./pages/Store'))
const FindServers = page(() => import('./pages/FindServers'))
const Report = lazy(() => import('./pages/Report'))
const Admin = lazy(() => import('./pages/Admin'))
// Only a brand-new launcher (or Settings → About) ever shows it.
const Welcome = lazy(() => import('./components/Welcome'))

/** The pages the header and an instance open, read one by one while the launcher has nothing else to do. */
function preloadPages() {
  const pages = [InstancePage, Discover, SettingsPage, Servers, QuickJoin, FindServers, Accounts, Bedrock, BigScreen]
  const next = () => {
    const p = pages.shift()
    if (!p) return
    p.preload()
    requestIdleCallback(next, { timeout: 2000 })
  }
  window.setTimeout(() => requestIdleCallback(next, { timeout: 2000 }), 1500)
}

/**
 * The header's pages, in three groups: `play` (what you already have) and `get` (where more comes from) are its
 * links; `you` (your accounts and the launcher itself) sits at its right end, as icons and the account's menu.
 */
const NAV: { page: Route['page']; label: Key; icon: ReactNode; group: 'play' | 'get' | 'you' }[] = [
  { page: 'home', label: 'nav.home', icon: <HomeIcon />, group: 'play' },
  { page: 'library', label: 'nav.library', icon: <Layers />, group: 'play' },
  { page: 'quickjoin', label: 'quickjoin.nav', icon: <Zap />, group: 'play' },
  // The public server list: somewhere new to play.
  { page: 'findservers', label: 'servers.find.nav', icon: <Radar />, group: 'play' },
  { page: 'bedrock', label: 'nav.bedrock', icon: <Smartphone />, group: 'play' },
  // Horizon's own launcher (pages/Horizon.tsx) is hidden: Horizon instances and servers don't need it.
  { page: 'discover', label: 'nav.discover', icon: <Compass />, group: 'get' },
  { page: 'servers', label: 'servers.nav', icon: <Server />, group: 'get' },
  // While ResinCraft Relay sells add-ons and packs (useStoreOpen).
  { page: 'store', label: 'servers.store.nav', icon: <ShoppingBag />, group: 'get' },
  { page: 'accounts', label: 'nav.accounts', icon: <Users />, group: 'you' },
  { page: 'settings', label: 'nav.settings', icon: <SettingsIcon />, group: 'you' },
  // Only for ResinCraft Relay's admin accounts (useAdminStatus).
  { page: 'admin', label: 'servers.admin.nav', icon: <Shield />, group: 'you' }
]

/** The header's account menu: switch the active account right here, or go manage them. */
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
  const { route, go, instances, games, accounts, activeAccount, switchAccount, toast, newInstanceOpen, setNewInstanceOpen, settings, ready } = useStore()
  const welcome = useWelcome()
  useEffect(() => {
    if (ready) decideWelcome(!accounts.length && !instances.length)
    if (ready) preloadPages()
  }, [ready])
  useRemoteActions()
  // Discord's presence while no game runs says what the player is doing here (discord.rs `page_lines`). Sent once
  // the page has settled for a moment: Discord only takes a few changes a minute.
  const presenceName = route.page === 'instance' ? instances.find((i) => i.id === route.id)?.name : undefined
  const presenceKind = route.page === 'discover' ? (route.kind ?? null) : null
  useEffect(() => {
    const timer = window.setTimeout(() => void api.app.discordPage(route.page, presenceKind, presenceName ?? null).catch(() => {}), 1200)
    return () => window.clearTimeout(timer)
  }, [route.page, presenceKind, presenceName])
  useGamepad(
    settings?.gamepad !== false,
    (name) => {
      // Big screen mode by itself when a controller connects, if the player asked for that.
      if (bigScreenPrefs().autoController) void setBigScreen(true)
      else toast(t('gamepad.connected', { name }))
    },
    () => go({ page: 'home' })
  )
  const bigScreen = useBigScreen()
  // F11: big screen mode on or off.
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.key !== 'F11') return
      e.preventDefault()
      void toggleBigScreen()
    }
    window.addEventListener('keydown', key)
    return () => window.removeEventListener('keydown', key)
  }, [])
  const isAdmin = useAdminStatus()
  const storeOpen = useStoreOpen()
  const current = route.page === 'instance' ? 'library' : route.page
  const instance = route.page === 'instance' ? instances.find((i) => i.id === route.id) : undefined
  const nav = NAV.filter((n) => (n.page !== 'bedrock' || hasBedrockApp) && (n.page !== 'admin' || isAdmin) && (n.page !== 'store' || storeOpen))
  const playing = Object.values(games).some((g) => g === 'running')

  // The window buttons float over the header's right end; it keeps clear of however wide they are just now
  // (an "Update" button joins them when there is one).
  useEffect(() => {
    const el = document.querySelector('.win-controls')
    if (!el) return
    const set = () => document.documentElement.style.setProperty('--win-w', `${Math.ceil(el.getBoundingClientRect().width)}px`)
    const ro = new ResizeObserver(set)
    ro.observe(el)
    set()
    return () => ro.disconnect()
  }, [])

  return (
    <div className="app">
      {/* The site header, which is also the window's title bar. It keeps the class it had as a left bar, so themes
          and custom CSS written for .sidebar still colour it. Its empty parts drag the window. */}
      <header className="sidebar" data-tauri-drag-region>
        <div className="brand" data-tauri-drag-region>
          <Logo />
          <div className="brand-name">ResinCraft</div>
        </div>

        <HeaderNav
          current={current}
          onGo={(page) => go({ page } as Route)}
          items={nav
            .filter((n) => n.group !== 'you')
            .map((n) => ({
              page: n.page,
              label: t(n.label),
              icon: n.icon,
              extra:
                n.page === 'library' ? (
                  <>
                    {instances.length > 0 && <span className="count">{instances.length}</span>}
                    {playing && <span className="live" title={t('app.running')} />}
                  </>
                ) : undefined
            }))}
        />

        <div className="head-tools" data-tauri-drag-region>
          <QuickSearch pages={nav.map((n) => ({ page: n.page, label: t(n.label), icon: n.icon }))} />
          <button className="head-btn" data-guide="bigscreen" title={`${t('bigscreen.enter')} (F11)`} aria-label={t('bigscreen.enter')} onClick={() => void setBigScreen(true)}>
            <Tv />
          </button>
          {nav
            .filter((n) => n.group === 'you' && n.page !== 'accounts')
            .map((n) => (
              <button
                key={n.page}
                data-guide={n.page}
                className={`head-btn ${current === n.page ? 'active' : ''}`}
                title={t(n.label)}
                aria-label={t(n.label)}
                aria-current={current === n.page ? 'page' : undefined}
                onClick={() => go({ page: n.page } as Route)}
              >
                {n.icon}
              </button>
            ))}
          <button
            className={`account-chip ${current === 'accounts' ? 'active' : ''}`}
            title={accounts.length ? t('account.switch') : t('app.clickToSignIn')}
            aria-haspopup={accounts.length ? 'menu' : undefined}
            onClick={(e) => {
              if (!accounts.length) return go({ page: 'accounts' })
              const pick = (a: AccountView) => void switchAccount(a.id).then((ok) => ok && toast(t('account.switched', { name: a.username }), 'info'))
              openMenuBelow(e.currentTarget, accountMenu(accounts, pick, () => go({ page: 'accounts' })), e.detail === 0, t('account.switch'))
            }}
          >
            <Avatar account={activeAccount} size={26} />
            <div className="who">
              <b>{activeAccount?.username ?? t('app.notSignedIn')}</b>
            </div>
            {accounts.length > 0 && <ChevronDown className="chip-caret" />}
          </button>
        </div>
      </header>

      <main className="main">
        <div className="content">
          {/* The way back from an instance; it scrolls away with the page. */}
          {instance && (
            <div className="topbar">
              <div className="crumb">
                <button onClick={() => go({ page: 'library' })}>{t('nav.library')}</button>
                <ChevronRight size={14} />
                <b>{instance.name}</b>
              </div>
            </div>
          )}
          <Suspense fallback={null}>
            {route.page === 'home' && <Home />}
            {route.page === 'library' && <Library />}
            {route.page === 'quickjoin' && <QuickJoin />}
            {route.page === 'instance' && <InstancePage key={`${route.id}:${route.tab ?? ''}`} id={route.id} tab={route.tab} />}
            {route.page === 'discover' && (
              <Discover
                key={`${route.instanceId ?? ''}${route.kind ?? ''}${route.query ?? ''}`}
                initialKind={route.kind}
                instanceId={route.instanceId}
                build={route.build}
                initialQuery={route.query}
              />
            )}
            {route.page === 'accounts' && <Accounts />}
            {route.page === 'bedrock' && <Bedrock />}
            {route.page === 'horizon' && <Horizon />}
            {route.page === 'settings' && <SettingsPage />}
            {route.page === 'admin' && <Admin />}
            {route.page === 'store' && <StorePage />}
            {route.page === 'findservers' && <FindServers />}
            {route.page === 'report' && <Report type={route.type} title={route.title} message={route.message} />}
            {route.page === 'servers' && <Servers key={`${route.id ?? ''}${route.fromInstance ?? ''}`} id={route.id} fromInstance={route.fromInstance} />}
          </Suspense>
        </div>
      </main>

      {welcome.open && (
        <Suspense fallback={null}>
          <Welcome />
        </Suspense>
      )}
      {newInstanceOpen && <NewInstanceModal onClose={() => setNewInstanceOpen(false)} />}
      {bigScreen && (
        <Suspense fallback={null}>
          <BigScreen pages={nav.filter((n) => n.page !== 'home' && n.page !== 'library').map((n) => ({ page: n.page, label: t(n.label), icon: n.icon }))} />
        </Suspense>
      )}
      <LaunchScreen />
      <InstanceDock />
      <Dock />
      <PageGuide />
      <UpdateBanner />
      <CrashFixer />
      <NativeBuildHost />
      <EasterEgg />
      <ThemeDecor />
      {/* The holiday piece (a pumpkin, a Christmas chest) stands in the window's bottom-left corner. */}
      <div className="shell-decor" aria-hidden>
        <SidebarPumpkin />
      </div>
      <ThemeAmbient />
      <PackUpdatesBanner />
      <JoinAnswersNotice />
      <RelayWatcher />
      <ContextMenuHost />
      <DropImport />
      <OpenFiles />
      <OpenLinks />
      {isTauri ? (
        <WindowControls />
      ) : import.meta.env.DEV ? (
        // The browser preview shows them, to see their style; they do nothing there.
        <WindowControls live={false} />
      ) : (
        <div className="win-controls">
          <UpdateTitleButton />
        </div>
      )}
    </div>
  )
}
