// The emoji picker under the chat's message box: recent ones first, then by group, with a search by name (English
// words, as emoji are named in most pickers). Picking one puts it where the cursor was.
import { useEffect, useMemo, useRef, useState } from 'react'
import { Search, Settings2 } from 'lucide-react'
import type { ChatGroup } from '@shared/types'
import { t, type Key } from '../../i18n'
import { FileImage } from './ChatMedia'
import { myEmoji } from './chatEmoji'

/** "emoji name name …" per line, by group. */
const GROUPS: [Key, string][] = [
  [
    'chat.emoji.smileys',
    `😀 grin smile happy|😃 smile happy|😄 laugh smile|😁 beam grin|😆 laugh lol|😅 sweat laugh|🤣 rofl lol|😂 joy tears lol|🙂 slight smile|🙃 upside down|😉 wink|😊 blush|😇 angel halo|🥰 love hearts|😍 heart eyes love|🤩 star struck|😘 kiss|😋 yum tongue|😛 tongue|😜 wink tongue|🤪 zany crazy|😝 squint tongue|🤑 money|🤗 hug|🤭 oops giggle|🤫 shush quiet|🤔 think hmm|🤐 zip|🤨 raised eyebrow|😐 neutral|😑 expressionless|😶 silent|😏 smirk|😒 unamused|🙄 eye roll|😬 grimace|😌 relieved|😔 pensive|😪 sleepy|🤤 drool|😴 sleep zzz|😷 mask sick|🤒 sick|🤕 hurt|🤢 nauseous|🤮 vomit|🥵 hot|🥶 cold freeze|🥴 woozy|😵 dizzy|🤯 mind blown|🤠 cowboy|🥳 party|😎 cool sunglasses|🤓 nerd|🧐 monocle|😕 confused|😟 worried|🙁 frown|😮 open mouth wow|😯 hushed|😲 astonished|😳 flushed|🥺 pleading|😦 frowning|😧 anguished|😨 fearful|😰 anxious|😥 sad relieved|😢 cry|😭 sob cry|😱 scream|😖 confounded|😣 persevere|😞 disappointed|😓 downcast sweat|😩 weary|😫 tired|🥱 yawn|😤 triumph huff|😡 angry rage|😠 angry|🤬 swear|😈 devil smile|👿 imp angry|💀 skull dead|☠️ skull crossbones|💩 poop|🤡 clown|👻 ghost|👽 alien|🤖 robot`
  ],
  [
    'chat.emoji.people',
    `👋 wave hi|🤚 raised back hand|✋ hand stop|🖖 vulcan|👌 ok|🤌 pinched|✌️ peace victory|🤞 crossed fingers luck|🤟 love you|🤘 rock|🤙 call me|👈 left|👉 right|👆 up|👇 down|☝️ point up|👍 thumbs up yes|👎 thumbs down no|✊ fist|👊 punch|🤛 left fist|🤜 right fist|👏 clap|🙌 raise hands|👐 open hands|🤲 palms up|🤝 handshake deal|🙏 pray thanks please|💪 strong muscle|🧠 brain|👀 eyes look|👁️ eye|👅 tongue|👄 mouth|🫡 salute|🫶 heart hands|🙋 raise hand|🤷 shrug|🤦 facepalm|🙇 bow|💃 dance|🕺 dance|🏃 run|🚶 walk`
  ],
  [
    'chat.emoji.nature',
    `🐶 dog|🐱 cat|🐭 mouse|🐰 rabbit|🦊 fox|🐻 bear|🐼 panda|🐨 koala|🐯 tiger|🦁 lion|🐮 cow|🐷 pig|🐸 frog|🐵 monkey|🐔 chicken|🐧 penguin|🐦 bird|🐤 chick|🦆 duck|🦉 owl|🐺 wolf|🐗 boar|🐴 horse|🦄 unicorn|🐝 bee|🐛 bug|🦋 butterfly|🐌 snail|🐞 ladybug|🐢 turtle|🐍 snake|🐙 octopus|🦑 squid|🐠 fish|🐬 dolphin|🐳 whale|🦈 shark|🐊 crocodile|🐘 elephant|🐑 sheep|🦇 bat|🕷️ spider|🌵 cactus|🌲 tree evergreen|🌳 tree|🌴 palm|🍀 clover luck|🍁 leaf maple|🍄 mushroom|🌷 tulip|🌹 rose|🌻 sunflower|🌸 blossom|🌞 sun|🌙 moon|⭐ star|🌟 glowing star|✨ sparkles|⚡ lightning|🔥 fire lit|🌈 rainbow|☁️ cloud|🌧️ rain|⛈️ storm|❄️ snow|💧 water drop|🌊 wave ocean`
  ],
  [
    'chat.emoji.food',
    `🍎 apple|🍏 green apple|🍊 orange|🍋 lemon|🍌 banana|🍉 watermelon|🍇 grapes|🍓 strawberry|🍒 cherry|🍑 peach|🥭 mango|🍍 pineapple|🥥 coconut|🥝 kiwi|🍅 tomato|🥕 carrot|🌽 corn|🥔 potato|🍞 bread|🧀 cheese|🍖 meat|🍗 chicken leg|🥩 steak|🥓 bacon|🍔 burger|🍟 fries|🍕 pizza|🌭 hotdog|🌮 taco|🍣 sushi|🍜 noodles ramen|🍚 rice|🍛 curry|🍪 cookie|🎂 cake birthday|🍰 cake|🧁 cupcake|🍫 chocolate|🍬 candy|🍩 donut|🍿 popcorn|☕ coffee|🍵 tea|🧋 bubble tea|🥤 drink soda|🍺 beer|🍷 wine|🥛 milk`
  ],
  [
    'chat.emoji.activity',
    `⚽ soccer football|🏀 basketball|🏈 football|⚾ baseball|🎾 tennis|🏐 volleyball|🎱 billiards|🏓 ping pong|🏸 badminton|🥊 boxing|🎮 game controller|🕹️ joystick|🎲 dice|🧩 puzzle|♟️ chess|🎯 target bullseye|🎳 bowling|🏆 trophy win|🥇 gold first|🥈 silver|🥉 bronze|🏅 medal|🎉 party tada|🎊 confetti|🎁 gift present|🎈 balloon|🎄 christmas tree|🎃 pumpkin halloween|🎆 fireworks|🎵 music note|🎶 music|🎧 headphones|🎤 microphone|🎸 guitar|🎹 piano|🥁 drum|📷 camera|💻 laptop|🖥️ computer|⌨️ keyboard|🖱️ mouse|📱 phone|💡 idea bulb|🔦 flashlight|🗡️ sword|⚔️ swords fight|🛡️ shield|🏹 bow arrow|⛏️ pickaxe mine|🪓 axe|🔨 hammer|🧱 brick|🪵 wood|💎 diamond gem|🪙 coin|💰 money bag|🧪 potion|📦 package chest|🗺️ map|🧭 compass|🏠 house home|🏰 castle|⛺ tent|🚀 rocket|✈️ plane|🚗 car|⛵ boat`
  ],
  [
    'chat.emoji.symbols',
    `❤️ heart love red|🧡 orange heart|💛 yellow heart|💚 green heart|💙 blue heart|💜 purple heart|🖤 black heart|🤍 white heart|💔 broken heart|💕 two hearts|💞 hearts|💖 sparkling heart|💯 hundred|💢 anger|💥 boom|💫 dizzy|💬 speech|💭 thought|💤 zzz|✅ check yes|☑️ checkbox|✔️ check|❌ cross no|❗ exclamation|❓ question|⚠️ warning|🚫 no prohibited|⛔ no entry|🔴 red circle|🟢 green circle|🔵 blue circle|🟡 yellow circle|⬆️ up|⬇️ down|➡️ right|⬅️ left|🔄 refresh|➕ plus|➖ minus|✖️ times|💲 dollar|🆗 ok|🆕 new|🆒 cool|🔝 top|⏰ alarm|⌛ hourglass|🔔 bell|🔒 lock|🔑 key|📌 pin|📎 clip|✏️ pencil|📝 memo note|📅 calendar|🗓️ calendar`
  ]
]

type Emoji = { e: string; words: string }
const ALL: { group: Key; list: Emoji[] }[] = GROUPS.map(([group, text]) => ({
  group,
  list: text.split('|').map((x) => {
    const [e, ...words] = x.trim().split(' ')
    return { e, words: words.join(' ') }
  })
}))
const RECENT_KEY = 'resin.chat.recentEmoji'

function recent(): string[] {
  try {
    return (JSON.parse(localStorage.getItem(RECENT_KEY) ?? '[]') as string[]).slice(0, 24)
  } catch {
    return []
  }
}

/**
 * `groups`: every group's emoji shows (they work in every chat), the open chat's group (`here`) first. `onPick` gets a
 * group emoji as `:name:` with its file. `onManage`: the emoji manager.
 */
export default function EmojiPicker({ onPick, onClose, groups = [], here, onManage }: { onPick: (emoji: string, file?: string) => void; onClose: () => void; groups?: ChatGroup[]; here?: string; onManage?: () => void }) {
  const [query, setQuery] = useState('')
  const box = useRef<HTMLDivElement>(null)
  const recents = useMemo(recent, [])
  useEffect(() => {
    const away = (e: MouseEvent) => {
      if (box.current && !box.current.contains(e.target as Node) && !(e.target as HTMLElement).closest('.chat-emoji-btn')) onClose()
    }
    const esc = (e: KeyboardEvent) => e.key === 'Escape' && onClose()
    document.addEventListener('mousedown', away)
    document.addEventListener('keydown', esc)
    return () => {
      document.removeEventListener('mousedown', away)
      document.removeEventListener('keydown', esc)
    }
  }, [onClose])
  const pick = (e: string) => {
    try {
      localStorage.setItem(RECENT_KEY, JSON.stringify([e, ...recent().filter((x) => x !== e)].slice(0, 24)))
    } catch {}
    onPick(e)
  }
  const q = query.trim().toLowerCase()
  const found = q ? ALL.flatMap((g) => g.list).filter((x) => x.words.includes(q)) : []
  const custom = myEmoji(groups, here)
  const customFound = q ? custom.filter((e) => e.name.includes(q.replace(/:/g, ''))) : []
  // By group, in the order myEmoji gives (the open group first).
  const sets = groups
    .filter((g) => g.emoji?.length)
    .sort((a, b) => Number(b.conv === here) - Number(a.conv === here))
  const customGrid = (list: typeof custom) => (
    <div className="ep-grid">
      {list.map((e) => (
        <button key={`${e.conv}:${e.name}`} type="button" className="ep-emoji" title={e.conv === here ? `:${e.name}:` : `:${e.name}: · ${e.group}`} onClick={() => onPick(`:${e.name}:`, e.file)}>
          <FileImage id={e.file} alt={`:${e.name}:`} className="ct-emoji" />
        </button>
      ))}
    </div>
  )
  const grid = (list: string[]) => (
    <div className="ep-grid">
      {list.map((e) => (
        <button key={e} type="button" className="ep-emoji" onClick={() => pick(e)} title={ALL.flatMap((g) => g.list).find((x) => x.e === e)?.words.split(' ')[0]}>
          {e}
        </button>
      ))}
    </div>
  )
  return (
    <div className="chat-emoji" ref={box} role="dialog" aria-label={t('chat.emoji.title')}>
      <div className="ep-search">
        <Search aria-hidden />
        <input className="input" autoFocus placeholder={t('chat.emoji.search')} value={query} onChange={(e) => setQuery(e.target.value)} />
        {onManage && (
          <button type="button" className="btn ghost icon sm" title={t('chat.emoji.manage')} aria-label={t('chat.emoji.manage')} onClick={onManage}>
            <Settings2 />
          </button>
        )}
      </div>
      <div className="ep-body">
        {q ? (
          found.length || customFound.length ? (
            <>
              {customFound.length > 0 && customGrid(customFound)}
              {found.length > 0 && grid(found.map((x) => x.e))}
            </>
          ) : (
            <p className="hint ep-none">{t('chat.emoji.none')}</p>
          )
        ) : (
          <>
            {sets.map((g) => (
              <div key={g.conv}>
                <h5>{g.conv === here ? t('chat.emoji.group') : g.name}</h5>
                {customGrid(custom.filter((e) => e.conv === g.conv))}
              </div>
            ))}
            {recents.length > 0 && (
              <>
                <h5>{t('chat.emoji.recent')}</h5>
                {grid(recents)}
              </>
            )}
            {ALL.map((g) => (
              <div key={g.group}>
                <h5>{t(g.group)}</h5>
                {grid(g.list.map((x) => x.e))}
              </div>
            ))}
          </>
        )}
      </div>
    </div>
  )
}
