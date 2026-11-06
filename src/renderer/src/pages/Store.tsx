import { ShoppingBag } from 'lucide-react'
import RelayPlan from '../components/RelayPlan'
import { t } from '../i18n'

/** The Store: ResinCraft Relay's add-ons and packs, for each signed-in Microsoft account. */
export default function Store() {
  return (
    <div className="page store-page">
      <div className="page-head">
        <div>
          <h1>{t('servers.store.title')}</h1>
          <p>{t('servers.store.subtitle')}</p>
        </div>
      </div>
      <RelayPlan
        page
        empty={
          <div className="callout" style={{ alignItems: 'center' }}>
            <ShoppingBag />
            <p>{t('servers.store.closed')}</p>
          </div>
        }
      />
    </div>
  )
}
