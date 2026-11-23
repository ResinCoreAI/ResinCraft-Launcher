// Admin → Chat: friends and chat on the relay (relay chat.rs), as on the dashboard's Chat page: how big chat is,
// reported messages (dismiss, delete, delete and mute the sender), muted players, gifts friends sent each other (take
// one back), how long messages are kept and the gift rules, and the daily copies of chat.db.
import { useEffect, useState } from 'react'
import { Flag, Gift, MicOff, Save, ShieldCheck, Trash2, Undo2 } from 'lucide-react'
import { money } from '../../storeStatus'
import { t, type Key } from '../../i18n'
import { ConfirmModal, Switch } from '../ui'
import { ago, AskModal, Loading, shortId, size, Stat, useAct, useAdmin, when } from './common'
import '../../chat.css'

interface Msg {
  id: number
  sender: string
  at: number
  kind: string
  body: string
  deleted: number | null
}
interface Report {
  id: number
  at: number
  reporter: string
  reporterName: string
  message: number
  sender: string
  senderName: string
  reason: string
  body: string
  context: Msg[]
  state: 'open' | 'handled'
  handledBy: string | null
  outcome: string | null
}
interface Settings {
  enabled: boolean
  keepDays: number
  maxMb: number
  backups: number
  giftCredit: boolean
  giftItems: boolean
  givenSendable: boolean
  dailyLimit: number
  minGift: number
  minAccountDays: number
}
interface ChatAdminData {
  available: boolean
  error?: string
  currency: string
  path: string
  settings: Settings
  stats: { messages: number; today: number; online: number; friendships: number; bytes: number; freeBytes: number; oldest: number; conversations: number; players: number }
  reports: Report[]
  openReports: number
  mutes: { uuid: string; name: string; until: number; reason: string; by: string; at: number }[]
  gifts: { id: string; at: number; sender: string; senderName: string; receiver: string; receiverName: string; kind: 'credit' | 'item'; amount: number; product: string | null; yearly: boolean; reversed: number | null; reversedBy: string | null }[]
  backups: { name: string; bytes: number; at: number }[]
}

const REASON: Record<string, Key> = {
  spam: 'chat.report.spam',
  harassment: 'chat.report.harassment',
  scam: 'chat.report.scam',
  inappropriate: 'chat.report.inappropriate',
  other: 'chat.report.other'
}

export function ChatAdmin() {
  const { data, error, reload } = useAdmin<ChatAdminData>('/api/chat-admin', 30_000)
  const act = useAct(reload)
  const [s, setS] = useState<Settings>()
  const [muting, setMuting] = useState<{ report?: Report } | null>(null)
  const [reversing, setReversing] = useState<ChatAdminData['gifts'][number]>()
  useEffect(() => {
    if (data?.settings) setS(data.settings)
  }, [data?.settings])
  if (!data) return <Loading error={error} />
  if (!data.available) return <p className="hint bad">{data.error || t('servers.admin.chat.unavailable')}</p>
  const cur = data.currency || 'thb'
  const st = data.stats
  const who = (name: string, uuid: string) => (
    <>
      <b>{name || shortId(uuid)}</b> <span className="hint mono">{shortId(uuid)}</span>
    </>
  )
  const giftWhat = (g: ChatAdminData['gifts'][number]) =>
    g.kind === 'credit' ? t('chat.gift.cardCredit', { amount: money(g.amount, cur) }) : `${g.product} · ${t(g.yearly ? 'chat.gift.oneYear' : 'chat.gift.oneMonth')} · ${money(g.amount, cur)}`
  const num = (k: keyof Settings, scale = 1) => (
    <input
      className="input"
      type="number"
      min={0}
      step={scale === 1 ? 1 : 'any'}
      value={s ? Number(s[k]) / scale : ''}
      onChange={(e) => s && setS({ ...s, [k]: Math.round(Number(e.target.value || 0) * scale) })}
    />
  )
  const flag = (k: keyof Settings, label: Key, hint?: Key) => (
    <div className="setting">
      <div className="text">
        <b>{t(label)}</b>
        {hint && <span>{t(hint)}</span>}
      </div>
      <Switch on={!!s?.[k]} onChange={(on) => s && setS({ ...s, [k]: on })} />
    </div>
  )
  const changed = !!s && JSON.stringify(s) !== JSON.stringify(data.settings)
  return (
    <>
      <div className="stats admin-stats">
        <Stat label={t('servers.admin.chat.messages')} value={st.messages.toLocaleString()} />
        <Stat label={t('servers.admin.chat.today')} value={st.today.toLocaleString()} />
        <Stat label={t('servers.admin.chat.online')} value={st.online} />
        <Stat label={t('servers.admin.chat.friendships')} value={st.friendships.toLocaleString()} />
        <Stat label={t('servers.admin.chat.size')} value={size(st.bytes)} />
        <Stat label={t('servers.admin.chat.openReports')} value={data.openReports} tone={data.openReports ? 'warn' : 'ok'} />
      </div>

      <div className="settings-section">
        <h3>
          <Flag className="inline" /> {t('servers.admin.chat.reports')}
        </h3>
        <p className="hint">{t('servers.admin.chat.reportsHint')}</p>
        {data.reports.length === 0 ? (
          <p className="hint">{t('servers.admin.chat.noReports')}</p>
        ) : (
          <div className="card admin-items">
            {data.reports.map((r) => (
              <div key={r.id} className={`setting stack ${r.state === 'open' ? '' : 'faint'}`}>
                <div className="text">
                  <span>
                    {who(r.senderName, r.sender)} · <span className="chip compact warn">{t(REASON[r.reason] ?? 'chat.report.other')}</span> · {t('servers.admin.chat.reportedBy', { name: r.reporterName || shortId(r.reporter), when: ago(r.at) })}
                  </span>
                  <span className="admin-chat-body">{r.body}</span>
                  {r.context.length > 1 && (
                    <details>
                      <summary className="hint">{t('servers.admin.chat.before')}</summary>
                      <div className="admin-chat-context">
                        {r.context.map((m) => (
                          <div key={m.id} className={m.id === r.message ? 'reported' : ''}>
                            <span className="hint">
                              {when(m.at)} · {m.sender === r.sender ? r.senderName || shortId(r.sender) : r.reporterName || shortId(r.reporter)}:
                            </span>{' '}
                            {m.deleted ? <em className="hint">{t('chat.deleted')}</em> : m.kind === 'gift' ? <em>{t('chat.giftLine')}</em> : m.body}
                          </div>
                        ))}
                      </div>
                    </details>
                  )}
                  {r.state !== 'open' && <span className="hint">{t('servers.admin.chat.handled', { outcome: r.outcome ?? '', by: r.handledBy ?? '' })}</span>}
                </div>
                {r.state === 'open' && (
                  <div className="row end">
                    <button className="btn sm ghost" onClick={() => void act('/api/chat-admin/report', { id: r.id, action: 'dismiss' }, 'servers.admin.chat.dismissed')}>
                      <ShieldCheck /> {t('servers.admin.chat.dismiss')}
                    </button>
                    <button className="btn sm" onClick={() => void act('/api/chat-admin/report', { id: r.id, action: 'delete' }, 'servers.admin.chat.deletedMsg')}>
                      <Trash2 /> {t('servers.admin.chat.deleteMsg')}
                    </button>
                    <button className="btn sm danger" onClick={() => setMuting({ report: r })}>
                      <MicOff /> {t('servers.admin.chat.mute')}
                    </button>
                  </div>
                )}
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="settings-section">
        <div className="row">
          <h3 className="grow">
            <MicOff className="inline" /> {t('servers.admin.chat.muted')}
          </h3>
          <button className="btn sm" onClick={() => setMuting({})}>
            {t('servers.admin.chat.mutePlayer')}
          </button>
        </div>
        {data.mutes.length === 0 ? (
          <p className="hint">{t('servers.admin.chat.noMutes')}</p>
        ) : (
          <div className="card admin-items">
            {data.mutes.map((m) => (
              <div key={m.uuid} className="setting">
                <div className="text">
                  <span>{who(m.name, m.uuid)}</span>
                  <span>{t('servers.admin.chat.mutedUntil', { when: when(m.until), reason: m.reason || '—', by: m.by })}</span>
                </div>
                <button className="btn sm" onClick={() => void act('/api/chat-admin/unmute', { uuid: m.uuid }, 'servers.admin.chat.unmuted')}>
                  {t('servers.admin.chat.unmute')}
                </button>
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="settings-section">
        <h3>
          <Gift className="inline" /> {t('servers.admin.chat.gifts')}
        </h3>
        <p className="hint">{t('servers.admin.chat.giftsHint')}</p>
        {data.gifts.length === 0 ? (
          <p className="hint">{t('servers.admin.chat.noGifts')}</p>
        ) : (
          <div className="card admin-items">
            {data.gifts.map((g) => (
              <div key={g.id} className={`setting ${g.reversed ? 'faint' : ''}`}>
                <div className="text">
                  <span>
                    {who(g.senderName, g.sender)} → {who(g.receiverName, g.receiver)}
                  </span>
                  <span>
                    {giftWhat(g)} · {ago(g.at)}
                    {g.reversed ? ` · ${t('servers.admin.chat.takenBack', { by: g.reversedBy ?? '' })}` : ''}
                  </span>
                </div>
                {!g.reversed && (
                  <button className="btn sm danger" onClick={() => setReversing(g)}>
                    <Undo2 /> {t('servers.admin.chat.takeBack')}
                  </button>
                )}
              </div>
            ))}
          </div>
        )}
      </div>

      {s && (
        <div className="settings-section">
          <h3>{t('servers.admin.chat.settings')}</h3>
          <div className="card">
            {flag('enabled', 'servers.admin.chat.enabled', 'servers.admin.chat.enabledHint')}
            <div className="setting stack">
              <div className="field-row">
                <label className="field">
                  <span>{t('servers.admin.chat.keepDays')}</span>
                  {num('keepDays')}
                </label>
                <label className="field">
                  <span>{t('servers.admin.chat.maxMb')}</span>
                  {num('maxMb')}
                </label>
                <label className="field">
                  <span>{t('servers.admin.chat.backups')}</span>
                  {num('backups')}
                </label>
              </div>
            </div>
            {flag('giftCredit', 'servers.admin.chat.giftCredit')}
            {flag('giftItems', 'servers.admin.chat.giftItems')}
            {flag('givenSendable', 'servers.admin.chat.givenSendable', 'servers.admin.chat.givenSendableHint')}
            <div className="setting stack">
              <div className="field-row">
                <label className="field">
                  <span>{t('servers.admin.chat.dailyLimit', { currency: cur.toUpperCase() })}</span>
                  {num('dailyLimit', 100)}
                </label>
                <label className="field">
                  <span>{t('servers.admin.chat.minGift', { currency: cur.toUpperCase() })}</span>
                  {num('minGift', 100)}
                </label>
                <label className="field">
                  <span>{t('servers.admin.chat.minAccountDays')}</span>
                  {num('minAccountDays')}
                </label>
              </div>
            </div>
            <div className="setting">
              <div className="text" />
              <button className="btn sm primary" disabled={!changed} onClick={() => void act('/api/chat-admin/settings', s, 'servers.admin.chat.saved')}>
                <Save /> {t('common.save')}
              </button>
            </div>
          </div>
        </div>
      )}

      <div className="settings-section">
        <div className="row">
          <h3 className="grow">{t('servers.admin.chat.storage')}</h3>
          <button className="btn sm" onClick={() => void act('/api/chat-admin/backup', {}, 'servers.admin.chat.copyMade')}>
            {t('servers.admin.chat.copyNow')}
          </button>
        </div>
        <p className="hint">{t('servers.admin.chat.storageHint', { path: data.path })}</p>
        <p className="hint">{st.oldest ? t('servers.admin.chat.oldest', { when: when(st.oldest) }) : t('servers.admin.chat.noMessages')}</p>
        {data.backups.length > 0 && (
          <div className="card admin-items">
            {data.backups.map((b) => (
              <div key={b.name} className="setting">
                <div className="text">
                  <b className="mono">{b.name}</b>
                  <span>
                    {size(b.bytes)} · {when(b.at)}
                  </span>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {muting && (
        <AskModal
          title={muting.report ? t('servers.admin.chat.muteTitle', { name: muting.report.senderName || shortId(muting.report.sender) }) : t('servers.admin.chat.mutePlayer')}
          fields={[
            ...(muting.report ? [] : [{ key: 'player', label: t('servers.admin.chat.player') }]),
            { key: 'days', label: t('servers.admin.chat.days'), type: 'number' as const, value: '7' },
            { key: 'reason', label: t('servers.admin.chat.reason'), value: muting.report ? t(REASON[muting.report.reason] ?? 'chat.report.other') : '' }
          ]}
          confirm={t('servers.admin.chat.mute')}
          danger
          body={muting.report ? <p className="hint">{t('servers.admin.chat.muteBody')}</p> : undefined}
          onSubmit={(v) =>
            muting.report
              ? act('/api/chat-admin/report', { id: muting.report.id, action: 'mute', days: Number(v.days) || 7, reason: v.reason }, 'servers.admin.chat.mutedDone')
              : act('/api/chat-admin/mute', { player: v.player, days: Number(v.days) || 7, reason: v.reason }, 'servers.admin.chat.mutedDone')
          }
          onClose={() => setMuting(null)}
        />
      )}
      {reversing && (
        <ConfirmModal
          title={t('servers.admin.chat.takeBackTitle')}
          body={t('servers.admin.chat.takeBackBody', { what: giftWhat(reversing) })}
          confirm={t('servers.admin.chat.takeBack')}
          onConfirm={async () => {
            await act('/api/chat-admin/gift/reverse', { id: reversing.id }, 'servers.admin.chat.takenBackDone')
            setReversing(undefined)
          }}
          onClose={() => setReversing(undefined)}
        />
      )}
    </>
  )
}
