// Big screen mode: Settings (mock "Settings"). Only what's worth changing from a couch: the controller, the size of
// everything, the account. "All settings" opens the launcher's own Settings page.
import { useState, type ReactNode } from 'react'
import { ChevronLeft, ChevronRight, Gamepad2, LogOut, Minus, Monitor, Plus, Settings2, UserRound } from 'lucide-react'
import { useStore, type Route } from '../../store'
import { t, type Key } from '../../i18n'
import { setBigScreen, setBigScreenPrefs, useBigScreenPrefs, type BigScreenPrefs } from '../../bigScreen'
import { Avatar, Switch } from '../ui'

type Group = 'controller' | 'screen' | 'account'
const GROUPS: { id: Group; label: Key; icon: ReactNode }[] = [
  { id: 'controller', label: 'bigscreen.set.controller', icon: <Gamepad2 /> },
  { id: 'screen', label: 'bigscreen.set.screen', icon: <Monitor /> },
  { id: 'account', label: 'bigscreen.account', icon: <UserRound /> }
]
const PADS: BigScreenPrefs['pad'][] = ['auto', 'xbox', 'playstation']
const PAD_LABEL: Record<BigScreenPrefs['pad'], Key> = { auto: 'bigscreen.set.padAuto', xbox: 'bigscreen.set.padXbox', playstation: 'bigscreen.set.padPs' }

function Row({ icon, title, hint, children, onClick, first }: { icon: ReactNode; title: string; hint?: string; children?: ReactNode; onClick?: () => void; first?: boolean }) {
  const body = (
    <>
      {icon}
      <span className="grow">
        <b>{title}</b>
        {hint && <span>{hint}</span>}
      </span>
      {children}
    </>
  )
  return onClick ? (
    <button className="bs-row bss-row" data-first={first || undefined} onClick={onClick}>
      {body}
    </button>
  ) : (
    <div className="bs-row bss-row">{body}</div>
  )
}

export default function BsSettings({ openPage }: { openPage: (r: Route) => void }) {
  const { settings, updateSettings, accounts, activeAccount, switchAccount, toast } = useStore()
  const prefs = useBigScreenPrefs()
  const [group, setGroup] = useState<Group>('controller')
  const stepPad = (d: number) => setBigScreenPrefs({ pad: PADS[(PADS.indexOf(prefs.pad) + d + PADS.length) % PADS.length] })
  const size = (d: number) => setBigScreenPrefs({ scale: Math.min(150, Math.max(80, prefs.scale + d)) })

  return (
    <div className="bss">
      <nav className="bsd-rail" aria-label={t('bigscreen.tab.settings')}>
        {GROUPS.map((g, i) => (
          <button key={g.id} className={`bsd-kind ${group === g.id ? 'on' : ''}`} data-first={i === 0 || undefined} aria-pressed={group === g.id} onClick={() => setGroup(g.id)}>
            {g.icon} {t(g.label)}
          </button>
        ))}
        <button className="bsd-kind" onClick={() => openPage({ page: 'settings' })}>
          <Settings2 /> {t('bigscreen.allSettings')}
        </button>
      </nav>

      <div className="bss-rows">
        {group === 'controller' && (
          <>
            <Row icon={<Gamepad2 />} title={t('bigscreen.autoOpen')} hint={t('bigscreen.autoOpenHint')}>
              <Switch on={prefs.autoController} onChange={(v) => setBigScreenPrefs({ autoController: v })} label={t('bigscreen.autoOpen')} />
            </Row>
            <Row icon={<Gamepad2 />} title={t('bigscreen.set.pad')} hint={t('bigscreen.set.padHint')}>
              <span className="bss-pick">
                <button className="bss-step" aria-label={t('bigscreen.set.prev')} onClick={() => stepPad(-1)}>
                  <ChevronLeft />
                </button>
                <b>{t(PAD_LABEL[prefs.pad])}</b>
                <button className="bss-step" aria-label={t('bigscreen.set.next')} onClick={() => stepPad(1)}>
                  <ChevronRight />
                </button>
              </span>
            </Row>
            <Row icon={<Gamepad2 />} title={t('settings.gamepad')} hint={t('bigscreen.set.gamepadHint')}>
              <Switch on={settings?.gamepad !== false} onChange={(v) => void updateSettings({ gamepad: v })} label={t('settings.gamepad')} />
            </Row>
          </>
        )}
        {group === 'screen' && (
          <>
            <Row icon={<Monitor />} title={t('bigscreen.set.size')} hint={t('bigscreen.set.sizeHint')}>
              <span className="bss-pick">
                <button className="bss-step" aria-label={t('bigscreen.set.smaller')} disabled={prefs.scale <= 80} onClick={() => size(-10)}>
                  <Minus />
                </button>
                <span className="bss-meter" aria-hidden>
                  <i style={{ width: `${((prefs.scale - 80) / 70) * 100}%` }} />
                </span>
                <b className="mono">{prefs.scale}%</b>
                <button className="bss-step" aria-label={t('bigscreen.set.bigger')} disabled={prefs.scale >= 150} onClick={() => size(10)}>
                  <Plus />
                </button>
              </span>
            </Row>
            <Row icon={<LogOut />} title={t('bigscreen.leave')} hint={t('bigscreen.leaveHint')} onClick={() => void setBigScreen(false)} />
          </>
        )}
        {group === 'account' &&
          (accounts.length ? (
            accounts.map((a) => (
              <Row
                key={a.id}
                icon={<Avatar account={a} size={40} />}
                title={a.username}
                hint={a.id === activeAccount?.id ? t('bigscreen.inUse') : undefined}
                onClick={() => a.id !== activeAccount?.id && void switchAccount(a.id).then((ok) => ok && toast(t('bigscreen.switched', { name: a.username })))}
              />
            ))
          ) : (
            <Row icon={<UserRound />} title={t('bigscreen.noAccount')} onClick={() => openPage({ page: 'accounts' })} />
          ))}
      </div>
    </div>
  )
}
