// The gifts page: every gift that came to you and that you sent (in chats and from the Store), where each stands, and
// Receive on the ones that wait (one by one, or all at once). For when there are too many to find in the chats.
import { useEffect, useState } from 'react'
import { Gift, Inbox as InboxIcon, Loader2, MessageCircle, Undo2 } from 'lucide-react'
import type { ChatGiftInfo, ChatGiftsList } from '@shared/types'
import { chat } from '../../chatState'
import { money, setWalletBalance } from '../../storeStatus'
import { useStore } from '../../store'
import { currentLanguage, t } from '../../i18n'
import { Empty, Modal, Segmented } from '../ui'

type Tab = 'waiting' | 'received' | 'sent'

/** "in 5 hours", "in 3 days": when a waiting gift goes back. */
function backIn(at: number): string {
  const minutes = Math.max(1, Math.ceil((at * 1000 - Date.now()) / 60_000))
  const rtf = new Intl.RelativeTimeFormat(currentLanguage(), { numeric: 'always' })
  if (minutes < 120) return rtf.format(minutes, 'minute')
  if (minutes < 48 * 60) return rtf.format(Math.round(minutes / 60), 'hour')
  return rtf.format(Math.round(minutes / 1440), 'day')
}

const when = (secs: number) => new Date(secs * 1000).toLocaleString(currentLanguage(), { dateStyle: 'medium', timeStyle: 'short' })

export default function GiftsModal({ onClose }: { onClose: () => void }) {
  const { toast, activeAccount } = useStore()
  const [list, setList] = useState<ChatGiftsList>()
  const [error, setError] = useState('')
  const [tab, setTab] = useState<Tab>('waiting')
  const [busy, setBusy] = useState<string | null>(null)
  const load = () =>
    chat.gifts().then(
      (l) => {
        setList(l)
        setError('')
      },
      (e) => setError((e as Error).message)
    )
  useEffect(() => {
    void load().then(() => setTab((t0) => t0))
  }, [])
  // Nothing waits: start on what came.
  useEffect(() => {
    if (list && list.waiting === 0 && tab === 'waiting') setTab('received')
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [list])

  const balance = (b: number) => activeAccount?.type === 'msa' && setWalletBalance(activeAccount.uuid, b, list?.currency ?? 'thb')
  const receive = async (g: ChatGiftInfo) => {
    setBusy(g.id)
    try {
      const r = await chat.claimGift(g.id)
      balance(r.balance)
      toast(t('chat.gift.received', { what: r.summary }), 'success')
      await load()
    } catch (e) {
      toast((e as Error).message, 'error')
      await load()
    } finally {
      setBusy(null)
    }
  }
  const receiveAll = async () => {
    setBusy('all')
    try {
      const r = await chat.claimAllGifts()
      balance(r.balance)
      toast(t('chat.gifts.receivedAll', { count: r.received.length }), 'success')
      if (r.failed.length) toast(r.failed[0].error, 'error')
      await load()
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(null)
    }
  }

  const waiting = list?.received.filter((g) => g.state === 'waiting') ?? []
  const shown = !list ? [] : tab === 'waiting' ? waiting : tab === 'received' ? list.received.filter((g) => g.state !== 'waiting') : list.sent
  const row = (g: ChatGiftInfo) => {
    const mine = tab === 'sent'
    const who = mine ? (g.toName ? t('chat.gifts.to', { name: g.toName }) : t('chat.gifts.toGroup')) : t('chat.gifts.from', { name: g.fromName || '?' })
    const state =
      g.state === 'waiting'
        ? t('chat.gifts.waitingBack', { when: backIn(g.expires ?? 0) })
        : g.state === 'received'
          ? t('chat.gifts.received')
          : mine
            ? t('chat.gifts.cameBack')
            : t('chat.gifts.wentBack')
    return (
      <div key={g.id} className={`gifts-row ${g.state}`}>
        <span className="gifts-icon" aria-hidden>
          <Gift />
        </span>
        <span className="gifts-text">
          <b>{g.gift === 'credit' ? money(g.amount, g.currency) : g.summary}</b>
          <span className="hint">
            {who} · {when(g.at)} · {g.inbox ? <InboxIcon size={12} className="inline" aria-label={t('chat.gifts.inInbox')} /> : <MessageCircle size={12} className="inline" aria-label={t('chat.gifts.inChat')} />}
          </span>
        </span>
        <span className={`chip compact gifts-state ${g.state}`}>
          {g.state === 'returned' && <Undo2 size={12} />} {state}
        </span>
        {!mine && g.state === 'waiting' && (
          <button className="btn sm primary" disabled={!!busy} onClick={() => void receive(g)}>
            {busy === g.id ? <Loader2 className="spin" /> : <Gift />} {t('chat.gift.receive')}
          </button>
        )}
      </div>
    )
  }

  return (
    <Modal
      title={t('chat.gifts.title')}
      subtitle={t('chat.gifts.subtitle')}
      wide
      onClose={onClose}
      footer={
        <>
          {waiting.length > 1 && (
            <button className="btn primary" disabled={!!busy} onClick={() => void receiveAll()}>
              {busy === 'all' ? <Loader2 className="spin" /> : <Gift />} {t('chat.gifts.receiveAll', { count: waiting.length })}
            </button>
          )}
          <div className="spacer" />
          <button className="btn ghost" onClick={onClose}>
            {t('common.close')}
          </button>
        </>
      }
    >
      <Segmented<Tab>
        value={tab}
        onChange={setTab}
        options={[
          { value: 'waiting', label: t('chat.gifts.tabWaiting', { count: waiting.length }) },
          { value: 'received', label: t('chat.gifts.tabReceived') },
          { value: 'sent', label: t('chat.gifts.tabSent') }
        ]}
      />
      <div className="gifts-list">
        {error ? (
          <p className="hint bad">{error}</p>
        ) : !list ? (
          <Loader2 className="spin" />
        ) : shown.length === 0 ? (
          <Empty icon={<Gift />} title={tab === 'waiting' ? t('chat.gifts.noneWaiting') : t('chat.gifts.none')} />
        ) : (
          shown.map(row)
        )}
      </div>
    </Modal>
  )
}
