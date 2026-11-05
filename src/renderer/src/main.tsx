import { StrictMode, useEffect } from 'react'
import { createRoot, type Root } from 'react-dom/client'
import App from './App'
import { restoreLanguage, setLanguage, useLanguage } from './i18n'
import { StoreProvider, useStore } from './store'
import { installDesktopBehaviour } from './desktop'
import { api } from './api'
import './fonts'
import './styles.css'
import './playful.css'
import './i18n/fonts.css'

/** Follows the Language setting; the whole UI re-renders in the new language when it changes. */
function Localized() {
  const { settings } = useStore()
  const lang = useLanguage()
  useEffect(() => {
    if (settings) void setLanguage(settings.language)
  }, [settings?.language])
  // Re-mounting on a language change is simplest: every t() call runs again, and it happens rarely.
  return <App key={lang} />
}

/** Once, after the move to https: keeps every entry of the old origin's storage that isn't set here yet. */
async function carryStorage() {
  try {
    const old = await api.app.storageImport()
    for (const [k, v] of Object.entries(old ?? {})) if (localStorage.getItem(k) === null) localStorage.setItem(k, v)
  } catch {
    // Nothing to carry over: the launcher starts as it is.
  }
}

// Hot reload re-runs this file during development; reuse the existing root instead of creating a second one.
const holder = window as unknown as { __resinRoot?: Root }

installDesktopBehaviour()
void carryStorage()
  .then(() => restoreLanguage())
  .finally(() =>
    (holder.__resinRoot ??= createRoot(document.getElementById('root')!)).render(
      <StrictMode>
        <StoreProvider>
          <Localized />
        </StoreProvider>
      </StrictMode>
    )
  )
