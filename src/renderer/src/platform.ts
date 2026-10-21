/**
 * The operating system the launcher runs on, read from the web view's user agent so it's known before the first
 * paint (the engine's `app:info` says the same thing, a moment later).
 */
export type Platform = 'windows' | 'macos' | 'linux'

function detect(): Platform {
  const ua = typeof navigator === 'undefined' ? '' : navigator.userAgent
  if (/Mac OS X|Macintosh/.test(ua)) return 'macos'
  if (/Linux|X11|CrOS/.test(ua) && !/Windows/.test(ua)) return 'linux'
  return 'windows'
}

export const platform: Platform = detect()

/** Minecraft: Bedrock Edition and Minecraft Preview come from the Microsoft Store, so only Windows has them. */
export const hasBedrockApp = platform === 'windows'

/** Mojang's Bedrock Dedicated Server is built for Windows and x64 Linux only. */
export const canHostBedrock = platform === 'windows' || (platform === 'linux' && !/aarch64|arm/i.test(typeof navigator === 'undefined' ? '' : navigator.userAgent))

/**
 * English strings name Windows' Recycle Bin and File Explorer; elsewhere they become the Trash and Finder or the
 * file manager. Translations keep their own wording (most languages already say "trash").
 */
export function localWords(text: string): string {
  if (platform === 'windows') return text
  return text
    .replace(/Recycle Bins?/g, 'Trash')
    .replace(/\bin (File )?Explorer\b/g, platform === 'macos' ? 'in Finder' : 'in the file manager')
    .replace(/\b(File )?Explorer\b/g, platform === 'macos' ? 'Finder' : 'the file manager')
}
