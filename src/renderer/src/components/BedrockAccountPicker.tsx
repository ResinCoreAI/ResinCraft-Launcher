import { useEffect, useRef, useState } from 'react'
import { Check, ExternalLink, Loader2, Play, UserRound } from 'lucide-react'
import type { AccountView, BedrockEditionKind, XboxSignIn } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { t } from '../i18n'
import { Modal } from './ui'

/** How often to look whether the Xbox app has switched accounts, while the player is switching. */
const POLL_MS = 3000

/** The Xbox app's own account, when it isn't one of ResinCraft's. */
const XBOX = 'xbox'

type Step = 'pick' | 'switch' | 'unverified'

/** One open picker: which edition, and how to say it has finished. */
type Request = { edition: BedrockEditionKind; done: () => void }

let open: ((r: Request) => void) | undefined

/**
 * Bedrock's Play. Minecraft for Windows signs in as the Xbox account the Xbox app has for this Windows user, and
 * no launcher can choose it. So Play asks which account to play as: when it is the Xbox app's, the game starts;
 * when it isn't, the player switches in the Xbox app (opened for them) and ResinCraft starts the game once Windows
 * shows the Xbox app on that account. With no Microsoft accounts in ResinCraft there is nothing to pick from, and the
 * game starts at once. Resolves when the game was started or the picker was closed.
 */
export function playBedrock(edition: BedrockEditionKind): Promise<void> {
  if (!open) return api.bedrock.play(edition)
  return new Promise((resolve) => open?.({ edition, done: resolve }))
}

/** Rendered once on the Bedrock page: the picker `playBedrock` opens. */
export function BedrockAccountPicker() {
  const { accounts, toast } = useStore()
  const [request, setRequest] = useState<Request>()
  const [sign, setSign] = useState<XboxSignIn>()
  const [picked, setPicked] = useState<string>()
  const [step, setStep] = useState<Step>('pick')
  const [starting, setStarting] = useState(false)
  const ours = accounts.filter((a) => a.type === 'msa' || a.type === 'bedrock')
  const oursRef = useRef(ours)
  oursRef.current = ours

  const start = async (r: Request) => {
    setStarting(true)
    try {
      await api.bedrock.play(r.edition)
      setRequest(undefined)
      r.done()
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setStarting(false)
    }
  }

  useEffect(() => {
    open = (r) => {
      if (!oursRef.current.length) {
        void api.bedrock
          .play(r.edition)
          .catch((e: Error) => toast(e.message, 'error'))
          .finally(r.done)
        return
      }
      setStep('pick')
      setSign(undefined)
      setPicked(undefined)
      setRequest(r)
      void api.bedrock
        .xboxUser()
        .then((s) => {
          setSign(s)
          setPicked(s.accountId ?? (s.user ? XBOX : oursRef.current.find((a) => a.active)?.id))
        })
        .catch(() => setSign({ unknown: [] }))
    }
    return () => {
      open = undefined
    }
  }, [toast])

  // While switching: look again until the Xbox app is on the chosen account, then start the game.
  useEffect(() => {
    if (!request || step !== 'switch' || !picked) return
    let stopped = false
    const timer = setInterval(() => {
      void api.bedrock
        .xboxUser()
        .then((s) => {
          if (stopped) return
          setSign(s)
          if (s.accountId === picked) {
            stopped = true
            clearInterval(timer)
            void start(request)
          }
        })
        .catch(() => {})
    }, POLL_MS)
    return () => {
      stopped = true
      clearInterval(timer)
    }
    // start reads the latest state through its argument.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [request, step, picked])

  if (!request) return null
  const close = () => {
    setRequest(undefined)
    request.done()
  }
  const chosen = ours.find((a) => a.id === picked)
  const xboxName = sign?.user?.gamertag || t('bedrock.account.unnamed')
  const openXbox = () => void api.bedrock.xboxAccount().catch((e: Error) => toast(e.message, 'error'))

  const next = () => {
    if (!sign || !picked) return
    if (picked === XBOX || picked === sign.accountId) void start(request)
    else if (sign.unknown.includes(picked)) setStep('unverified')
    else {
      setStep('switch')
      openXbox()
    }
  }

  const kind = (a: AccountView) => t(a.type === 'bedrock' ? 'account.kind.bedrock' : 'account.kind.java')

  if (step === 'pick') {
    return (
      <Modal
        title={t('bedrock.account.title')}
        subtitle={t('bedrock.account.subtitle')}
        onClose={close}
        footer={
          <>
            <button className="btn ghost" onClick={close}>
              {t('common.cancel')}
            </button>
            <button className="btn primary" disabled={!sign || !picked || starting} onClick={next}>
              {starting ? <Loader2 className="spin" /> : <Play />}{' '}
              {picked === XBOX || (picked && picked === sign?.accountId) ? t('bedrock.play') : t('bedrock.account.continue')}
            </button>
          </>
        }
      >
        {!sign ? (
          <div className="hint row" style={{ gap: 8 }}>
            <Loader2 className="spin" /> {t('bedrock.account.checking')}
          </div>
        ) : (
          <>
            <p className="muted" style={{ marginTop: 0 }}>
              {sign.user ? t('bedrock.account.xboxIs', { name: xboxName }) : t('bedrock.account.xboxUnknown')}
            </p>
            <div className="card list" role="radiogroup" aria-label={t('bedrock.account.title')}>
              {ours.map((a) => (
                <label key={a.id} className="list-row" style={{ cursor: 'pointer' }}>
                  <input type="radio" name="bedrock-account" checked={picked === a.id} onChange={() => setPicked(a.id)} />
                  <UserRound size={18} className="faint" />
                  <div className="grow" style={{ minWidth: 0 }}>
                    <div className="title ellipsis">{a.username}</div>
                    <div className="file">{kind(a)}</div>
                  </div>
                  {a.id === sign.accountId ? (
                    <span className="chip accent">
                      <Check size={12} /> {t('bedrock.account.inXbox')}
                    </span>
                  ) : sign.unknown.includes(a.id) ? (
                    <span className="chip" title={t('bedrock.account.unknownHint')}>
                      {t('bedrock.account.unknown')}
                    </span>
                  ) : null}
                </label>
              ))}
              {sign.user && !sign.accountId && (
                <label className="list-row" style={{ cursor: 'pointer' }}>
                  <input type="radio" name="bedrock-account" checked={picked === XBOX} onChange={() => setPicked(XBOX)} />
                  <UserRound size={18} className="faint" />
                  <div className="grow" style={{ minWidth: 0 }}>
                    <div className="title ellipsis">{xboxName}</div>
                    <div className="file">{t('bedrock.account.xboxOnly')}</div>
                  </div>
                  <span className="chip accent">
                    <Check size={12} /> {t('bedrock.account.inXbox')}
                  </span>
                </label>
              )}
            </div>
          </>
        )}
      </Modal>
    )
  }

  // Switching in the Xbox app: its steps, and what Windows shows now.
  const name = chosen?.username ?? ''
  return (
    <Modal
      title={t('bedrock.account.switchTitle', { name })}
      subtitle={t('bedrock.account.switchSubtitle')}
      onClose={close}
      footer={
        <>
          <button className="btn ghost" onClick={() => setStep('pick')}>
            {t('common.back')}
          </button>
          <span className="grow" />
          <button className="btn" onClick={openXbox}>
            <ExternalLink /> {t('bedrock.account.openXbox')}
          </button>
          {step === 'unverified' && (
            <button className="btn primary" disabled={starting} onClick={() => void start(request)}>
              {starting ? <Loader2 className="spin" /> : <Play />} {t('bedrock.account.playNow')}
            </button>
          )}
        </>
      }
    >
      <ol className="muted" style={{ margin: 0, paddingInlineStart: 20, display: 'grid', gap: 6 }}>
        <li>{t('bedrock.account.step1')}</li>
        <li>{t('bedrock.account.step2')}</li>
        <li>{t('bedrock.account.step3', { name })}</li>
      </ol>
      <div className="callout" style={{ marginTop: 14, alignItems: 'center' }}>
        {step === 'switch' ? <Loader2 className="spin" /> : <UserRound />}
        <span className="grow">
          {step === 'unverified'
            ? t('bedrock.account.cantCheck', { name })
            : sign?.user
              ? t('bedrock.account.waiting', { now: xboxName, name })
              : t('bedrock.account.waitingUnknown', { name })}
        </span>
      </div>
      <p className="hint" style={{ marginBottom: 0 }}>
        {t('bedrock.account.whole')}
      </p>
    </Modal>
  )
}
