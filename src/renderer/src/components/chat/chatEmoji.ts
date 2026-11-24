// Group emoji in every chat, like Discord's: a group's emoji can be used in any chat by its members. In the message box
// it's `:name:`; what's sent is `<:name:file>` (the picture's file on the relay, which shows it to anyone), in its own
// group too, so renaming an emoji later doesn't break the messages that used it. Older messages with a plain `:name:`
// still show their group's emoji. The relay keeps a `<:name:file>` only when the sender is in a group with that emoji
// (relay-server chat.rs `usable_emoji`).
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

/** Every emoji of the groups you're in, the open group's first. */
export function myEmoji(groups: ChatGroup[], first?: string): GroupEmoji[] {
  const sorted = [...groups].sort((a, b) => Number(b.conv === first) - Number(a.conv === first))
  return sorted.flatMap((g) => (g.emoji ?? []).map((e) => ({ name: e.name, file: e.file, conv: g.conv, group: g.name })))
}

/** A message as plain text (a chat's last line, a notification, editing): `<:name:file>` as `:name:`. */
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
 * The message box's `:name:` into what's sent, `<:name:file>`: the one picked from the picker for this message
 * (`picked`), else this chat's group's own, else the first group of yours with that name. Names no group of yours has
 * stay as typed.
 */
export function withEmojiFiles(text: string, conv: string, groups: ChatGroup[], picked: Map<string, string>): string {
  const here = groups.find((g) => g.conv === conv)
  const own = new Map((here?.emoji ?? []).map((e) => [e.name, e.file]))
  const anywhere = new Map<string, string>()
  for (const e of myEmoji(groups)) if (!anywhere.has(e.name)) anywhere.set(e.name, e.file)
  return outsideCode(text, (part) =>
    // Not after a letter or digit ("12:30:45" isn't an emoji), nor inside a <:name:file> already.
    part.replace(/(?<![<a-z0-9_]):([a-z0-9_]{2,32}):(?!f[0-9a-f]{20}>)/g, (all, name: string) => {
      const file = picked.get(name) ?? own.get(name) ?? anywhere.get(name)
      return file ? `<:${name}:${file}>` : all
    })
  )
}

/** A message back into the message box (Edit): its `<:name:file>` as `:name:`, remembered so the same ones go back. */
export function forEditing(text: string, picked: Map<string, string>): string {
  for (const m of text.matchAll(EMOJI_TOKEN)) picked.set(m[1], m[2])
  return plainEmoji(text)
}
