// The placeholders a mail's subject and message can use (the relay fills them in for each reader), as buttons that put
// one where the cursor was, as if typed. Admin → Mail's compose form and each Welcome mail; the relay dashboard's Mail
// page has the same. The fields of one form carry `data-tags={group}`; a click goes into the one used last (the
// message, until one was clicked).
import { useEffect, useRef } from 'react'
import { t, type Key } from '../../i18n'

export type MailTag = 'playername' | 'date' | 'month' | 'year' | 'credit'

/** Sent mail and schedules: the name, and when it goes out. */
export const SEND_TAGS: MailTag[] = ['playername', 'date', 'month', 'year']
/** Welcome mail: the name and the mail's own Resin Credit. */
export const WELCOME_TAGS: MailTag[] = ['playername', 'credit']

type Field = HTMLInputElement | HTMLTextAreaElement

/** The relay's three spellings of the reader's name, for previews ("Steve"). */
export const fillName = (text: string, name: string) => text.replaceAll('{playername}', name).replaceAll('{player}', name).replaceAll('{name}', name)

/** Changes a React-controlled field the way typing does, so its onChange runs. */
function typeInto(el: Field, value: string) {
  const proto = el instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype
  Object.getOwnPropertyDescriptor(proto, 'value')?.set?.call(el, value)
  el.dispatchEvent(new Event('input', { bubbles: true }))
}

export function MailTags({ group, tags }: { group: string; tags: MailTag[] }) {
  const last = useRef<Field | null>(null)
  useEffect(() => {
    const on = (e: FocusEvent) => {
      const el = e.target
      if ((el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) && el.dataset.tags === group) last.current = el
    }
    document.addEventListener('focusin', on)
    return () => document.removeEventListener('focusin', on)
  }, [group])
  const put = (tag: string) => {
    const all = [...document.querySelectorAll<Field>(`[data-tags="${CSS.escape(group)}"]`)]
    const el = last.current && last.current.isConnected ? last.current : all[all.length - 1]
    if (!el) return
    const a = el.selectionStart ?? el.value.length
    const b = el.selectionEnd ?? a
    typeInto(el, el.value.slice(0, a) + tag + el.value.slice(b))
    requestAnimationFrame(() => {
      el.focus()
      el.setSelectionRange(a + tag.length, a + tag.length)
    })
  }
  return (
    <div className="field mail-tags">
      <span>{t('servers.admin.mail.tags')}</span>
      <div className="row wrap">
        {tags.map((tag) => (
          <button key={tag} type="button" className="chip tag" title={t(`servers.admin.mail.tag.${tag}` as Key)} onClick={() => put(`{${tag}}`)}>
            {`{${tag}}`}
          </button>
        ))}
      </div>
      <span className="hint">{t('servers.admin.mail.tags.hint', { name: '{name}', player: '{player}', playername: '{playername}' })}</span>
    </div>
  )
}
