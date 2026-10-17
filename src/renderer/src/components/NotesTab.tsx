import { useEffect, useRef, useState } from 'react'
import { Check, Loader2, PencilLine } from 'lucide-react'
import type { Instance } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import '../instance-tabs.css'

const SAVE_DELAY = 700

/** A free-text notepad per instance, saved to notes.md as you type. */
export default function NotesTab({ inst }: { inst: Instance }) {
  const { toast } = useStore()
  const [text, setText] = useState<string>()
  const [state, setState] = useState<'saved' | 'dirty' | 'saving'>('saved')
  // The latest unsaved text, so it can be flushed when the tab closes mid-debounce.
  const pending = useRef<string | null>(null)
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined)

  const save = async (id: string, value: string) => {
    setState('saving')
    try {
      await api.notes.set(id, value)
      if (pending.current === value) {
        pending.current = null
        setState('saved')
      }
    } catch (e) {
      setState('dirty')
      toast(`Couldn't save notes: ${(e as Error).message}`, 'error')
    }
  }

  useEffect(() => {
    const id = inst.id
    setText(undefined)
    api.notes
      .get(id)
      .then(setText)
      .catch((e) => {
        setText('')
        toast((e as Error).message, 'error')
      })
    return () => {
      clearTimeout(timer.current)
      if (pending.current != null) void api.notes.set(id, pending.current).catch(() => {})
      pending.current = null
    }
  }, [inst.id, toast])

  const change = (value: string) => {
    setText(value)
    setState('dirty')
    pending.current = value
    clearTimeout(timer.current)
    timer.current = setTimeout(() => void save(inst.id, value), SAVE_DELAY)
  }

  return (
    <div className="it-notes">
      <div className="toolbar">
        <span className="hint grow">
          Anything worth remembering: coordinates, to-dos, which mods need configuring. Saved to notes.md in the instance folder.
        </span>
        <span className="it-save-state" aria-live="polite">
          {state === 'saving' ? (
            <>
              <Loader2 className="spin" /> Saving…
            </>
          ) : state === 'dirty' ? (
            <>
              <PencilLine /> Editing
            </>
          ) : (
            <>
              <Check /> Saved
            </>
          )}
        </span>
      </div>
      {text === undefined ? (
        <div className="skeleton" style={{ height: 420 }} />
      ) : (
        <textarea
          className="input"
          value={text}
          spellCheck
          placeholder={`Notes for ${inst.name}…`}
          aria-label="Instance notes"
          onChange={(e) => change(e.target.value)}
          onKeyDown={(e) => {
            if ((e.ctrlKey || e.metaKey) && e.key === 's') {
              e.preventDefault()
              clearTimeout(timer.current)
              if (pending.current != null) void save(inst.id, pending.current)
            }
          }}
        />
      )}
    </div>
  )
}
