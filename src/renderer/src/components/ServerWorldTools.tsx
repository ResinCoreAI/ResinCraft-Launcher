// Server → Settings → World tools (server/worldtools.rs): several worlds, chunk reset and a small player editor.
// On this PC only: the web console and other PCs never reach these, so Servers.tsx doesn't show this there.
import { useCallback, useEffect, useMemo, useState, type ReactNode } from 'react'
import { AlertTriangle, ArrowRightLeft, Eraser, Loader2, Plus, Search, Trash2, Users } from 'lucide-react'
import type { ServerInfo } from '@shared/types'
import type { ChunkArea, ChunkCount, ServerWorld, WorldPlayer, WorldPlayerEdit, WorldToolResult, WorldToolsInfo } from '@shared/worldtools'
import { api } from '../api'
import { Modal, Segmented, Switch } from './ui'
import { useStore } from '../store'
import { bytes } from '../util'
import { t, type Key } from '../i18n'
import '../server-worldtools.css'

/** Whether a backup comes first. Off asks for a second look: it's the only thing between a mistake and a lost world. */
function BackupChoice({ on, onChange, offText = 'servers.worldTools.backupOff' }: { on: boolean; onChange: (v: boolean) => void; offText?: Key }) {
  return (
    <>
      <div className="setting">
        <div className="text">
          <b>{t('servers.worldTools.backup')}</b>
          <span>{t('servers.worldTools.backupHint')}</span>
        </div>
        <Switch on={on} onChange={onChange} label={t('servers.worldTools.backup')} />
      </div>
      {!on && (
        <p className="sv-wt-warn" role="alert">
          <AlertTriangle /> {t(offText)}
        </p>
      )}
    </>
  )
}

/** A dialog whose action runs one tool and reports the backup it made. */
function ToolDialog({
  title,
  action,
  icon,
  danger,
  ready = true,
  wide,
  run,
  onDone,
  onClose,
  children
}: {
  title: string
  action: string
  icon?: ReactNode
  danger?: boolean
  ready?: boolean
  wide?: boolean
  run: () => Promise<void>
  onDone?: () => void
  onClose: () => void
  children: ReactNode
}) {
  const { toast } = useStore()
  const [busy, setBusy] = useState(false)
  const go = async () => {
    setBusy(true)
    try {
      await run()
      onDone?.()
      onClose()
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(false)
    }
  }
  return (
    <Modal
      title={title}
      onClose={onClose}
      wide={wide}
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className={`btn ${danger ? 'danger solid' : 'primary'}`} disabled={!ready || busy} onClick={() => void go()}>
            {busy ? <Loader2 className="spin" /> : icon} {action}
          </button>
        </>
      }
    >
      <div className="sv-wt-form">{children}</div>
    </Modal>
  )
}

function useReport() {
  const { toast } = useStore()
  return useCallback(
    (r: WorldToolResult, world: string) => {
      const file = r.backup?.split(/[\\/]/).pop()
      toast(file ? t('servers.worldTools.backedUp', { file, world }) : t('servers.worldTools.done'), 'success')
    },
    [toast]
  )
}

function SwitchDialog({ s, world, current, onDone, onClose }: { s: ServerInfo; world: ServerWorld; current: string; onDone: () => void; onClose: () => void }) {
  const report = useReport()
  const [backup, setBackup] = useState(true)
  return (
    <ToolDialog
      title={t('servers.worldTools.switchTitle', { name: world.name })}
      action={t('servers.worldTools.switch')}
      icon={<ArrowRightLeft />}
      run={async () => report(await api.server.worldSwitch(s.id, { name: world.name, backup }), current)}
      onDone={onDone}
      onClose={onClose}
    >
      <p className="muted">{t('servers.worldTools.switchBody', { name: world.name, current })}</p>
      <BackupChoice on={backup} onChange={setBackup} offText="servers.worldTools.backupOffSwitch" />
    </ToolDialog>
  )
}

function NewWorldDialog({ s, current, onDone, onClose }: { s: ServerInfo; current: string; onDone: () => void; onClose: () => void }) {
  const report = useReport()
  const [name, setName] = useState('')
  const [seed, setSeed] = useState('')
  const [backup, setBackup] = useState(true)
  return (
    <ToolDialog
      title={t('servers.worldTools.newTitle')}
      action={t('servers.worldTools.create')}
      icon={<Plus />}
      ready={name.trim() !== ''}
      run={async () => report(await api.server.worldCreate(s.id, { name: name.trim(), seed: seed.trim(), backup }), current)}
      onDone={onDone}
      onClose={onClose}
    >
      <p className="muted">{t('servers.worldTools.newBody')}</p>
      <label>
        {t('servers.worldTools.name')}
        <input className="input" value={name} maxLength={64} placeholder={t('servers.worldTools.namePlaceholder')} spellCheck={false} autoFocus onChange={(e) => setName(e.target.value)} />
      </label>
      <label>
        {t('servers.worldTools.seed')}
        <input className="input mono" value={seed} maxLength={64} placeholder={t('servers.worldTools.seedPlaceholder')} spellCheck={false} onChange={(e) => setSeed(e.target.value)} />
      </label>
      <BackupChoice on={backup} onChange={setBackup} offText="servers.worldTools.backupOffSwitch" />
    </ToolDialog>
  )
}

function DeleteDialog({ s, world, onDone, onClose }: { s: ServerInfo; world: ServerWorld; onDone: () => void; onClose: () => void }) {
  const report = useReport()
  const [typed, setTyped] = useState('')
  const [backup, setBackup] = useState(true)
  return (
    <ToolDialog
      title={t('servers.worldTools.deleteTitle', { name: world.name })}
      action={t('common.delete')}
      icon={<Trash2 />}
      danger
      ready={typed === world.name}
      run={async () => report(await api.server.worldDelete(s.id, { name: world.name, confirm: typed, backup }), world.name)}
      onDone={onDone}
      onClose={onClose}
    >
      <p className="muted">{t('servers.worldTools.deleteBody', { folders: world.folders.join(', ') })}</p>
      <BackupChoice on={backup} onChange={setBackup} />
      <label>
        {t('servers.worldTools.typeName', { name: world.name })}
        <input className="input" value={typed} autoComplete="off" spellCheck={false} onChange={(e) => setTyped(e.target.value)} />
      </label>
    </ToolDialog>
  )
}

const WHOLE = /^-?\d+$/

function ChunkDialog({ s, world, onClose }: { s: ServerInfo; world: ServerWorld; onClose: () => void }) {
  const { toast } = useStore()
  const report = useReport()
  const [dimension, setDimension] = useState(world.dimensions[0]?.path ?? '')
  const [unit, setUnit] = useState<'blocks' | 'chunks'>('blocks')
  const [c, setC] = useState({ x1: '0', z1: '0', x2: '0', z2: '0' })
  const [backup, setBackup] = useState(true)
  const [found, setFound] = useState<ChunkCount>()
  const [checking, setChecking] = useState(false)

  const valid = Object.values(c).every((v) => WHOLE.test(v.trim()))
  const area = (): ChunkArea => ({ x1: Number(c.x1), z1: Number(c.z1), x2: Number(c.x2), z2: Number(c.z2), blocks: unit === 'blocks' })
  // A count is only good for the area it was made for.
  useEffect(() => setFound(undefined), [dimension, unit, c])

  const check = async () => {
    setChecking(true)
    try {
      setFound(await api.server.chunkPreview(s.id, { world: world.name, dimension, area: area() }))
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setChecking(false)
    }
  }
  const total = found ? found.chunks + found.entities + found.poi + found.external : 0
  const field = (key: keyof typeof c, label: string) => (
    <label key={key}>
      {label}
      <input className="input narrow mono" inputMode="numeric" value={c[key]} spellCheck={false} onChange={(e) => setC({ ...c, [key]: e.target.value })} />
    </label>
  )
  return (
    <ToolDialog
      title={t('servers.worldTools.chunkTitle', { name: world.name })}
      action={t('servers.worldTools.resetCount', { chunks: String(found?.chunks ?? 0) })}
      icon={<Eraser />}
      danger
      ready={total > 0}
      run={async () => {
        const done = await api.server.chunkReset(s.id, { world: world.name, dimension, area: area(), backup })
        toast(t('servers.worldTools.resetDone', { chunks: String(done.chunks) }), 'success')
        if (done.backup) report(done, world.name)
      }}
      onClose={onClose}
    >
      <p className="muted">{t('servers.worldTools.chunkBody')}</p>
      <label>
        {t('servers.worldTools.dimension')}
        <select className="input" value={dimension} onChange={(e) => setDimension(e.target.value)}>
          {world.dimensions.map((d) => (
            <option key={d.path} value={d.path}>
              {d.name} ({d.path})
            </option>
          ))}
        </select>
      </label>
      <div className="field">
        {t('servers.worldTools.unit')}
        <Segmented
          value={unit}
          onChange={setUnit}
          options={[
            { value: 'blocks', label: t('servers.worldTools.blocks') },
            { value: 'chunks', label: t('servers.worldTools.chunkUnit') }
          ]}
        />
      </div>
      <div className="sv-wt-corners">
        <span>{t('servers.worldTools.corner1')}</span>
        {field('x1', 'X')}
        {field('z1', 'Z')}
        <span>{t('servers.worldTools.corner2')}</span>
        {field('x2', 'X')}
        {field('z2', 'Z')}
      </div>
      <div className="row">
        <button className="btn" disabled={!valid || !dimension || checking} onClick={() => void check()}>
          {checking ? <Loader2 className="spin" /> : <Search />} {t('servers.worldTools.check')}
        </button>
      </div>
      {found &&
        (total === 0 ? (
          <p className="muted">{t('servers.worldTools.nothing')}</p>
        ) : (
          <p>
            {t('servers.worldTools.found', { chunks: String(found.chunks), width: String(found.width), depth: String(found.depth), regions: String(found.regions) })}{' '}
            {t('servers.worldTools.foundParts', { entities: String(found.entities), poi: String(found.poi), external: String(found.external) })}
          </p>
        ))}
      <BackupChoice on={backup} onChange={setBackup} />
    </ToolDialog>
  )
}

type Fields = Record<'x' | 'y' | 'z' | 'health' | 'food' | 'xpLevel' | 'xpProgress' | 'xpTotal', string>

const text = (v: number | null | undefined) => (v === null || v === undefined ? '' : String(v))

function fieldsOf(p: WorldPlayer): Fields {
  return { x: text(p.pos?.[0]), y: text(p.pos?.[1]), z: text(p.pos?.[2]), health: text(p.health), food: text(p.food), xpLevel: text(p.xpLevel), xpProgress: text(p.xpProgress), xpTotal: text(p.xpTotal) }
}

/** Only what the host changed, so everything else stays exactly as the file has it. Undefined when a value isn't a number. */
function changes(p: WorldPlayer, f: Fields, mode: number | null): WorldPlayerEdit | undefined {
  const before = fieldsOf(p)
  const changed = (k: keyof Fields) => f[k].trim() !== before[k]
  const all = Object.keys(f) as (keyof Fields)[]
  if (all.some((k) => changed(k) && (f[k].trim() === '' || !Number.isFinite(Number(f[k]))))) return undefined
  const edit: WorldPlayerEdit = {}
  if (changed('x') || changed('y') || changed('z')) edit.pos = [Number(f.x), Number(f.y), Number(f.z)]
  if (mode !== p.gameMode && mode !== null) edit.gameMode = mode
  for (const k of ['health', 'food', 'xpLevel', 'xpProgress', 'xpTotal'] as const) if (changed(k)) edit[k] = Number(f[k])
  return edit
}

function PlayerForm({ s, world, player, onSaved, onClose }: { s: ServerInfo; world: ServerWorld; player: WorldPlayer; onSaved: () => void; onClose: () => void }) {
  const { toast } = useStore()
  const report = useReport()
  const [f, setF] = useState(() => fieldsOf(player))
  const [mode, setMode] = useState(player.gameMode)
  const [backup, setBackup] = useState(true)
  const edit = changes(player, f, mode)
  const name = player.name ?? player.uuid
  const input = (key: keyof Fields, label: string, shown: boolean) =>
    shown && (
      <label key={key}>
        {label}
        <input className="input narrow mono" inputMode="decimal" value={f[key]} spellCheck={false} onChange={(e) => setF({ ...f, [key]: e.target.value })} />
      </label>
    )
  return (
    <ToolDialog
      title={name}
      action={t('common.save')}
      ready={!!edit && Object.keys(edit).length > 0}
      run={async () => {
        const r = await api.server.worldPlayerSave(s.id, { world: world.name, file: player.file, edit: edit!, backup })
        toast(t('servers.worldTools.saved', { name }), 'success')
        if (r.backup) report(r, world.name)
      }}
      onDone={onSaved}
      onClose={onClose}
    >
      {player.pos && (
        <>
          <div className="field">
            {t('servers.worldTools.position')}
            <div className="sv-wt-row">
              {input('x', 'X', true)}
              {input('y', 'Y', true)}
              {input('z', 'Z', true)}
            </div>
            {player.dimension && <span className="hint">{t('servers.worldTools.inDimension', { dimension: player.dimension })}</span>}
            {player.vehicle && <span className="hint">{t('servers.worldTools.vehicle')}</span>}
          </div>
        </>
      )}
      {mode !== null && (
        <div className="field">
          {t('servers.worldTools.gameMode')}
          <Segmented value={String(mode)} onChange={(v) => setMode(Number(v))} options={[0, 1, 2, 3].map((m) => ({ value: String(m), label: t(`servers.worldTools.mode.${m}` as Key) }))} />
          <span className="hint">{t('servers.worldTools.gameModeHint')}</span>
        </div>
      )}
      <div className="sv-wt-row">
        {input('health', t('servers.worldTools.health'), player.health !== null)}
        {input('food', t('servers.worldTools.food'), player.food !== null)}
      </div>
      <div className="sv-wt-row">
        {input('xpLevel', t('servers.worldTools.xpLevel'), player.xpLevel !== null)}
        {input('xpProgress', t('servers.worldTools.xpProgress'), player.xpProgress !== null)}
        {input('xpTotal', t('servers.worldTools.xpTotal'), player.xpTotal !== null)}
      </div>
      {!edit && <p className="sv-wt-warn">{t('servers.worldTools.bad')}</p>}
      <BackupChoice on={backup} onChange={setBackup} />
    </ToolDialog>
  )
}

function PlayersDialog({ s, world, locked, onClose }: { s: ServerInfo; world: ServerWorld; locked: boolean; onClose: () => void }) {
  const { toast } = useStore()
  const [players, setPlayers] = useState<WorldPlayer[]>()
  const [q, setQ] = useState('')
  const [editing, setEditing] = useState<WorldPlayer>()
  const load = useCallback(
    () =>
      api.server.worldPlayers(s.id, world.name).then(setPlayers, (e) => {
        setPlayers([])
        toast((e as Error).message, 'error')
      }),
    [s.id, world.name, toast]
  )
  useEffect(() => void load(), [load])
  const shown = useMemo(() => (players ?? []).filter((p) => `${p.name ?? ''} ${p.uuid}`.toLowerCase().includes(q.toLowerCase())), [players, q])
  return (
    <Modal title={t('servers.worldTools.playersTitle', { name: world.name })} wide onClose={onClose}>
      <div className="sv-wt-form">
        {players === undefined ? (
          <Loader2 className="spin" />
        ) : players.length === 0 ? (
          <p className="muted">{t('servers.worldTools.noPlayers')}</p>
        ) : (
          <>
            <div className="search">
              <Search />
              <input className="input" placeholder={t('servers.worldTools.search')} value={q} onChange={(e) => setQ(e.target.value)} />
            </div>
            <div className="card">
              {shown.map((p) => (
                <div className="setting" key={p.file}>
                  <div className="text">
                    <b>{p.name ?? t('servers.worldTools.unknownName')}</b>
                    <span className="mono">{p.uuid}</span>
                    {p.error ? (
                      <span className="sv-wt-bad">{p.error}</span>
                    ) : (
                      <span>
                        {p.gameMode !== null && t(`servers.worldTools.mode.${p.gameMode}` as Key)}
                        {p.xpLevel !== null && ` · ${t('servers.worldTools.level', { level: String(p.xpLevel) })}`}
                        {p.pos && ` · ${p.pos.map((v) => Math.round(v)).join(', ')}`}
                      </span>
                    )}
                  </div>
                  <button className="btn sm" disabled={locked || !!p.error} onClick={() => setEditing(p)}>
                    {t('common.edit')}
                  </button>
                </div>
              ))}
            </div>
          </>
        )}
      </div>
      {editing && <PlayerForm s={s} world={world} player={editing} onSaved={() => void load()} onClose={() => setEditing(undefined)} />}
    </Modal>
  )
}

type Open = { kind: 'switch' | 'delete' | 'chunks' | 'players'; world: ServerWorld } | { kind: 'new' }

export default function ServerWorldTools({ s }: { s: ServerInfo }) {
  const { toast } = useStore()
  const [info, setInfo] = useState<WorldToolsInfo>()
  const [open, setOpen] = useState<Open>()
  const running = s.state !== 'stopped' && s.state !== 'crashed'

  const load = useCallback(() => api.server.worldTools(s.id).then(setInfo, (e) => toast((e as Error).message, 'error')), [s.id, toast])
  useEffect(() => void load(), [load, s.state])

  if (!info) return null
  const close = () => setOpen(undefined)
  return (
    <div className="settings-section">
      <div className="sv-wt-head">
        <h3>{t('servers.worldTools.title')}</h3>
        {!info.unsupported && (
          <button className="btn" disabled={running} onClick={() => setOpen({ kind: 'new' })}>
            <Plus /> {t('servers.worldTools.new')}
          </button>
        )}
      </div>
      <p>{info.unsupported ?? (running ? t('servers.worldTools.stopFirst') : t('servers.worldTools.hint'))}</p>
      {!info.unsupported && (
        <div className="card">
          {info.worlds.map((w) => (
            <div className="setting sv-wt-world" key={w.name}>
              <div className="text">
                <b>
                  {w.name} {w.active && <span className="chip accent">{t('servers.worldTools.inUse')}</span>}
                </b>
                {w.problem ? (
                  <span className="sv-wt-bad">{w.problem}</span>
                ) : (
                  <span>{w.generated ? t('servers.worldTools.summary', { size: bytes(w.size), dimensions: String(w.dimensions.length), players: String(w.players) }) : t('servers.worldTools.notMade')}</span>
                )}
              </div>
              {w.generated && (
                <div className="row">
                  {!w.active && (
                    <button className="btn sm" disabled={running || !!w.problem} onClick={() => setOpen({ kind: 'switch', world: w })}>
                      <ArrowRightLeft /> {t('servers.worldTools.switch')}
                    </button>
                  )}
                  <button className="btn sm" disabled={running || !!w.problem || w.dimensions.length === 0} onClick={() => setOpen({ kind: 'chunks', world: w })}>
                    <Eraser /> {t('servers.worldTools.chunks')}
                  </button>
                  <button className="btn sm" disabled={!!w.problem || w.players === 0} onClick={() => setOpen({ kind: 'players', world: w })}>
                    <Users /> {t('servers.worldTools.players')}
                  </button>
                  <button
                    className="btn sm danger"
                    disabled={running || w.active}
                    title={w.active ? t('servers.worldTools.deleteActive') : undefined}
                    aria-label={t('common.delete')}
                    onClick={() => setOpen({ kind: 'delete', world: w })}
                  >
                    <Trash2 />
                  </button>
                </div>
              )}
            </div>
          ))}
        </div>
      )}
      {open?.kind === 'switch' && <SwitchDialog s={s} world={open.world} current={info.active} onDone={() => void load()} onClose={close} />}
      {open?.kind === 'new' && <NewWorldDialog s={s} current={info.active} onDone={() => void load()} onClose={close} />}
      {open?.kind === 'delete' && <DeleteDialog s={s} world={open.world} onDone={() => void load()} onClose={close} />}
      {open?.kind === 'chunks' && <ChunkDialog s={s} world={open.world} onClose={close} />}
      {open?.kind === 'players' && <PlayersDialog s={s} world={open.world} locked={running} onClose={close} />}
    </div>
  )
}
