// The web console's entry (console.html, built with vite.console.config.ts): the launcher's own styles and server
// pages, with the relay as the engine (console-api.ts).
import { StrictMode, useEffect } from 'react'
import { createRoot } from 'react-dom/client'
import ConsoleApp from './ConsoleApp'
import { restoreLanguage, setLanguage, useLanguage } from '../i18n'
import { StoreProvider, useStore } from '../store'
import '../fonts'
import '../styles.css'
import '../playful.css'
import '../i18n/fonts.css'
import './console.css'

function Localized() {
  const { settings } = useStore()
  const lang = useLanguage()
  useEffect(() => {
    if (settings) void setLanguage(settings.language)
  }, [settings?.language])
  return <ConsoleApp key={lang} />
}

document.documentElement.classList.add('web-console')

void restoreLanguage().finally(() =>
  createRoot(document.getElementById('root')!).render(
    <StrictMode>
      <StoreProvider>
        <Localized />
      </StoreProvider>
    </StrictMode>
  )
)
