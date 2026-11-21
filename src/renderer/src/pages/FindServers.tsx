import { ServerList } from '../components/ServerList'
import { t } from '../i18n'

/**
 * Find servers: the public server list (servers hosted with ResinCraft and servers players added), as a page of
 * its own in the header, where people look for somewhere to play. It used to be a second view inside Discover.
 */
export default function FindServers() {
  return (
    <div className="page">
      <div className="page-head">
        <div>
          <h1>{t('servers.find.nav')}</h1>
          <p>{t('servers.list.subtitle')}</p>
        </div>
      </div>
      <ServerList />
    </div>
  )
}
