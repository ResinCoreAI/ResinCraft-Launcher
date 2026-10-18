import { useEffect, useRef, useState, type ReactNode } from 'react'
import { createPortal } from 'react-dom'
import { AlertTriangle, CheckCircle2, Info, Loader2, MoreHorizontal, Play, Square, X } from 'lucide-react'
import type { AccountView, Instance } from '@shared/types'
import { useInstanceTask, useStore } from '../store'
import { t, tx } from '../i18n'

/** The ResinCraft mark: a resin drop holding a block, with a pixel "R" on its side. Same art as resources/logo.svg. */
export function Logo({ className }: { className?: string }) {
  return (
    <svg viewBox="140 30 744 970" className={className} aria-hidden>
      <path fill="#8b46ff" d="M512 48C490 150 400 252 268 402C206 472 168 548 168 632A344 344 0 0 0 856 632C856 548 818 472 756 402C624 252 534 150 512 48Z" />
      <path fill="#4aff86" d="M299 500L520 372L741 500L520 628Z" />
      <path fill="#1bc866" d="M284 525L506 653L506 907L284 780Z" />
      <path fill="#3a178c" d="M534 653L755 525L755 780L534 907Z" />
      <g fill="#4aff86">
        <path d="M550 650L585 630L585 668L550 688Z" />
        <path d="M550 698L585 678L585 716L550 736Z" />
        <path d="M550 746L585 726L585 764L550 784Z" />
        <path d="M550 794L585 774L585 812L550 832Z" />
        <path d="M550 842L585 822L585 860L550 880Z" />
        <path d="M593 626L628 606L628 644L593 664Z" />
        <path d="M636 601L671 581L671 619L636 639Z" />
        <path d="M678 625L713 605L713 643L678 663Z" />
        <path d="M593 722L628 702L628 740L593 760Z" />
        <path d="M636 697L671 677L671 715L636 735Z" />
        <path d="M636 745L671 725L671 763L636 783Z" />
        <path d="M679 776C679 765 689 758 701 757C712 757 713 769 708 781C702 793 691 801 684 799C680 797 679 790 679 776Z" />
      </g>
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
      <button className={`btn play-btn running ${size}`} onClick={() => void stop(inst.id)} title={t('common.forceStop')}>
        <Square /> {t('common.stop')}
      </button>
    )
  if (state === 'preparing')
    return (
      <button className={`btn primary play-btn ${size}`} disabled>
        <Loader2 className="spin" /> {task?.progress != null ? <span className="pct">{Math.round(task.progress * 100)}%</span> : t('common.preparing')}
      </button>
    )
  return (
    <button className={`btn primary play-btn ${size}`} onClick={() => void play(inst.id)}>
      <Play fill="currentColor" /> {t('common.play')}
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
  // Rendered into <body>: a page's entrance animation uses a transform, which would otherwise
  // make this fixed backdrop cover (and centre within) the page instead of the window.
  return createPortal(
    <div className="backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className={`modal ${wide ? 'wide' : ''}`} role="dialog" aria-modal aria-label={title}>
        <div className="modal-head">
          <div className="grow">
            <h2>{title}</h2>
            {subtitle && <p>{subtitle}</p>}
          </div>
          <button className="btn ghost icon sm" onClick={onClose} aria-label={t('common.close')}>
            <X />
          </button>
        </div>
        <div className="modal-body">{children}</div>
        {footer && <div className="modal-foot">{footer}</div>}
      </div>
    </div>,
    document.body
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
      <button className="btn icon" onClick={() => setOpen((o) => !o)} aria-label={t('common.moreActions')}>
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
            {t('common.cancel')}
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

/**
 * A task's progress bar. Installs run in stages (pack files, libraries, assets), each counting 0→100%
 * with short "working…" gaps between them. The bar keeps the last value through those gaps instead of
 * flipping to the sweeping animation, and a new stage restarts it without sliding backwards.
 */
function TaskBar({ progress }: { progress?: number | null }) {
  const last = useRef<number | null>(null)
  const stage = useRef(0)
  if (progress != null) {
    if (last.current != null && progress < last.current - 0.02) stage.current++
    last.current = progress
  }
  const shown = progress ?? last.current
  if (shown == null) {
    return (
      <div className="bar indeterminate">
        <i />
      </div>
    )
  }
  return (
    <div className="bar">
      <i key={stage.current} style={{ width: `${shown * 100}%` }} />
    </div>
  )
}

export function Dock() {
  const { tasks, toasts, dismiss } = useStore()
  const icons = { success: <CheckCircle2 />, error: <AlertTriangle />, info: <Info /> }
  return (
    <div className="dock" aria-live="polite">
      {tasks.map((task) => (
        <div className="task" key={task.id}>
          <div className="top">
            {task.state === 'error' ? (
              <AlertTriangle style={{ color: 'var(--danger)' }} />
            ) : task.state === 'done' ? (
              <CheckCircle2 style={{ color: 'var(--success)' }} />
            ) : (
              <Loader2 className="spin" />
            )}
            <span className="grow ellipsis">{tx(task.titleText, task.title)}</span>
            {task.progress != null && task.state === 'running' && <span className="faint">{Math.round(task.progress * 100)}%</span>}
          </div>
          <div className="detail ellipsis">{task.state === 'error' ? tx(task.errorText, task.error) : tx(task.detailText, task.detail) || t('common.working')}</div>
          {task.state === 'running' && <TaskBar progress={task.progress} />}
        </div>
      ))}
      {toasts.map((toast) => (
        <div className={`toast ${toast.kind}`} key={toast.id}>
          {icons[toast.kind]}
          <span className="grow">{toast.text}</span>
          <button className="x" onClick={() => dismiss(toast.id)} aria-label={t('common.dismiss')}>
            <X size={15} />
          </button>
        </div>
      ))}
    </div>
  )
}
