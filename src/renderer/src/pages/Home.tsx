import { Check, Clock, Compass, Gamepad2, Layers, Plus, Sparkles, Timer } from 'lucide-react'
import { InstanceCard } from './Library'
import { InstanceTile, Logo, PlayButton } from '../components/ui'
import { useStore } from '../store'
import { describe, greeting, playtime, timeAgo } from '../util'

export default function Home() {
  const { instances, activeAccount, go, setNewInstanceOpen } = useStore()
  const featured = instances.find((i) => i.lastPlayed) ?? instances[0]
  const total = instances.reduce((s, i) => s + i.playtime, 0)

  if (!activeAccount || !instances.length) {
    const steps = [
      {
        done: !!activeAccount,
        title: 'Sign in',
        text: 'Use the Microsoft account that owns Minecraft.',
        action: () => go({ page: 'accounts' }),
        cta: 'Sign in'
      },
      {
        done: instances.length > 0,
        title: 'Create an instance',
        text: 'Pick a version and, if you like, Fabric or Quilt for mods.',
        action: () => setNewInstanceOpen(true),
        cta: 'Create'
      },
      {
        done: false,
        title: 'Add mods & play',
        text: 'Browse thousands of mods, packs and shaders from Modrinth.',
        action: () => go({ page: 'discover' }),
        cta: 'Explore'
      }
    ]
    return (
      <div className="page">
        <div className="welcome" style={{ marginTop: 12 }}>
          <div className="intro">
            <Logo className="logo" />
            <h1>Welcome to ResinCraft</h1>
            <p className="muted" style={{ fontSize: 15, maxWidth: 440 }}>
              A clean, friendly home for all your Minecraft setups. Keep separate instances for every modpack, version and world, and launch any of them in one
              click.
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
          <h1>
            {greeting()}, {activeAccount.username}
          </h1>
          <p>Ready when you are.</p>
        </div>
      </div>

      {featured && (
        <section className="hero">
          <InstanceTile inst={featured} className="hero-art" />
          <div>
            <div className="eyebrow">{featured.lastPlayed ? 'Continue playing' : 'Start playing'}</div>
            <h1>{featured.name}</h1>
            <div className="meta">
              <span>
                <Gamepad2 /> {describe(featured)}
              </span>
              <span>
                <Timer /> {playtime(featured.playtime)}
              </span>
              <span>
                <Clock /> {featured.lastPlayed ? `Played ${timeAgo(featured.lastPlayed).toLowerCase()}` : 'Not played yet'}
              </span>
            </div>
            <div className="row">
              <PlayButton inst={featured} size="lg" />
              <button className="btn lg" onClick={() => go({ page: 'instance', id: featured.id })}>
                Manage
              </button>
            </div>
          </div>
        </section>
      )}

      <div className="stats" style={{ marginTop: 16 }}>
        <div className="stat">
          <b>{instances.length}</b>
          <span>Instances</span>
        </div>
        <div className="stat">
          <b>{playtime(total)}</b>
          <span>Total play time</span>
        </div>
        <div className="stat">
          <b>{new Set(instances.map((i) => i.mcVersion)).size}</b>
          <span>Minecraft versions</span>
        </div>
        <button className="stat" style={{ textAlign: 'left', cursor: 'pointer' }} onClick={() => go({ page: 'discover' })}>
          <b style={{ color: 'var(--accent)', display: 'flex', alignItems: 'center', gap: 8 }}>
            <Sparkles size={20} /> Discover
          </b>
          <span>Find mods, packs & shaders</span>
        </button>
      </div>

      <InstanceRow />
    </div>
  )
}

function InstanceRow() {
  const { instances, go, setNewInstanceOpen } = useStore()
  return (
    <>
      <h2 className="section">
        <Layers size={18} /> Your instances
        <button className="link" onClick={() => go({ page: 'library' })}>
          View all
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
          New instance
        </button>
        {instances.length === 0 && (
          <button className="icard new" onClick={() => go({ page: 'discover', kind: 'modpack' })}>
            <span className="plus">
              <Compass />
            </span>
            Install a modpack
          </button>
        )}
      </div>
    </>
  )
}
