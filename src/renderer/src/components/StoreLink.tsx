import { ShoppingBag } from 'lucide-react'
import { t } from '../i18n'
import { useStore } from '../store'
import { useStoreOpen } from '../storeStatus'

/** The Servers tab's pointer to the Store, while ResinCraft Relay sells add-ons and packs. */
export function StoreLink() {
  const { go } = useStore()
  if (!useStoreOpen()) return null
  return (
    <div className="callout" style={{ alignItems: 'center', marginTop: 12 }}>
      <ShoppingBag />
      <div className="grow">{t('servers.store.link')}</div>
      <button className="btn sm primary" onClick={() => go({ page: 'store' })}>
        {t('servers.store.open')}
      </button>
    </div>
  )
}
