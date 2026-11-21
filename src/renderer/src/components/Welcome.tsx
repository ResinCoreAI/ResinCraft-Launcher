import { useEffect, useState, type ReactNode } from 'react'
import { ArrowLeft, ArrowRight, ArrowRightLeft, Check, Compass, LogIn, Monitor, Plus, UserPlus } from 'lucide-react'
import type { Settings } from '@shared/types'
import { Avatar, Logo } from './ui'
import MigrateModal from './MigrateModal'
import { MicrosoftLogin, OfflineModal } from '../pages/Accounts'
import { useStore } from '../store'
import { LANGUAGES, matchLanguage, t } from '../i18n'
import { activeSeason, effectiveTheme, themesNow } from '../themes'
import { closeWelcome, setWelcomeStep, useWelcome } from '../welcomeState'
import '../welcome.css'

const STEPS = 3

/**
 * The first-run welcome: a new player picks a language and a look, signs in, and makes (or brings over) a first
 * instance. It covers the launcher until it is finished or skipped; every step can be skipped.
 */
export default function Welcome() {
  const { step } = useWelcome()
  const { settings, updateSettings, accounts, activeAccount, refreshInstances, setNewInstanceOpen, go, toast } = useStore()
  const [dialog, setDialog] = useState<'microsoft' | 'offline' | 'migrate'>()

  // Escape leaves the welcome, unless a dialog is open (its own or any other: that one closes first).
  useEffect(() => {
    if (dialog) return
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && !document.querySelector('.backdrop') && closeWelcome()
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [dialog])

  if (!settings) return null
  const set = (p: Partial<Settings>) => void updateSettings(p).catch((e) => toast((e as Error).message, 'error'))
  const finish = (then?: () => void) => {
    closeWelcome()
    then?.()
  }

  return (
    <div className="welcome-screen" role="dialog" aria-modal="true" aria-labelledby="welcome-title">
      <div className="welcome-drag" data-tauri-drag-region />
      <div className="welcome-panel" key={step}>
        <div className="welcome-progress" role="img" aria-label={t('welcome.step', { n: step + 1, total: STEPS })}>
          {Array.from({ length: STEPS }, (_, n) => (
            <i key={n} className={n < step ? 'done' : n === step ? 'on' : ''} />
          ))}
        </div>

        {step === 0 && <Hello s={settings} set={set} />}
        {step === 1 && (
          <>
            <h1 id="welcome-title">{t('welcome.account.title')}</h1>
            <p className="welcome-lead">{t('welcome.account.body')}</p>
            {activeAccount ? (
              <div className="welcome-signed">
                <Avatar account={activeAccount} size={44} />
                <b>{t('welcome.account.signedIn', { name: activeAccount.username })}</b>
                <Check />
              </div>
            ) : (
              <div className="welcome-choices">
                <Choice primary icon={<LogIn />} title={t('accounts.signIn')} text={t('home.step.signIn.text')} onClick={() => setDialog('microsoft')} />
                <Choice icon={<UserPlus />} title={t('accounts.offlineProfile')} text={t('welcome.account.offlineHint')} onClick={() => setDialog('offline')} />
              </div>
            )}
          </>
        )}
        {step === 2 && (
          <>
            <h1 id="welcome-title">{t('welcome.first.title')}</h1>
            <p className="welcome-lead">{t('welcome.first.body')}</p>
            <div className="welcome-choices">
              <Choice primary icon={<Plus />} title={t('home.step.create.title')} text={t('home.step.create.text')} onClick={() => finish(() => setNewInstanceOpen(true))} />
              <Choice
                icon={<Compass />}
                title={t('home.instances.installModpack')}
                text={t('welcome.first.modpackHint')}
                onClick={() => finish(() => go({ page: 'discover', kind: 'modpack' }))}
              />
              <Choice icon={<ArrowRightLeft />} title={t('migrate.title')} text={t('welcome.first.migrateHint')} onClick={() => setDialog('migrate')} />
            </div>
          </>
        )}

        <div className="welcome-foot">
          {step > 0 && (
            <button className="btn lg ghost" onClick={() => setWelcomeStep(step - 1)}>
              <ArrowLeft /> {t('common.back')}
            </button>
          )}
          <span className="grow" />
          {step < STEPS - 1 ? (
            <button className={`btn lg ${step === 1 && !accounts.length ? '' : 'primary'}`} onClick={() => setWelcomeStep(step + 1)}>
              {t('common.next')} <ArrowRight />
            </button>
          ) : (
            <button className="btn lg" onClick={() => finish()}>
              {t('welcome.first.later')} <ArrowRight />
            </button>
          )}
        </div>
      </div>

      {dialog === 'microsoft' && <MicrosoftLogin onClose={() => setDialog(undefined)} />}
      {dialog === 'offline' && <OfflineModal onClose={() => setDialog(undefined)} />}
      {dialog === 'migrate' && (
        <MigrateModal
          onClose={() => setDialog(undefined)}
          onImported={() => {
            void refreshInstances()
            finish(() => go({ page: 'library' }))
          }}
        />
      )}
    </div>
  )
}

/** Step one: the greeting, with the two choices that change what the rest of the welcome looks and reads like. */
function Hello({ s, set }: { s: Settings; set: (p: Partial<Settings>) => void }) {
  const systemLanguage = matchLanguage(navigator.languages ?? [navigator.language])
  const season = activeSeason()
  const seasonOn = !!season && s.seasonalThemes !== false && s.seasonOff !== season.key
  const shown = effectiveTheme(s, window.matchMedia('(prefers-color-scheme: light)').matches)
  // As in Settings → Appearance: a pick made during a holiday stays the player's for that holiday.
  const pick = (id: string) => (season && id === season.theme ? set({ seasonOff: '' }) : set({ theme: id, ...(season ? { seasonOff: season.key } : {}) }))
  const followsSystem = !seasonOn && s.theme === 'system'

  return (
    <>
      <Logo className="welcome-logo" />
      <h1 id="welcome-title">{t('home.welcome.title')}</h1>
      <p className="welcome-lead">{t('home.welcome.body')}</p>

      <label className="welcome-field">
        <span>{t('settings.language')}</span>
        <select className="select" value={s.language || 'system'} onChange={(e) => set({ language: e.target.value })}>
          <option value="system">{t('settings.language.system', { name: LANGUAGES.find((l) => l.code === systemLanguage)?.name ?? 'English' })}</option>
          {LANGUAGES.map((l) => (
            <option key={l.code} value={l.code}>
              {l.name}
            </option>
          ))}
        </select>
      </label>

      <div className="welcome-field">
        <span id="welcome-theme">{t('settings.theme')}</span>
        <div className="theme-grid" role="radiogroup" aria-labelledby="welcome-theme">
          <button role="radio" aria-checked={followsSystem} className={`theme-card ${followsSystem ? 'on' : ''}`} onClick={() => pick('system')}>
            <span className="theme-swatch system">
              <Monitor size={16} />
            </span>
            <span className="theme-name">{t('settings.theme.system')}</span>
          </button>
          {themesNow().map((th) => {
            const on = shown === th.id && !followsSystem
            return (
              <button key={th.id} role="radio" aria-checked={on} className={`theme-card ${on ? 'on' : ''}`} onClick={() => pick(th.id)}>
                <span className="theme-swatch" style={{ background: th.swatch[0] }}>
                  {th.swatch.slice(1).map((c) => (
                    <i key={c} style={{ background: c }} />
                  ))}
                </span>
                <span className="theme-name">{t(th.label)}</span>
              </button>
            )
          })}
        </div>
      </div>
    </>
  )
}

function Choice({ icon, title, text, onClick, primary }: { icon: ReactNode; title: string; text: string; onClick: () => void; primary?: boolean }) {
  return (
    <button className={`welcome-choice ${primary ? 'primary' : ''}`} onClick={onClick}>
      <span className="ico">{icon}</span>
      <span className="grow">
        <b>{title}</b>
        <span>{text}</span>
      </span>
      <ArrowRight className="go" />
    </button>
  )
}
