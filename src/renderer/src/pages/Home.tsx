import { Check, Clock, Compass, Gamepad2, Layers, Plus, Sparkles, Timer } from 'lucide-react'
import type { Instance } from '@shared/types'
import { InstanceCard, useInstanceMenu } from './Library'
import { InstanceTile, Logo, PlayButton } from '../components/ui'
import { useStore } from '../store'
import { describe, greeting, playtime, timeAgo } from '../util'
import { t } from '../i18n'

export default function Home() {
  const { instances, activeAccount, go, setNewInstanceOpen } = useStore()
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

      <InstanceRow />
    </div>
  )
}

/** The instance you played last, front and centre; right-click it for the same menu as its card. */
function Hero({ featured }: { featured: Instance }) {
  const { go } = useStore()
  const menu = useInstanceMenu(featured)
  return (
    <>
      <section className="hero" onContextMenu={menu.open}>
        <InstanceTile inst={featured} className="hero-art" />
        <div>
          <div className="eyebrow">{featured.lastPlayed ? t('home.hero.continue') : t('home.hero.start')}</div>
          <h1>{featured.name}</h1>
          <div className="meta">
            <span>
              <Gamepad2 /> {describe(featured)}
            </span>
            <span>
              <Timer /> {playtime(featured.playtime)}
            </span>
            <span>
              <Clock /> {featured.lastPlayed ? t('home.hero.played', { when: timeAgo(featured.lastPlayed, true) }) : t('home.hero.notPlayed')}
            </span>
          </div>
          <div className="row">
            <PlayButton inst={featured} size="lg" />
            <button className="btn lg" onClick={() => go({ page: 'instance', id: featured.id })}>
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
