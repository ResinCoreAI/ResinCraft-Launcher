import { useMemo, useState } from 'react'
import { Copy, FileDown, FolderOpen, Layers, Loader2, Play, Plus, Search, Settings2, Square, Trash2 } from 'lucide-react'
import type { Instance } from '@shared/types'
import { api } from '../api'
import { ConfirmModal, Empty, InstanceTile, Menu } from '../components/ui'
import { useInstanceTask, useStore } from '../store'
import { describe, playtime } from '../util'
import { t } from '../i18n'

type Sort = 'recent' | 'name' | 'version' | 'playtime'

export default function Library() {
  const { instances, setNewInstanceOpen, refreshInstances, toast, go } = useStore()
  const [q, setQ] = useState('')
  const [sort, setSort] = useState<Sort>('recent')
  const [importing, setImporting] = useState(false)

  const shown = useMemo(() => {
    const list = instances.filter((i) => `${i.name} ${i.mcVersion} ${i.loader}`.toLowerCase().includes(q.toLowerCase()))
    const cmp: Record<Sort, (a: Instance, b: Instance) => number> = {
      recent: (a, b) => (b.lastPlayed ?? b.createdAt) - (a.lastPlayed ?? a.createdAt),
      name: (a, b) => a.name.localeCompare(b.name),
      version: (a, b) => b.mcVersion.localeCompare(a.mcVersion, undefined, { numeric: true }),
      playtime: (a, b) => b.playtime - a.playtime
    }
    return list.sort(cmp[sort])
  }, [instances, q, sort])

  const importPack = async () => {
    setImporting(true)
    try {
      const inst = await api.content.importModpack()
      if (inst) {
        await refreshInstances()
        toast(t('library.imported', { name: inst.name }))
        go({ page: 'instance', id: inst.id })
      }
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setImporting(false)
    }
  }

  return (
    <div className="page">
      <div className="page-head">
        <div>
          <h1>{t('library.title')}</h1>
          <p>{t('library.subtitle')}</p>
        </div>
        <div className="spacer" />
        <button className="btn" onClick={importPack} disabled={importing}>
          {importing ? <Loader2 className="spin" /> : <FileDown />} {t('library.importModpack')}
        </button>
        <button className="btn primary" onClick={() => setNewInstanceOpen(true)}>
          <Plus /> {t('library.newInstance')}
        </button>
      </div>

      {instances.length === 0 ? (
        <Empty
          icon={<Layers />}
          title={t('library.empty.title')}
          action={
            <div className="row" style={{ justifyContent: 'center' }}>
              <button className="btn primary" onClick={() => setNewInstanceOpen(true)}>
                <Plus /> {t('library.empty.create')}
              </button>
              <button className="btn" onClick={() => go({ page: 'discover', kind: 'modpack' })}>
                {t('library.empty.browse')}
              </button>
            </div>
          }
        >
          {t('library.empty.body')}
        </Empty>
      ) : (
        <>
          <div className="toolbar">
            <div className="search">
              <Search />
              <input className="input" placeholder={t('library.search')} value={q} onChange={(e) => setQ(e.target.value)} />
            </div>
            <select className="select" value={sort} onChange={(e) => setSort(e.target.value as Sort)} aria-label={t('library.sort.label')}>
              <option value="recent">{t('library.sort.recent')}</option>
              <option value="name">{t('library.sort.name')}</option>
              <option value="version">{t('library.sort.version')}</option>
              <option value="playtime">{t('library.sort.playtime')}</option>
            </select>
          </div>
          <div className="grid">
            {shown.map((i) => (
              <InstanceCard key={i.id} inst={i} withMenu />
            ))}
          </div>
          {shown.length === 0 && (
            <Empty icon={<Search />} title={t('library.noMatch.title')}>
              {t('library.noMatch.body')}
            </Empty>
          )}
        </>
      )}
    </div>
  )
}

export function InstanceCard({ inst, withMenu }: { inst: Instance; withMenu?: boolean }) {
  const { go, games, play, stop, refreshInstances, toast } = useStore()
  const task = useInstanceTask(inst.id)
  const [confirm, setConfirm] = useState(false)
  const state = games[inst.id]

  return (
    <>
      <div
        className="icard"
        role="button"
        tabIndex={0}
        onClick={() => go({ page: 'instance', id: inst.id })}
        onKeyDown={(e) => e.key === 'Enter' && go({ page: 'instance', id: inst.id })}
      >
        <InstanceTile inst={inst} wide />
        {state === 'running' && <span className="chip success badge-live">● {t('library.card.running')}</span>}
        <button
          className={`quick ${state === 'running' ? 'running busy' : state === 'preparing' ? 'busy' : ''}`}
          title={state === 'running' ? t('common.stop') : t('common.play')}
          onClick={(e) => {
            e.stopPropagation()
            if (state === 'running') void stop(inst.id)
            else if (state !== 'preparing') void play(inst.id)
          }}
        >
          {state === 'preparing' ? <Loader2 className="spin" /> : state === 'running' ? <Square fill="currentColor" /> : <Play fill="currentColor" />}
        </button>
        <div className="body">
          <div className="row">
            <div className="grow">
              <div className="name ellipsis">{inst.name}</div>
              <div className="sub">
                {state === 'preparing' ? (
                  <span style={{ color: 'var(--accent)' }}>
                    {task?.progress != null ? t('library.card.preparingProgress', { percent: Math.round(task.progress * 100) }) : t('library.card.preparing')}
                  </span>
                ) : (
                  <>
                    <span className="ellipsis">{describe(inst)}</span>
                    <span>·</span>
                    <span>{playtime(inst.playtime)}</span>
                  </>
                )}
              </div>
            </div>
            {withMenu && (
              <Menu
                items={[
                  { label: t('library.card.settings'), icon: <Settings2 />, onClick: () => go({ page: 'instance', id: inst.id, tab: 'settings' }) },
                  { label: t('common.openFolder'), icon: <FolderOpen />, onClick: () => void api.instances.openFolder(inst.id) },
                  {
                    label: t('library.card.duplicate'),
                    icon: <Copy />,
                    onClick: async () => {
                      try {
                        await api.instances.duplicate(inst.id)
                        await refreshInstances()
                        toast(t('library.card.duplicated', { name: inst.name }))
                      } catch (e) {
                        toast((e as Error).message, 'error')
                      }
                    }
                  },
                  { label: t('common.delete'), icon: <Trash2 />, danger: true, divider: true, onClick: () => setConfirm(true) }
                ]}
              />
            )}
          </div>
        </div>
      </div>
      {confirm && <DeleteInstance inst={inst} onClose={() => setConfirm(false)} />}
    </>
  )
}

export function DeleteInstance({ inst, onClose, onDeleted }: { inst: Instance; onClose: () => void; onDeleted?: () => void }) {
  const { refreshInstances, toast } = useStore()
  return (
    <ConfirmModal
      title={t('library.delete.title', { name: inst.name })}
      body={<>{t('library.delete.body')}</>}
      confirm={t('library.delete.confirm')}
      onClose={onClose}
      onConfirm={async () => {
        try {
          await api.instances.remove(inst.id)
          await refreshInstances()
          toast(t('library.delete.done', { name: inst.name }))
          onDeleted?.()
        } catch (e) {
          toast((e as Error).message, 'error')
        }
      }}
    />
  )
}
