import { ShoppingBag } from 'lucide-react'
import RelayPlan from '../components/RelayPlan'
import { t } from '../i18n'

/** The Store: ResinCraft Relay's add-ons and packs, for each signed-in Microsoft account. */
export default function Store() {
  return (
    <div className="page store-page">
      <RelayPlan
        page
        heading={
          <div>
            <h1>{t('servers.store.title')}</h1>
            <p className="hint">{t('servers.store.subtitle')}</p>
          </div>
        }
        empty={
          <>
            <div className="page-head">
              <div>
                <h1>{t('servers.store.title')}</h1>
                <p>{t('servers.store.subtitle')}</p>
              </div>
            </div>
            <div className="callout" style={{ alignItems: 'center' }}>
              <ShoppingBag />
              <p>{t('servers.store.closed')}</p>
            </div>
          </>
        }
      />
    </div>
  )
}
