// Share a Store item into a chat (a friend's or a group's): it shows there as a card with its price.
import { useState } from 'react'
import { Send } from 'lucide-react'
import { chat, dmId, useChat } from '../../chatState'
import { useStore } from '../../store'
import { t } from '../../i18n'
import { Modal } from '../ui'
import { GroupTile } from './ChatGroups'
import PlayerHead from '../PlayerHead'

export default function ShareModal({ product, name, onClose }: { product: string; name: string; onClose: () => void }) {
  const v = useChat()
  const { toast } = useStore()
  const [to, setTo] = useState('')
  const [note, setNote] = useState('')
  const [busy, setBusy] = useState(false)
  const chats = [...v.groups.map((g) => ({ conv: g.conv, name: g.name, group: true, uuid: '' })), ...v.friends.friends.map((f) => ({ conv: f.conv ?? dmId(v.me, f.uuid), name: f.name, group: false, uuid: f.uuid }))]
  const send = async () => {
    setBusy(true)
    try {
      await chat.share(to, { type: 'store', product }, note.trim())
      toast(t('chat.share.sent', { name: chats.find((c) => c.conv === to)?.name ?? '' }), 'success')
      onClose()
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(false)
    }
  }
  return (
    <Modal
      title={t('chat.share.title', { item: name })}
      onClose={onClose}
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={!to || busy} onClick={() => void send()}>
            <Send /> {t('chat.share.send')}
          </button>
        </>
      }
    >
      <div className="chat-gift">
        {chats.length === 0 ? (
          <p className="hint">{t('chat.share.none')}</p>
        ) : (
          <div className="chat-pick-list" role="radiogroup">
            {chats.map((c) => (
              <label key={c.conv} className="chat-pick-row">
                <input type="radio" name="share-to" checked={to === c.conv} onChange={() => setTo(c.conv)} />
                {c.group ? <GroupTile name={c.name} size={24} /> : <PlayerHead who={c.uuid} name={c.name} size={24} className="chat-head" />}
                <span className="ellipsis">{c.name}</span>
              </label>
            ))}
          </div>
        )}
        <label className="field">
          <span>{t('chat.gift.note')}</span>
          <input className="input" maxLength={500} value={note} onChange={(e) => setNote(e.target.value)} placeholder={t('chat.share.placeholder')} />
        </label>
      </div>
    </Modal>
  )
}
