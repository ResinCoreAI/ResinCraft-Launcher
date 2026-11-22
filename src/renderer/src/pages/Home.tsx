import { useEffect, useState } from 'react'
import { LoaderArt, loaderIcon } from '../components/LoaderArt'
import { Bug, Check, Clock, Compass, Layers, Plus, Sparkles, Timer } from 'lucide-react'
import type { Instance } from '@shared/types'
import { InstanceCard, useInstanceMenu } from './Library'
import { BlockArt, InstanceTile, Logo, PlayButton } from '../components/ui'
import { api } from '../api'
import { QuickJoinHome } from '../components/QuickJoin'
import { useInstanceTask, useStore } from '../store'
import { describe, DISCLAIMER, greeting, playtime, timeAgo } from '../util'
import { t } from '../i18n'
import { MemoryCard } from '../components/Memory'
import { AnnouncementsCard } from '../components/Announcements'
import { RtxBanner, useRtxRemix } from '../components/RtxBanner'
import { usePackArt } from '../components/packArt'
import { BasicIcon, iconPng, RTX_ICON } from '../components/BasicIcons'
import { useWelcome } from '../welcomeState'
import { makeStarterInstance } from '../starterInstance'

export default function Home() {
  const { instances, activeAccount, go, setNewInstanceOpen, settings, ready, refreshInstances } = useStore()
  // A first install gets one instance to start with (starterInstance.ts). The welcome's state is read so this runs
  // again once App has decided it is a first install (App's effects run after this page's).
  const welcome = useWelcome()
  useEffect(() => {
    if (ready) void makeStarterInstance(instances.length, refreshInstances)
  }, [ready, welcome.open, instances.length])
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
      <div className="page home">
        <div className="home-col first">
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
        <SiteFooter />
      </div>
    )
  }

  // Laid out like a site's front page: the picture runs the full width of the window, the sections follow in one
  // centred column, and a footer closes the page.
  return (
    <div className="page home">
      {featured && <Hero featured={featured} hello={t('home.greeting', { greeting: greeting(), name: activeAccount.username })} />}

      <div className="home-col">
        <div className="stats">
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
          <button className="stat" onClick={() => go({ page: 'discover' })}>
            <b>
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

      <SiteFooter />
    </div>
  )
}

/** What closes the front page, as on a site: who made this, the version, and the places people look for last. */
function SiteFooter() {
  const { go, info } = useStore()
  return (
    <footer className="site-foot">
      <div className="site-foot-in">
        <div className="site-foot-brand">
          <Logo />
          <div>
            <b>ResinCraft</b>
            <span>{t('app.tagline')}</span>
          </div>
        </div>
        <nav className="site-foot-links">
          <button onClick={() => go({ page: 'library' })}>{t('nav.library')}</button>
          <button onClick={() => go({ page: 'discover' })}>{t('nav.discover')}</button>
          <button onClick={() => go({ page: 'servers' })}>{t('servers.nav')}</button>
          <button onClick={() => go({ page: 'settings' })}>{t('nav.settings')}</button>
          <button onClick={() => go({ page: 'report' })}>
            <Bug size={14} /> {t('settings.about.report')}
          </button>
        </nav>
        <p className="site-foot-note">
          {t('settings.about.credit')}
          {info?.version && <span className="mono">v{info.version}</span>}
          <span className="legal">{DISCLAIMER}</span>
        </p>
      </div>
    </footer>
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
 * The instance you played last, front and centre, with a picture behind the title. In order of preference: its
 * own banner; the RTX Remix banner if it is an RTX Remix instance; its modpack's own art (the first picture in the
 * pack's gallery); its newest screenshot, softly blurred; its logo, out of focus, with the logo itself standing
 * beside the title; and only when it has none of those, its block pattern, kept quiet. Right-click it for the same menu as its card.
 */
function Hero({ featured, hello }: { featured: Instance; hello: string }) {
  const { go, games, refreshInstances } = useStore()
  const menu = useInstanceMenu(featured)
  // Its own banner first; an RTX Remix instance without one gets the RTX Remix banner; then its newest screenshot.
  const isRtx = useRtxRemix(featured)
  const rtx = isRtx && !featured.bannerUrl
  // An RTX Remix instance that has no icon takes the RTX one: once (so removing it later sticks), and not while
  // something is being installed into it (two writers to one instance).
  const busy = !!useInstanceTask(featured.id)
  const rtxIcon = isRtx && !featured.iconUrl
  useEffect(() => {
    if (!rtxIcon || busy) return
    const key = `resincraft.rtxIcon.${featured.id}`
    try {
      if (localStorage.getItem(key)) return
      localStorage.setItem(key, '1')
    } catch {
      return
    }
    void iconPng(RTX_ICON)
      .then((png) => api.instances.setImage(featured.id, 'icon', png))
      .then(() => refreshInstances())
      .catch(() => {})
  }, [rtxIcon, busy, featured.id])
  const [broken, setBroken] = useState<string>()
  const found = usePackArt(featured, !!featured.bannerUrl || rtx)
  const art = found !== broken ? found : undefined
  const shot = useLatestShot(featured, !!featured.bannerUrl || rtx)
  const backdrop = featured.bannerUrl ?? art ?? shot
  const sharp = !!(featured.bannerUrl || art)
  const logo = !rtx && !backdrop ? featured.iconUrl : undefined
  const live = games[featured.id] === 'running'
  const [version, loader] = describe(featured).split(' · ')
  return (
    <>
      <section
        className={`hero ${backdrop || rtx ? 'has-backdrop' : ''} ${sharp ? 'sharp' : ''} ${logo ? 'logo' : ''}`}
        aria-label={featured.lastPlayed ? t('home.hero.continue') : t('home.hero.start')}
        style={{ '--hero-tint': featured.color } as React.CSSProperties}
        onContextMenu={menu.open}
      >
        <div className="hero-backdrop" aria-hidden>
          {rtx ? (
            <RtxBanner />
          ) : backdrop ? (
            <img loading="lazy" decoding="async" src={backdrop} alt="" draggable={false} onError={() => backdrop === art && setBroken(art)} />
          ) : logo ? (
            <img loading="lazy" decoding="async" src={logo} alt="" draggable={false} />
          ) : loaderIcon(featured.loader) ? (
            <LoaderArt loader={loaderIcon(featured.loader)!} wide />
          ) : (
            <BlockArt seed={featured.id} color={featured.color} cols={24} rows={9} />
          )}
        </div>
        {logo && <InstanceTile inst={featured} className="hero-logo" />}
        <div className="hero-body">
          <p className="hero-hello">{hello}</p>
          <div className="hero-title">
            {rtxIcon ? (
              <div className="tile hero-icon">
                <BasicIcon icon={RTX_ICON} />
              </div>
            ) : (
              <InstanceTile inst={featured} className="hero-icon" />
            )}
            <h1>{featured.name}</h1>
          </div>
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
