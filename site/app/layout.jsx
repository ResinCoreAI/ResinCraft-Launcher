import localFont from 'next/font/local'
import Header from '@/components/Header'
import Footer from '@/components/Footer'
import Reveal from '@/components/Reveal'
import { IconSprite } from '@/components/Icons'
import { SITE_URL } from '@/lib/site'
import './globals.css'

const jakarta = localFont({
  src: './fonts/plus-jakarta-sans-latin-wght-normal.woff2',
  weight: '200 800',
  variable: '--font-jakarta',
  display: 'swap',
})

const plexMono = localFont({
  src: './fonts/ibm-plex-mono-latin-400-normal.woff2',
  weight: '400',
  variable: '--font-plex-mono',
  display: 'swap',
})

export const metadata = {
  metadataBase: new URL(SITE_URL),
  title: {
    default: 'ResinCraft Launcher – modded Minecraft, all in one place',
    template: '%s – ResinCraft Launcher',
  },
  description:
    'ResinCraft Launcher is a free, open-source Minecraft: Java Edition launcher for Windows: instances, mods and modpacks from five sources, and servers your friends can join.',
  alternates: { canonical: '/' },
  openGraph: {
    type: 'website',
    siteName: 'ResinCraft Launcher',
    title: 'ResinCraft Launcher',
    description: 'Modded Minecraft, all in one place. Free and open source, by Puirkle.',
    images: [{ url: '/assets/og.png', width: 1200, height: 630, alt: 'ResinCraft Launcher' }],
  },
  twitter: { card: 'summary_large_image' },
  icons: {
    icon: [
      { url: '/assets/favicon.ico', sizes: 'any' },
      { url: '/assets/logo.svg', type: 'image/svg+xml' },
    ],
    apple: '/assets/apple-touch-icon.png',
  },
}

export const viewport = {
  themeColor: [
    { media: '(prefers-color-scheme: light)', color: '#f7f6fc' },
    { media: '(prefers-color-scheme: dark)', color: '#0c0a12' },
  ],
}

// Runs before the first paint: applies a saved light/dark choice (so the page never flashes the wrong
// theme) and marks the page as having JavaScript, which turns on the scroll reveal.
const BOOT = `(function(){var d=document.documentElement;try{var t=localStorage.getItem('resincraft.theme');if(t==='light'||t==='dark')d.dataset.theme=t}catch(e){}d.classList.add('js')})()`

export default function RootLayout({ children }) {
  return (
    <html lang="en" className={`${jakarta.variable} ${plexMono.variable}`} suppressHydrationWarning>
      <head>
        <script dangerouslySetInnerHTML={{ __html: BOOT }} />
      </head>
      <body>
        <IconSprite />
        <a className="skip" href="#main">
          Skip to content
        </a>
        <Header />
        <main id="main">{children}</main>
        <Footer />
        <Reveal />
      </body>
    </html>
  )
}
