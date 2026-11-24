// Group emoji in every chat, like Discord's. In the message box an emoji is written
//   :name:            one of this chat's group's own emoji;
//   :groupid|name:    any group's you're in (groupid: the 16 hex digits of its conv, "g:<groupid>"), in any chat;
//   <:name:file>      as messages carry them (editing one brings it back like this);
// and the box shows each one that's right as its picture (ChatComposer). What's sent is always `<:name:file>` (the
// picture's file on the relay, which shows it to anyone), so renaming an emoji later doesn't break messages that used it.
// The relay keeps a `<:name:file>` only when the sender is in a group with that emoji (relay-server chat.rs
// `usable_emoji`). Older messages with a plain `:name:` still show their group's emoji (ChatText).
import type { ChatGroup } from '@shared/types'

export interface GroupEmoji {
  name: string
  file: string
  conv: string
  group: string
}

/** `<:name:file>`: a name of 2 to 32 of a-z 0-9 _, a file `f` and 20 hex digits. */
export const EMOJI_TOKEN = /<:([a-z0-9_]{2,32}):(f[0-9a-f]{20})>/g
export const EMOJI_TOKEN_AT = /^<:([a-z0-9_]{2,32}):(f[0-9a-f]{20})>/

/**
 * Every way an emoji is written in the message box: `<:name:file>`, `:groupid|name:` or `:name:`. Not after a letter or
 * digit ("12:30:45" isn't one).
 */
export const BOX_TOKEN = /<:[a-z0-9_]{2,32}:f[0-9a-f]{20}>|(?<![<a-z0-9_]):(?:[0-9a-f]{16}\|)?[a-z0-9_]{2,32}:/g

/** The id a group's emoji are written with in other chats: its conv without "g:". */
export const groupTag = (conv: string) => conv.replace(/^g:/, '')

/** How an emoji is written in the box of the chat `here`: `:name:` in its own group, else `:groupid|name:`. */
export const emojiText = (e: { name: string; conv: string }, here?: string) => (e.conv === here ? `:${e.name}:` : `:${groupTag(e.conv)}|${e.name}:`)

/** Every emoji of the groups you're in, the open group's first. */
export function myEmoji(groups: ChatGroup[], first?: string): GroupEmoji[] {
  const sorted = [...groups].sort((a, b) => Number(b.conv === first) - Number(a.conv === first))
  return sorted.flatMap((g) => (g.emoji ?? []).map((e) => ({ name: e.name, file: e.file, conv: g.conv, group: g.name })))
}

/** The picture a way of writing an emoji (BOX_TOKEN) stands for in the chat `here`, if it's right. */
export function resolveEmoji(token: string, groups: ChatGroup[], here?: string): string | undefined {
  const sent = EMOJI_TOKEN_AT.exec(token)
  if (sent) return sent[2]
  const m = /^:(?:([0-9a-f]{16})\|)?([a-z0-9_]{2,32}):$/.exec(token)
  if (!m) return undefined
  const conv = m[1] ? `g:${m[1]}` : here
  return groups.find((g) => g.conv === conv)?.emoji?.find((e) => e.name === m[2])?.file
}

/** A message as plain text (a chat's last line, a notification, copying): `<:name:file>` as `:name:`. */
export function plainEmoji(text: string): string {
  return text.replace(EMOJI_TOKEN, ':$1:')
}

/** Runs `f` on the parts of `text` outside `code` and ``` blocks (an emoji written in code stays as typed). */
function outsideCode(text: string, f: (part: string) => string): string {
  return text
    .split(/(```[\s\S]*?```|`[^`\n]+`)/)
    .map((part, i) => (i % 2 ? part : f(part)))
    .join('')
}

/**
 * The message box's text into what's sent: every emoji that's right (BOX_TOKEN, resolveEmoji) as `<:name:file>`.
 * The rest stays as typed (a `:name:` of another group's emoji needs its group id).
 */
export function withEmojiFiles(text: string, conv: string, groups: ChatGroup[]): string {
  return outsideCode(text, (part) =>
    part.replace(BOX_TOKEN, (token) => {
      const file = resolveEmoji(token, groups, conv)
      const name = token.startsWith('<') ? null : token.slice(1, -1).split('|').pop()
      return file && name ? `<:${name}:${file}>` : token
    })
  )
}
