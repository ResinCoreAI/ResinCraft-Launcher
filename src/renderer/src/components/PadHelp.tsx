/**
 * The controller's buttons and what they do, opened with View (Back / Select / Share). Named the way the pad in use
 * names them: Xbox letters, or PlayStation shapes for a DualShock / DualSense.
 */
import { useSyncExternalStore } from 'react'
import { createRoot } from 'react-dom/client'
import { Gamepad2, X } from 'lucide-react'
import { t, type Key } from '../i18n'

let open = false
const subs = new Set<() => void>()
let host: HTMLDivElement | null = null

export function toggleHelp(next = !open) {
  if (!host) {
    host = document.createElement('div')
    createRoot(host).render(<PadHelp />)
  }
  // Last in <body>, so it's above any dialog that's open.
  if (next) document.body.appendChild(host)
  open = next
  subs.forEach((f) => f())
}

/** Sony's vendor id, or the names its pads report. */
function playStation(): boolean {
  const pads = typeof navigator.getGamepads === 'function' ? navigator.getGamepads() : []
  return [...pads].some((p) => p?.connected && /054c|dualshock|dualsense|wireless controller|playstation/i.test(p.id))
}

type Row = [string, string, Key]

function PadHelp() {
  const on = useSyncExternalStore(
    (cb) => {
      subs.add(cb)
      return () => subs.delete(cb)
    },
    () => open
  )
  if (!on) return null
  const ps = playStation()
  const rows: Row[] = [
    ['✚', 'L', 'pad.help.move'],
    [ps ? '✕' : 'A', '', 'pad.help.select'],
    [ps ? '○' : 'B', '', 'pad.help.back'],
    [ps ? 'L1' : 'LB', ps ? 'R1' : 'RB', 'pad.help.pages'],
    [ps ? 'L2' : 'LT', ps ? 'R2' : 'RT', 'pad.help.tabs'],
    [ps ? '□' : 'X', '', 'pad.help.menu'],
    [ps ? '△' : 'Y', '', 'pad.help.search'],
    ['R', '', 'pad.help.scroll'],
    [ps ? 'Options' : '☰', '', 'pad.help.home'],
    [ps ? 'Share' : '⧉', '', 'pad.help.help']
  ]
  return (
    <div className="backdrop padhelp" onMouseDown={(e) => e.target === e.currentTarget && toggleHelp(false)}>
      <div className="modal padhelp-panel" role="dialog" aria-modal aria-label={t('pad.help.title')}>
        <div className="modal-head">
          <Gamepad2 />
          <div className="grow">
            <h2>{t('pad.help.title')}</h2>
          </div>
          <button className="btn ghost icon sm" onClick={() => toggleHelp(false)} aria-label={t('common.close')}>
            <X />
          </button>
        </div>
        <div className="modal-body">
          <dl className="padhelp-list">
            {rows.map(([a, b, label]) => (
              <div key={label}>
                <dt>
                  <kbd>{a}</kbd>
                  {b && <kbd>{b}</kbd>}
                </dt>
                <dd>{t(label)}</dd>
              </div>
            ))}
          </dl>
        </div>
      </div>
    </div>
  )
}

if (typeof window !== 'undefined') {
  // B (Escape) closes it, before anything under it sees the key.
  window.addEventListener(
    'keydown',
    (e) => {
      if (open && e.key === 'Escape') {
        e.stopImmediatePropagation()
        toggleHelp(false)
      }
    },
    true
  )
}
