import { StrictMode, useEffect } from 'react'
import { createRoot, type Root } from 'react-dom/client'
import App from './App'
import { restoreLanguage, setLanguage, useLanguage } from './i18n'
import { StoreProvider, useStore } from './store'
import { installDesktopBehaviour } from './desktop'
// Fonts are bundled with the app (no network): Chakra Petch for display, IBM Plex for text and data.
// Extra subsets cover the shipped languages; Chinese, Japanese and Korean use the system's CJK fonts.
import '@fontsource/chakra-petch/latin-500.css'
import '@fontsource/chakra-petch/latin-600.css'
import '@fontsource/chakra-petch/latin-700.css'
import '@fontsource/chakra-petch/latin-ext-600.css'
import '@fontsource/chakra-petch/vietnamese-600.css'
import '@fontsource/chakra-petch/thai-500.css'
import '@fontsource/chakra-petch/thai-600.css'
import '@fontsource/ibm-plex-sans/latin-400.css'
import '@fontsource/ibm-plex-sans/latin-500.css'
import '@fontsource/ibm-plex-sans/latin-600.css'
import '@fontsource/ibm-plex-sans/latin-ext-400.css'
import '@fontsource/ibm-plex-sans/latin-ext-600.css'
import '@fontsource/ibm-plex-sans/cyrillic-400.css'
import '@fontsource/ibm-plex-sans/cyrillic-500.css'
import '@fontsource/ibm-plex-sans/cyrillic-600.css'
import '@fontsource/ibm-plex-sans/vietnamese-400.css'
import '@fontsource/ibm-plex-sans/vietnamese-600.css'
import '@fontsource/ibm-plex-mono/latin-400.css'
import '@fontsource/ibm-plex-mono/latin-500.css'
import './styles.css'
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

installDesktopBehaviour()

// Hot reload re-runs this file during development; reuse the existing root instead of creating a second one.
const holder = window as unknown as { __resinRoot?: Root }

void restoreLanguage().finally(() =>
  (holder.__resinRoot ??= createRoot(document.getElementById('root')!)).render(
    <StrictMode>
      <StoreProvider>
        <Localized />
      </StoreProvider>
    </StrictMode>
  )
)
