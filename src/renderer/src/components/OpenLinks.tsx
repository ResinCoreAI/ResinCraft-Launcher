import { useEffect } from 'react'
import { api } from '../api'
import { parseLink } from '../links'
import { useStore } from '../store'
import { t } from '../i18n'

/**
 * resincraft:// links (links.ts): a server opens in Quick join ready to add, a friend's invite code is filled in.
 * Nothing joins or connects by itself: the player confirms there.
 */
export function OpenLinks() {
  const { go, toast } = useStore()
  useEffect(() => {
    const open = (url: string) => {
      const target = parseLink(url)
      if (!target) return toast(t('links.unknown'), 'error')
      if (target.kind === 'join') go({ page: 'quickjoin', add: { name: target.name, address: target.address, edition: target.edition } })
      else go({ page: 'quickjoin', friendCode: target.code })
    }
    // Started by a link, then any opened while ResinCraft runs.
    void api.app.pendingLink().then(
      (url) => void (url && open(url)),
      () => {}
    )
    return api.on.openLink((e) => open(e.url))
  }, [go, toast])
  return null
}
