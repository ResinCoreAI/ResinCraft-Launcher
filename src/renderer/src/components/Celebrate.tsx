// "You're a tester now": the pop-up for an invitation to the Test or Dev build (the relay keeps it until this launcher
// shows it, inboxState.ts), and for joining the Beta channel in Settings → Updates. The same news is in the Inbox,
// and in the player's email when they linked one.
import type { ReactNode } from 'react'
import { Code2, FlaskConical, Sparkles } from 'lucide-react'
import { useStore } from '../store'
import { t, type Key } from '../i18n'
import { celebrated, useCelebrations, type Celebration } from '../inboxState'
import { Modal } from './ui'
import '../celebrate.css'

const LOOK: Record<Celebration['kind'], { icon: ReactNode; title: Key; body: Key; go?: Key }> = {
  tester: { icon: <FlaskConical />, title: 'celebrate.tester.title', body: 'celebrate.tester.body', go: 'celebrate.tester.go' },
  developer: { icon: <Code2 />, title: 'celebrate.developer.title', body: 'celebrate.developer.body', go: 'celebrate.developer.go' },
  beta: { icon: <Sparkles />, title: 'celebrate.beta.title', body: 'celebrate.beta.body' }
}

export function Celebrate() {
  const list = useCelebrations()
  const { go } = useStore()
  const c = list[0]
  if (!c) return null
  const look = LOOK[c.kind]
  const close = () => celebrated(c)
  return (
    <Modal
      title={t(look.title)}
      onClose={close}
      footer={
        look.go ? (
          <>
            <button className="btn ghost" onClick={close}>
              {t('celebrate.later')}
            </button>
            <button
              className="btn primary"
              onClick={() => {
                close()
                go({ page: 'settings', section: 'updates' })
              }}
            >
              {t(look.go)}
            </button>
          </>
        ) : (
          <button className="btn primary" onClick={close}>
            {t('celebrate.gotIt')}
          </button>
        )
      }
    >
      <div className={`celebrate celebrate-${c.kind}`}>
        <div className="celebrate-badge" aria-hidden>
          {look.icon}
        </div>
        <p className="celebrate-lead">{t('celebrate.congrats')}</p>
        <p>{t(look.body, { by: c.by || t('celebrate.team') })}</p>
        <p className="hint">{t('celebrate.inbox')}</p>
      </div>
    </Modal>
  )
}
