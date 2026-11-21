// How far a starting game has got, read from what it prints (components/LaunchScreen.tsx), shown like the old Forge
// loading screen: a bar for the stage the game is in, and one for the mods it is going through.

/** 0 Java starting · 1 finding mods · 2 starting Minecraft · 3 loading resources (until it says it is ready). */
export type LaunchStage = 0 | 1 | 2 | 3
export const STAGE_COUNT = 4

export interface LaunchProgress {
  stage: LaunchStage
  /** Mods gone through so far (Forge: mod files found; Fabric: the mods it lists as it loads them). */
  mods: number
  /** How many there are: Fabric says so; else the instance's own count of enabled mods (from the caller). */
  modsTotal?: number
  /** The mod, file or texture atlas it is on now. */
  current?: string
  /** Texture atlases built while loading resources: a rough measure of that last stage. */
  atlases: number
  /** Fabric lists its mods in one block after "Loading N mods:": its lines are counted until the block ends. */
  inList: boolean
}

export const startProgress = (modsTotal?: number): LaunchProgress => ({ stage: 0, mods: 0, modsTotal, atlases: 0, inList: false })

/** A log line without its "[time] [thread/LEVEL] [logger/MARKER]:" prefix. */
export function message(text: string): string {
  return text.replace(/^(\[[^\]]*\]\s*)+:?\s*/, '').trim()
}

const STAGE_MODS = /Found mod file|Loading \d+ mods|Scanning mods|Discovering mods|ModLauncher running|Loading Minecraft .* with (Fabric|Quilt)/i
const STAGE_GAME = /ModernFix reached bootstrap|Launching target|Setting user|Datafixer optimizations|Loaded \d+ mixins|Backend library/i
const STAGE_RESOURCES = /Reloading ResourceManager|Created: \d+x\d+x\d+ |Loading \d+ sounds|Resource reload/i
/** Forge and NeoForge: "Found mod file sodium-0.5.jar of type MOD with provider …" (libraries aren't counted). */
const FORGE_MOD = /Found mod file (\S+?)(?:\.jar)? of type MOD\b/i
/** Fabric: "Loading 247 mods:" then a line per mod, "\t- sodium 0.6.0". */
const FABRIC_TOTAL = /Loading (\d+) mods:/i
const FABRIC_ITEM = /^\s*[|\\]?-+\s*([a-z0-9_.+-]+)\s+\S+/i
/** The same list when the log gives it in one entry: "Loading 202 mods: - animatica 0.6.2 - axiom 6.1 …". */
// The mods each one bundles are listed under it with "|--" and "\--", and are counted in the total too.
const FABRIC_ITEMS = /(?:^|\s)[|\\]?-+\s+([a-z0-9_.+-]+)\s+\S+/gi
const ATLAS = /Created: \d+x\d+x\d+ (\S+?)(?:-atlas)?$/i
/** A mod speaking for itself: "[ModernFix] …", "[Controlify] …" (not a thread or logger prefix, those are gone). */
const MOD_TAG = /^\[([A-Za-z][\w .'-]{1,30})\]/

/** A line to show under the bars: the first line of an entry, and a long mod list cut to its count. */
export function shortLine(text: string): string {
  const msg = message(text).split('\n')[0]
  const total = FABRIC_TOTAL.exec(msg)
  return total ? msg.slice(0, total.index + total[0].length - 1) : msg
}

/** The progress after one more line. Never goes back a stage. */
export function readLine(p: LaunchProgress, text: string): LaunchProgress {
  const msg = message(text)
  let next = p
  const at = (stage: LaunchStage) => (stage > next.stage ? { ...next, stage } : next)
  if (STAGE_MODS.test(msg)) next = at(1)
  if (STAGE_GAME.test(msg)) next = at(2)
  if (STAGE_RESOURCES.test(msg)) next = at(3)

  const forge = FORGE_MOD.exec(msg)
  if (forge) next = { ...next, mods: next.mods + 1, current: forge[1] }
  const total = FABRIC_TOTAL.exec(msg)
  if (total) {
    // The list can come in the same entry: then every mod in it has been found at once.
    const listed = [...msg.slice(total.index + total[0].length).matchAll(FABRIC_ITEMS)].length
    next = { ...next, modsTotal: Number(total[1]), inList: listed === 0, mods: listed ? Math.min(listed, Number(total[1])) : next.mods }
  } else if (next.inList) {
    // The raw line, not `msg`: Fabric's list lines have no prefix of their own. The first ordinary log line ends it.
    const item = FABRIC_ITEM.exec(text)
    if (/^\s*\[/.test(text) || next.stage >= 2) next = { ...next, inList: false }
    else if (item) next = { ...next, mods: Math.min(next.mods + 1, next.modsTotal ?? Infinity), current: item[1] }
  }
  // After the list, the mod that is speaking is the one being set up.
  const tag = !forge && !total && next.stage >= 1 ? MOD_TAG.exec(msg) : null
  if (tag) next = { ...next, current: tag[1] }
  const atlas = ATLAS.exec(msg)
  if (atlas) next = { ...next, atlases: next.atlases + 1, current: atlas[1].replace(/^minecraft:textures\/atlas\//, '').replace(/\.png$/, '') }
  return next
}

/** The stage bar: whole stages done, plus how far into this one (the mods counted, the atlases built). */
export function stageFraction(p: LaunchProgress): number {
  let within = 0
  if (p.stage === 1 && p.modsTotal) within = Math.min(1, p.mods / p.modsTotal)
  if (p.stage === 3) within = Math.min(0.9, p.atlases / 8)
  return Math.min(1, (p.stage + within) / STAGE_COUNT)
}
