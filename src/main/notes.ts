import { readFile, rm } from 'node:fs/promises'
import { join } from 'node:path'
import { instanceRoot, writeAtomic } from './instancefiles'

// Plain Markdown next to instance.json, so it's readable outside the launcher too.
const notesFile = async (id: string) => join(await instanceRoot(id), 'notes.md')

export async function getNotes(id: string): Promise<string> {
  return readFile(await notesFile(id), 'utf8').catch(() => '')
}

export async function setNotes(id: string, text: string): Promise<void> {
  if (typeof text !== 'string') throw new Error('Notes must be text')
  if (text.length > 1_000_000) throw new Error('Notes are too long')
  const file = await notesFile(id)
  if (!text.trim()) await rm(file, { force: true })
  else await writeAtomic(file, text)
}
