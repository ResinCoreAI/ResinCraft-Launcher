import { ChevronDown, ChevronUp, Plus } from 'lucide-react'
import { useEffect, useState } from 'react'
import type { Instance } from '@shared/types'
import { useInstanceMenu } from '../pages/Library'
import { useInstanceTask, useStore } from '../store'
import { t } from '../i18n'
import { InstanceTile } from './ui'

/**
 * One instance in the dock: click to open it, double-click to play it (as its Library card's Play button: not while
 * it is already starting or running, or being installed), right-click for the same menu as its Library card.
 */
function DockItem({ inst, current, running }: { inst: Instance; current: boolean; running: boolean }) {
  const { go, play, games } = useStore()
  const menu = useInstanceMenu(inst)
  // Being installed, updated or prepared: an outline fills around the tile as it goes.
  const task = useInstanceTask(inst.id)
  const pct = task?.progress != null ? Math.round(task.progress * 100) : undefined
  const name = task ? (pct != null ? `${inst.name} · ${pct}%` : `${inst.name} · ${t('common.working')}`) : inst.name
  return (
    <>
      <button
        className={`idock-item ${current ? 'current' : ''} ${running ? 'running' : ''} ${task ? 'busy' : ''}`}
        data-name={name}
        aria-label={running ? `${inst.name} (${t('app.running')})` : name}
        aria-current={current ? 'page' : undefined}
        onClick={() => go({ page: 'instance', id: inst.id })}
        onDoubleClick={() => {
          if (!games[inst.id] && !task) void play(inst.id)
        }}
        onContextMenu={menu.open}
      >
        <InstanceTile inst={inst} />
        {task && (
          <svg className={`idock-ring ${pct == null ? 'unknown' : ''}`} viewBox="0 0 52 52" aria-hidden>
            <rect className="track" x="2" y="2" width="48" height="48" rx="15" />
            <rect className="fill" x="2" y="2" width="48" height="48" rx="15" pathLength={100} strokeDasharray={`${pct ?? 25} 100`} />
          </svg>
        )}
      </button>
      {menu.dialogs}
    </>
  )
}

/** Whether the dock is put away (its Hide button): remembered on this computer until it is brought back. */
const STOWED = 'resincraft.dock'
function wasStowed(): boolean {
  try {
    return localStorage.getItem(STOWED) === 'hidden'
  } catch {
    return false
  }
}

/**
 * How many instances the dock has room for: it sits in the middle of the window's foot, between a notice in the
 * left corner and the task list in the right one (each up to 360 px wide), so a narrower window shows fewer.
 */
function useRoom(): number {
  const count = () => Math.max(2, Math.min(7, Math.floor((window.innerWidth - 2 * 392 - 74) / 50)))
  const [room, setRoom] = useState(count)
  useEffect(() => {
    const onResize = () => setRoom(count())
    window.addEventListener('resize', onResize)
    return () => window.removeEventListener('resize', onResize)
  }, [])
  return room
}

/**
 * The dock: the instances played last (and any that are running), a click away from every page, in a small bar at
 * the foot of the window. It slides out of the way while a page is scrolled down, and comes back on scrolling up or
 * when the pointer reaches the window's bottom edge. Its last button puts it away for good, leaving a small tab at
 * the window's foot that brings it back.
 */
export default function InstanceDock() {
  const { instances, games, tasks, route, setNewInstanceOpen } = useStore()
  const [away, setAway] = useState(false)
  const [stowed, setStowed] = useState(wasStowed)
  const stow = (next: boolean) => {
    setStowed(next)
    setAway(false)
    try {
      localStorage.setItem(STOWED, next ? 'hidden' : 'shown')
    } catch {
      // Storage unavailable: the choice holds until the launcher closes.
    }
  }
  const room = useRoom()
  const busy = (i: Instance) => tasks.some((x) => x.instanceId === i.id && x.state === 'running')
  // What is being installed right now comes first, so it is in the dock whatever else is.
  const recent = [...instances.filter(busy), ...instances.filter((i) => !busy(i) && (i.lastPlayed || games[i.id]))].slice(0, room)
  const shown = recent.length > 0
  const here = route.page === 'instance' ? route.id : route.page

  useEffect(() => {
    if (!shown) return
    const page = document.querySelector('.content')
    if (!page) return
    let last = page.scrollTop
    const onScroll = () => {
      const y = page.scrollTop
      // Small jitters (a list growing, a smooth scroll settling) don't count as a change of direction.
      if (Math.abs(y - last) < 8) return
      setAway(y > last && y > 80)
      last = y
    }
    page.addEventListener('scroll', onScroll, { passive: true })
    return () => page.removeEventListener('scroll', onScroll)
  }, [shown])

  // A page that has just opened starts with the dock in view.
  useEffect(() => setAway(false), [here])

  if (!shown) return null
  if (stowed)
    return (
      <button
        className={`idock-tab ${instances.some(busy) ? 'busy' : ''}`}
        title={t('version.show')}
        aria-label={`${t('version.show')}: ${t('app.jumpBackIn')}`}
        onClick={() => stow(false)}
      >
        <ChevronUp />
      </button>
    )
  return (
    <>
      <div className="idock-edge" onMouseEnter={() => setAway(false)} />
      <nav className={`idock ${away ? 'away' : ''}`} aria-label={t('app.jumpBackIn')} onFocus={() => setAway(false)}>
        {recent.map((i) => (
          <DockItem key={i.id} inst={i} current={route.page === 'instance' && route.id === i.id} running={games[i.id] === 'running'} />
        ))}
        <span className="idock-sep" aria-hidden />
        <button className="idock-item idock-add" data-name={t('home.instances.new')} aria-label={t('home.instances.new')} onClick={() => setNewInstanceOpen(true)}>
          <Plus />
        </button>
        <button className="idock-stow" title={t('version.hide')} aria-label={`${t('version.hide')}: ${t('app.jumpBackIn')}`} onClick={() => stow(true)}>
          <ChevronDown />
        </button>
      </nav>
    </>
  )
}
