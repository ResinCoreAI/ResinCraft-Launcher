import { useEffect, useRef, useState, type ReactNode } from 'react'
import { AlertTriangle, CheckCircle2, Info, Loader2, MoreHorizontal, Play, Square, X } from 'lucide-react'
import type { AccountView, Instance } from '@shared/types'
import { useInstanceTask, useStore } from '../store'

export function Logo({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 64 64" className={className} aria-hidden>
      <defs>
        <linearGradient id="resin-frame" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#b26bff" />
          <stop offset="1" stopColor="#22d3ee" />
        </linearGradient>
        <linearGradient id="resin-drop" x1="0" y1="0" x2="0.6" y2="1">
          <stop offset="0" stopColor="#39ff88" />
          <stop offset="1" stopColor="#22d3ee" />
        </linearGradient>
      </defs>
      <path d="M32 4 56 18v28L32 60 8 46V18z" fill="#120c1f" stroke="url(#resin-frame)" strokeWidth="3.5" strokeLinejoin="round" />
      <path d="M32 15c7 9 11 15 11 21a11 11 0 0 1-22 0c0-6 4-12 11-21z" fill="url(#resin-drop)" />
      <path d="M26.5 37.5a5.5 5.5 0 0 0 5.5 5.5" stroke="#fff" strokeOpacity=".6" strokeWidth="3" strokeLinecap="round" fill="none" />
    </svg>
  )
}

/** Small deterministic PRNG so the same name always produces the same block. */
function seeded(seed: string) {
  let h = 2166136261
  for (let i = 0; i < seed.length; i++) h = Math.imul(h ^ seed.charCodeAt(i), 16777619)
  return () => {
    h ^= h << 13
    h ^= h >>> 17
    h ^= h << 5
    return (h >>> 0) / 4294967296
  }
}

const SHADES = [0.14, 0.3, 0.55, 0.9]

/**
 * A pixel "block texture" generated from a seed: mostly dark speckle with a few bright ore-like
 * clusters in the given colour. Every instance gets its own, so the library reads like an inventory.
 */
export function BlockArt({ seed, color, cols = 8, rows = 8 }: { seed: string; color: string; cols?: number; rows?: number }) {
  const rand = seeded(seed)
  const cells: number[] = Array.from({ length: cols * rows }, () => {
    const r = rand()
    return r < 0.46 ? 0 : r < 0.78 ? 1 : r < 0.94 ? 2 : 3
  })
  // Grow two or three small "ore" clusters so it looks like a texture, not noise.
  const clusters = 2 + Math.floor(rand() * 2)
  for (let c = 0; c < clusters; c++) {
    let x = Math.floor(rand() * cols)
    let y = Math.floor(rand() * rows)
    for (let s = 0; s < 4; s++) {
      cells[y * cols + x] = 3
      x = Math.min(cols - 1, Math.max(0, x + (rand() < 0.5 ? -1 : 1)))
      y = Math.min(rows - 1, Math.max(0, y + (rand() < 0.5 ? 0 : 1)))
    }
  }
  return (
    <svg className="block-art" viewBox={`0 0 ${cols} ${rows}`} preserveAspectRatio="xMidYMid slice" shapeRendering="crispEdges" aria-hidden>
      <rect width={cols} height={rows} style={{ fill: 'var(--surface-2)' }} />
      {cells.map((lvl, i) => (
        <rect key={i} x={i % cols} y={Math.floor(i / cols)} width="1" height="1" fill={color} fillOpacity={SHADES[lvl]} />
      ))}
    </svg>
  )
}

export function InstanceTile({
  inst,
  className,
  style,
  wide
}: {
  inst: Pick<Instance, 'name' | 'color' | 'iconUrl'> & { id?: string }
  className?: string
  style?: React.CSSProperties
  /** Use a 16×9 texture for banner-shaped tiles. */
  wide?: boolean
}) {
  return (
    <div className={`tile ${className ?? ''}`} style={style}>
      {inst.iconUrl ? <img src={inst.iconUrl} alt="" /> : <BlockArt seed={inst.id ?? inst.name} color={inst.color} cols={wide ? 16 : 8} rows={wide ? 9 : 8} />}
    </div>
  )
}

/** Renders the face (plus hat layer) straight from the player's skin texture. */
export function Avatar({ account, size = 32 }: { account?: AccountView; size?: number }) {
  if (!account?.skinUrl)
    return (
      <div className="avatar" style={{ width: size, height: size }} title={account?.username}>
        <BlockArt seed={account?.username ?? '?'} color={account?.type === 'offline' ? '#a8a3bd' : '#b26bff'} />
      </div>
    )
  const layer = (x: number) => `${-x * size}px ${-size}px`
  return (
    <div
      className="avatar"
      style={{
        width: size,
        height: size,
        backgroundImage: `url("${account.skinUrl}"), url("${account.skinUrl}")`,
        backgroundSize: `${size * 8}px auto, ${size * 8}px auto`,
        backgroundPosition: `${layer(5)}, ${layer(1)}`,
        backgroundRepeat: 'no-repeat'
      }}
    />
  )
}

export function PlayButton({ inst, size = '' }: { inst: Instance; size?: 'lg' | 'sm' | '' }) {
  const { games, play, stop } = useStore()
  const task = useInstanceTask(inst.id)
  const state = games[inst.id]
  if (state === 'running')
    return (
      <button className={`btn play-btn running ${size}`} onClick={() => void stop(inst.id)} title="Force-stop the game">
        <Square /> Stop
      </button>
    )
  if (state === 'preparing')
    return (
      <button className={`btn primary play-btn ${size}`} disabled>
        <Loader2 className="spin" /> {task?.progress != null ? <span className="pct">{Math.round(task.progress * 100)}%</span> : 'Preparing'}
      </button>
    )
  return (
    <button className={`btn primary play-btn ${size}`} onClick={() => void play(inst.id)}>
      <Play fill="currentColor" /> Play
    </button>
  )
}

export function Modal({
  title,
  subtitle,
  children,
  footer,
  onClose,
  wide
}: {
  title: string
  subtitle?: string
  children: ReactNode
  footer?: ReactNode
  onClose: () => void
  wide?: boolean
}) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && onClose()
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [onClose])
  return (
    <div className="backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className={`modal ${wide ? 'wide' : ''}`} role="dialog" aria-modal aria-label={title}>
        <div className="modal-head">
          <div className="grow">
            <h2>{title}</h2>
            {subtitle && <p>{subtitle}</p>}
          </div>
          <button className="btn ghost icon sm" onClick={onClose} aria-label="Close">
            <X />
          </button>
        </div>
        <div className="modal-body">{children}</div>
        {footer && <div className="modal-foot">{footer}</div>}
      </div>
    </div>
  )
}

export interface MenuItem {
  label: string
  icon?: ReactNode
  onClick: () => void
  danger?: boolean
  divider?: boolean
}

export function Menu({ items, trigger }: { items: MenuItem[]; trigger?: ReactNode }) {
  const [open, setOpen] = useState(false)
  const ref = useRef<HTMLDivElement>(null)
  useEffect(() => {
    if (!open) return
    const close = (e: MouseEvent) => !ref.current?.contains(e.target as Node) && setOpen(false)
    window.addEventListener('mousedown', close)
    return () => window.removeEventListener('mousedown', close)
  }, [open])
  return (
    <div className="menu-wrap" ref={ref} onClick={(e) => e.stopPropagation()}>
      <button className="btn icon" onClick={() => setOpen((o) => !o)} aria-label="More actions">
        {trigger ?? <MoreHorizontal />}
      </button>
      {open && (
        <div className="menu">
          {items.map((it) => (
            <div key={it.label}>
              {it.divider && <hr />}
              <button
                className={it.danger ? 'danger' : ''}
                onClick={() => {
                  setOpen(false)
                  it.onClick()
                }}
              >
                {it.icon}
                {it.label}
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}

export function Switch({ on, onChange, label }: { on: boolean; onChange: (v: boolean) => void; label?: string }) {
  return <button className={`switch ${on ? 'on' : ''}`} role="switch" aria-checked={on} aria-label={label} onClick={() => onChange(!on)} />
}

export function Segmented<T extends string>({
  value,
  options,
  onChange
}: {
  value: T
  options: { value: T; label: ReactNode; disabled?: boolean }[]
  onChange: (v: T) => void
}) {
  return (
    <div className="segmented">
      {options.map((o) => (
        <button key={o.value} className={o.value === value ? 'on' : ''} disabled={o.disabled} onClick={() => onChange(o.value)}>
          {o.label}
        </button>
      ))}
    </div>
  )
}

export function Empty({ icon, title, children, action }: { icon: ReactNode; title: string; children?: ReactNode; action?: ReactNode }) {
  return (
    <div className="empty">
      <div className="ico">{icon}</div>
      <h3>{title}</h3>
      {children && <p>{children}</p>}
      {action}
    </div>
  )
}

export function ConfirmModal({
  title,
  body,
  confirm,
  onConfirm,
  onClose
}: {
  title: string
  body: ReactNode
  confirm: string
  onConfirm: () => Promise<void> | void
  onClose: () => void
}) {
  const [busy, setBusy] = useState(false)
  return (
    <Modal
      title={title}
      onClose={onClose}
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            Cancel
          </button>
          <button
            className="btn danger solid"
            disabled={busy}
            onClick={async () => {
              setBusy(true)
              try {
                await onConfirm()
                onClose()
              } finally {
                setBusy(false)
              }
            }}
          >
            {confirm}
          </button>
        </>
      }
    >
      <div className="muted">{body}</div>
    </Modal>
  )
}

export function Dock() {
  const { tasks, toasts, dismiss } = useStore()
  const icons = { success: <CheckCircle2 />, error: <AlertTriangle />, info: <Info /> }
  return (
    <div className="dock" aria-live="polite">
      {tasks.map((t) => (
        <div className="task" key={t.id}>
          <div className="top">
            {t.state === 'error' ? (
              <AlertTriangle style={{ color: 'var(--danger)' }} />
            ) : t.state === 'done' ? (
              <CheckCircle2 style={{ color: 'var(--success)' }} />
            ) : (
              <Loader2 className="spin" />
            )}
            <span className="grow ellipsis">{t.title}</span>
            {t.progress != null && t.state === 'running' && <span className="faint">{Math.round(t.progress * 100)}%</span>}
          </div>
          <div className="detail ellipsis">{t.state === 'error' ? t.error : (t.detail ?? 'Working…')}</div>
          {t.state === 'running' && (
            <div className={`bar ${t.progress == null ? 'indeterminate' : ''}`}>
              <i style={{ width: `${(t.progress ?? 0) * 100}%` }} />
            </div>
          )}
        </div>
      ))}
      {toasts.map((t) => (
        <div className={`toast ${t.kind}`} key={t.id}>
          {icons[t.kind]}
          <span className="grow">{t.text}</span>
          <button className="x" onClick={() => dismiss(t.id)} aria-label="Dismiss">
            <X size={15} />
          </button>
        </div>
      ))}
    </div>
  )
}
