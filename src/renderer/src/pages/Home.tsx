import { useEffect, useState } from 'react'
import { Check, Clock, Compass, Layers, Plus, Sparkles, Timer } from 'lucide-react'
import type { Instance } from '@shared/types'
import { InstanceCard, useInstanceMenu } from './Library'
import { BlockArt, InstanceTile, Logo, PlayButton } from '../components/ui'
import { api } from '../api'
import { QuickJoinHome } from '../components/QuickJoin'
import { useStore } from '../store'
import { describe, greeting, playtime, timeAgo } from '../util'
import { t } from '../i18n'
import { MemoryCard } from '../components/Memory'
import { AnnouncementsCard } from '../components/Announcements'

export default function Home() {
  const { instances, activeAccount, go, setNewInstanceOpen, settings } = useStore()
  const featured = instances.find((i) => i.lastPlayed) ?? instances[0]
  const total = instances.reduce((s, i) => s + i.playtime, 0)
  const versionCount = new Set(instances.map((i) => i.mcVersion)).size

  if (!activeAccount || !instances.length) {
    const steps = [
      {
        done: !!activeAccount,
        title: t('home.step.signIn.title'),
        text: t('home.step.signIn.text'),
        action: () => go({ page: 'accounts' }),
        cta: t('home.step.signIn.cta')
      },
      {
        done: instances.length > 0,
        title: t('home.step.create.title'),
        text: t('home.step.create.text'),
        action: () => setNewInstanceOpen(true),
        cta: t('home.step.create.cta')
      },
      {
        done: false,
        title: t('home.step.explore.title'),
        text: t('home.step.explore.text'),
        action: () => go({ page: 'discover' }),
        cta: t('home.step.explore.cta')
      }
    ]
    return (
      <div className="page">
        <div className="welcome" style={{ marginTop: 12 }}>
          <div className="intro">
            <Logo className="logo" />
            <h1>{t('home.welcome.title')}</h1>
            <p className="muted" style={{ fontSize: 15, maxWidth: 440 }}>
              {t('home.welcome.body')}
            </p>
          </div>
          <div className="steps">
            {steps.map((s, n) => (
              <div key={s.title} className={`step ${s.done ? 'done' : ''}`}>
                <div className="num">{s.done ? <Check size={16} /> : n + 1}</div>
                <div className="grow">
                  <b>{s.title}</b>
                  <span>{s.text}</span>
                </div>
                {!s.done && (
                  <button className={`btn sm ${steps.findIndex((x) => !x.done) === n ? 'primary' : ''}`} onClick={s.action}>
                    {s.cta}
                  </button>
                )}
              </div>
            ))}
          </div>
        </div>
        {/* A hosted server can run before the first instance exists. */}
        {settings?.showMemoryOnHome && <MemoryCard />}
        {instances.length > 0 && <InstanceRow />}
      </div>
    )
  }

  return (
    <div className="page">
      <div className="page-head" style={{ marginBottom: 16 }}>
        <div>
          <h1>{t('home.greeting', { greeting: greeting(), name: activeAccount.username })}</h1>
          <p>{t('home.ready')}</p>
        </div>
      </div>

      {featured && <Hero featured={featured} />}

      <div className="stats" style={{ marginTop: 16 }}>
        <div className="stat">
          <b>{instances.length}</b>
          <span>{t('home.stats.instances', { count: instances.length })}</span>
        </div>
        <div className="stat">
          <b>{playtime(total)}</b>
          <span>{t('home.stats.playtime')}</span>
        </div>
        <div className="stat">
          <b>{versionCount}</b>
          <span>{t('home.stats.versions', { count: versionCount })}</span>
        </div>
        <button className="stat" style={{ textAlign: 'left', cursor: 'pointer' }} onClick={() => go({ page: 'discover' })}>
          <b style={{ color: 'var(--accent)', display: 'flex', alignItems: 'center', gap: 8 }}>
            <Sparkles size={20} /> {t('home.stats.discover')}
          </b>
          <span>{t('home.stats.discoverHint')}</span>
        </button>
      </div>

      {settings?.showMemoryOnHome && <MemoryCard />}

      <AnnouncementsCard />

      <QuickJoinHome />

      <InstanceRow />
    </div>
  )
}

/** The newest screenshot of an instance as a small JPEG data URL (the hero's backdrop), or undefined. */
function useLatestShot(inst: Instance, skip: boolean) {
  const [shot, setShot] = useState<string>()
  useEffect(() => {
    setShot(undefined)
    if (skip) return
    let live = true
    void api.screenshots
      .list(inst.id)
      .then((list) => {
        const newest = [...list].sort((a, b) => b.mtime - a.mtime)[0]
        return newest ? api.screenshots.thumbnail(inst.id, newest.name) : undefined
      })
      .then((url) => live && setShot(url))
      .catch(() => {})
    return () => {
      live = false
    }
  }, [inst.id, skip])
  return shot
}

/**
 * The instance you played last, front and centre: its banner (or, without one, its newest screenshot, softly
 * blurred) fills the card behind the title. Right-click it for the same menu as its card.
 */
function Hero({ featured }: { featured: Instance }) {
  const { go, games } = useStore()
  const menu = useInstanceMenu(featured)
  const shot = useLatestShot(featured, !!featured.bannerUrl)
  const backdrop = featured.bannerUrl ?? shot
  const live = games[featured.id] === 'running'
  const [version, loader] = describe(featured).split(' · ')
  return (
    <>
      <section
        className={`hero ${backdrop ? 'has-backdrop' : ''} ${featured.bannerUrl ? 'sharp' : ''}`}
        style={{ '--hero-tint': featured.color } as React.CSSProperties}
        onContextMenu={menu.open}
      >
        <div className="hero-backdrop" aria-hidden>
          {backdrop ? <img src={backdrop} alt="" draggable={false} /> : <BlockArt seed={featured.id} color={featured.color} cols={24} rows={9} />}
        </div>
        <div className="hero-body">
          <div className="hero-top">
            <InstanceTile inst={featured} className="hero-icon" />
            <div className="eyebrow">{featured.lastPlayed ? t('home.hero.continue') : t('home.hero.start')}</div>
          </div>
          <h1>{featured.name}</h1>
          <div className="hero-chips">
            <span className="chip">{version}</span>
            {loader && <span className="chip">{loader}</span>}
            {live && (
              <span className="chip live">
                <span className="dot" aria-hidden /> {t('library.card.running')}
              </span>
            )}
          </div>
          <div className="meta">
            <span>
              <Timer /> {playtime(featured.playtime)}
            </span>
            <span>
              <Clock /> {featured.lastPlayed ? t('home.hero.played', { when: timeAgo(featured.lastPlayed, true) }) : t('home.hero.notPlayed')}
            </span>
          </div>
          <div className="row">
            <PlayButton inst={featured} size="lg" />
            <button className="btn lg glass" onClick={() => go({ page: 'instance', id: featured.id })}>
              {t('home.hero.manage')}
            </button>
          </div>
        </div>
      </section>
      {menu.dialogs}
    </>
  )
}

function InstanceRow() {
  const { instances, go, setNewInstanceOpen } = useStore()
  return (
    <>
      <h2 className="section">
        <Layers size={18} /> {t('home.instances.title')}
        <button className="link" onClick={() => go({ page: 'library' })}>
          {t('home.instances.viewAll')}
        </button>
      </h2>
      <div className="grid">
        {instances.slice(0, 7).map((i) => (
          <InstanceCard key={i.id} inst={i} />
        ))}
        <button className="icard new" onClick={() => setNewInstanceOpen(true)}>
          <span className="plus">
            <Plus />
          </span>
          {t('home.instances.new')}
        </button>
        {instances.length === 0 && (
          <button className="icard new" onClick={() => go({ page: 'discover', kind: 'modpack' })}>
            <span className="plus">
              <Compass />
            </span>
            {t('home.instances.installModpack')}
          </button>
        )}
      </div>
    </>
  )
}
