// Classic layout (Settings → Appearance, classicLayoutState.ts): the 1.6.1 shell around today's pages. A bar down the
// left with every page, the instances played last ("Jump back in") and the signed-in account at its foot; over the
// page, a thin title bar with where you are, search and big screen mode. classic.css styles it.
import { ChevronRight, ChevronsUpDown, Tv } from 'lucide-react'
import type { ReactNode } from 'react'
import type { AccountView, Instance } from '@shared/types'
import { useInstanceMenu } from '../pages/Library'
import { useStore, type Route } from '../store'
import { t, type Key } from '../i18n'
import { setBigScreen } from '../bigScreen'
import { Avatar, InstanceTile, Logo, openContextMenu, type ContextEntry } from './ui'
import QuickSearch from './QuickSearch'
import { SidebarPumpkin } from './ThemeDecor'

export type ClassicNavItem = { page: Route['page']; label: Key; icon: ReactNode }

/** The line under the account's name. */
const ACCOUNT_KIND: Record<AccountView['type'], Key> = { msa: 'app.microsoftAccount', bedrock: 'app.bedrockProfile', offline: 'app.offlineProfile' }

/** An instance under "Jump back in"; right-click for the same menu as its Library card. */
function RecentItem({ inst, active, running }: { inst: Instance; active: boolean; running: boolean }) {
  const { go } = useStore()
  const menu = useInstanceMenu(inst)
  return (
    <>
      <button className={`nav-item ${active ? 'active' : ''}`} onClick={() => go({ page: 'instance', id: inst.id })} onContextMenu={menu.open}>
        <InstanceTile inst={inst} className="nav-dot" />
        <span className="ellipsis">{inst.name}</span>
        {running && <span className="live" title={t('app.running')} />}
      </button>
      {menu.dialogs}
    </>
  )
}

export function ClassicSidebar({
  nav,
  current,
  accountMenu
}: {
  nav: ClassicNavItem[]
  current: string
  accountMenu: (accounts: AccountView[], pick: (a: AccountView) => void, manage: () => void) => ContextEntry[]
}) {
  const { route, go, instances, games, accounts, activeAccount, switchAccount, toast } = useStore()
  const recent = instances.filter((i) => i.lastPlayed || games[i.id]).slice(0, 6)
  return (
    <aside className="sidebar classic-side">
      <div className="brand" data-tauri-drag-region>
        <Logo />
        <div className="brand-name">
          ResinCraft
          <small>{t('app.tagline')}</small>
        </div>
      </div>

      <nav className="nav">
        {nav.map((n) => (
          <button
            key={n.page}
            data-guide={n.page === 'settings' || n.page === 'admin' ? n.page : undefined}
            className={`nav-item ${current === n.page ? 'active' : ''}`}
            aria-current={current === n.page ? 'page' : undefined}
            onClick={() => go({ page: n.page } as Route)}
          >
            {n.icon}
            <span className="ellipsis">{t(n.label)}</span>
            {n.page === 'library' && instances.length > 0 && <span className="count">{instances.length}</span>}
          </button>
        ))}
      </nav>

      <div className="classic-label">{t('app.jumpBackIn')}</div>
      <div className="nav nav-recent">
        {recent.length === 0 && <div className="hint classic-empty">{t('app.recentEmpty')}</div>}
        {recent.map((i) => (
          <RecentItem key={i.id} inst={i} active={route.page === 'instance' && route.id === i.id} running={games[i.id] === 'running'} />
        ))}
      </div>

      <SidebarPumpkin />
      <button
        className={`account-chip ${current === 'accounts' ? 'active' : ''}`}
        title={accounts.length ? t('account.switch') : t('app.clickToSignIn')}
        aria-haspopup={accounts.length ? 'menu' : undefined}
        onClick={(e) => {
          if (!accounts.length) return go({ page: 'accounts' })
          // Opens upward from the chip's top-left (it sits at the foot of the bar).
          const r = e.currentTarget.getBoundingClientRect()
          const pick = (a: AccountView) => void switchAccount(a.id).then((ok) => ok && toast(t('account.switched', { name: a.username }), 'info'))
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
  )
}

/** The title bar over the page: where you are, then search and big screen mode. Its empty parts drag the window. */
export function ClassicTopBar({ title, searchPages }: { title: string; searchPages: { page: Route['page']; label: string; icon: ReactNode }[] }) {
  const { route, go, instances } = useStore()
  const instance = route.page === 'instance' ? instances.find((i) => i.id === route.id) : undefined
  return (
    <header className="classic-top" data-tauri-drag-region>
      <div className="crumb" data-tauri-drag-region>
        {instance ? (
          <>
            <button onClick={() => go({ page: 'library' })}>{t('nav.library')}</button>
            <ChevronRight size={14} />
            <b>{instance.name}</b>
          </>
        ) : (
          <b>{title}</b>
        )}
      </div>
      <div className="classic-tools">
        <QuickSearch pages={searchPages} />
        <button className="head-btn" data-guide="bigscreen" title={`${t('bigscreen.enter')} (F11)`} aria-label={t('bigscreen.enter')} onClick={() => void setBigScreen(true)}>
          <Tv />
        </button>
      </div>
    </header>
  )
}
