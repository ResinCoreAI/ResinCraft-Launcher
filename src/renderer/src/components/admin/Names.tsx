// Admin → Names: every reserved relay name, its owner and domain; rename, release, give a name, reserved words.
import { useState } from 'react'
import { Globe, Pencil, Plus, Search, Tag, Trash2, Unlink, X } from 'lucide-react'
import { t, type Key } from '../../i18n'
import { ConfirmModal, Empty, MoreButton } from '../ui'
import { account, ago, AskModal, Loading, shortId, useAct, useAdmin } from './common'

interface NameRow {
  name: string
  owner: string
  ownerName?: string | null
  created: number
  lastUsed?: number | null
  online: boolean
  assigned: boolean
  domain: null | {
    domain: string
    state: 'verified' | 'pending' | 'misconfigured'
    routeOk: boolean
    https: null | { serving: boolean; expires?: number | null; error?: string | null }
  }
}
interface Names {
  total: number
  names: NameRow[]
  reserved: string[]
  builtinReserved: string[]
}

const DOMAIN_TONE = { verified: 'success', pending: 'warn', misconfigured: 'danger' } as const

export function NamesAdmin() {
  const [q, setQ] = useState('')
  const [query, setQuery] = useState('')
  const { data, error, reload } = useAdmin<Names>(`/api/names${query ? `?q=${encodeURIComponent(query)}` : ''}`)
  const act = useAct(reload)
  const [renaming, setRenaming] = useState<NameRow>()
  const [releasing, setReleasing] = useState<NameRow>()
  const [giving, setGiving] = useState(false)
  const [word, setWord] = useState('')
  return (
    <>
      <div className="toolbar">
        <form
          className="search"
          onSubmit={(e) => {
            e.preventDefault()
            setQuery(q.trim().toLowerCase())
          }}
        >
          <Search />
          <input className="input" placeholder={t('servers.admin.relay.searchNames')} value={q} onChange={(e) => setQ(e.target.value)} />
        </form>
        <span className="grow" />
        <button className="btn sm primary" onClick={() => setGiving(true)}>
          <Plus /> {t('servers.admin.relay.giveName')}
        </button>
      </div>
      {!data ? (
        <Loading error={error} />
      ) : data.names.length === 0 ? (
        <Empty icon={<Tag />} title={t('servers.admin.relay.noNames')} />
      ) : (
        <>
          <p className="hint">{t('servers.admin.relay.namesCount', { shown: data.names.length, total: data.total })}</p>
          <div className="card admin-items">
            {data.names.map((n) => (
              <div key={n.name} className="setting admin-item">
                <div className="text">
                  <b>
                    {n.name}
                    {n.online && <span className="chip compact success">{t('servers.admin.relay.online')}</span>}
                    {n.assigned && <span className="chip compact">{t('servers.admin.relay.given')}</span>}
                  </b>
                  <span>
                    {n.ownerName ?? shortId(n.owner)} · {t('servers.admin.relay.reserved', { when: ago(n.created) })} · {t('servers.admin.relay.lastOnline', { when: ago(n.lastUsed) })}
                  </span>
                  {n.domain && (
                    <span className="hint">
                      <Globe className="inline" /> {n.domain.domain}{' '}
                      <span className={`chip compact ${DOMAIN_TONE[n.domain.state]}`}>{t(`servers.admin.relay.domain.${n.domain.state}` as Key)}</span>
                      {n.domain.https?.serving && <span className="chip compact success">HTTPS</span>}
                      {n.domain.https?.error && <span className="chip compact danger" title={n.domain.https.error}>HTTPS</span>}
                    </span>
                  )}
                </div>
                <MoreButton
                  items={() => [
                    { label: t('servers.admin.relay.rename'), icon: <Pencil />, onSelect: () => setRenaming(n) },
                    !!n.domain && { label: t('servers.admin.relay.removeDomain'), icon: <Unlink />, onSelect: () => void act('/api/names/remove-domain', { name: n.name }, 'servers.admin.relay.done') },
                    '-',
                    { label: t('servers.admin.relay.release'), icon: <Trash2 />, danger: true, onSelect: () => setReleasing(n) }
                  ]}
                />
              </div>
            ))}
          </div>
          <div className="settings-section">
            <h3>{t('servers.admin.relay.reservedWords')}</h3>
            <div className="card">
              <div className="setting stack">
                <div className="tag-picker">
                  {data.reserved.map((w) => (
                    <button key={w} className="tag-toggle on" title={t('servers.admin.relay.removeWord')} onClick={() => void act('/api/reserved/remove', { word: w })}>
                      {w} <X className="inline" />
                    </button>
                  ))}
                </div>
                <form
                  className="row"
                  onSubmit={(e) => {
                    e.preventDefault()
                    if (word.trim()) void act('/api/reserved/add', { word: word.trim().toLowerCase() }).then((ok) => ok && setWord(''))
                  }}
                >
                  <input className="input" value={word} placeholder={t('servers.admin.relay.addWord')} onChange={(e) => setWord(e.target.value)} />
                  <button className="btn sm" disabled={!word.trim()}>
                    <Plus /> {t('servers.list.field.addTag')}
                  </button>
                </form>
                <span className="hint">{t('servers.admin.relay.builtinWords', { count: data.builtinReserved.length })}</span>
              </div>
            </div>
          </div>
        </>
      )}
      {renaming && (
        <AskModal
          title={t('servers.admin.relay.renameTitle', { name: renaming.name })}
          fields={[{ key: 'to', label: t('servers.admin.relay.newName'), value: renaming.name, hint: t('servers.admin.relay.nameRule') }]}
          confirm={t('servers.admin.relay.rename')}
          onSubmit={(v) => act('/api/names/rename', { name: renaming.name, to: v.to.trim().toLowerCase() }, 'servers.admin.relay.done')}
          onClose={() => setRenaming(undefined)}
        />
      )}
      {giving && (
        <AskModal
          title={t('servers.admin.relay.giveName')}
          fields={[
            { key: 'name', label: t('servers.admin.relay.name'), hint: t('servers.admin.relay.nameRule') },
            { key: 'who', label: t('servers.admin.relay.account'), placeholder: t('servers.admin.relay.accountPlaceholder') }
          ]}
          confirm={t('servers.admin.relay.give')}
          onSubmit={(v) => {
            const who = account(v.who)
            return act('/api/names/assign', { name: v.name.trim().toLowerCase(), ...('uuid' in who ? who : { account: who.name }) }, 'servers.admin.relay.done')
          }}
          onClose={() => setGiving(false)}
        />
      )}
      {releasing && (
        <ConfirmModal
          title={t('servers.admin.relay.releaseTitle', { name: releasing.name })}
          body={t('servers.admin.relay.releaseBody')}
          confirm={t('servers.admin.relay.release')}
          onConfirm={async () => void (await act('/api/names/release', { name: releasing.name }, 'servers.admin.relay.done'))}
          onClose={() => setReleasing(undefined)}
        />
      )}
    </>
  )
}
