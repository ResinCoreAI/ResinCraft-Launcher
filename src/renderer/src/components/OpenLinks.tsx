import { useEffect, useRef, useState } from 'react'
import { api } from '../api'
import { parseLink } from '../links'
import { useStore } from '../store'
import { t } from '../i18n'

/**
 * resincraft:// links (links.ts): a server opens in Quick join ready to add, a friend's invite code is filled in.
 * Nothing joins or connects by itself: the player confirms there. The one link that acts at once is a desktop
 * shortcut's (resincraft://play/<id>), which only the engine can make: it opens that instance and starts it.
 */
export function OpenLinks() {
  const { go, toast, instances, games, play, ready } = useStore()
  // A shortcut clicked while the launcher is still starting waits here until the instances are loaded.
  const [starting, setStarting] = useState<string>()
  const now = useRef({ instances, games })
  now.current = { instances, games }
  useEffect(() => {
    if (!starting || !ready) return
    setStarting(undefined)
    const inst = now.current.instances.find((i) => i.id === starting)
    if (!inst) return toast(t('links.unknown'), 'error')
    go({ page: 'instance', id: inst.id })
    // Already starting or running: opening its page is all there is to do.
    if (!now.current.games[inst.id]) void play(inst.id)
  }, [starting, ready])
  useEffect(() => {
    const open = (url: string) => {
      const target = parseLink(url)
      if (!target) return toast(t('links.unknown'), 'error')
      if (target.kind === 'play') return setStarting(target.id)
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
