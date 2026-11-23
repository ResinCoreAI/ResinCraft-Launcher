// Send a friend a gift from a chat: Resin Credit (what was paid for; given credit only buys things), or a month or a
// year of a Store item paid with your credit. It shows in the chat and in both players' Store history (relay chat.rs).
import { useEffect, useState } from 'react'
import { Gift, Wallet } from 'lucide-react'
import type { ChatGiftOptions } from '@shared/types'
import { chat } from '../../chatState'
import { money, setWalletBalance } from '../../storeStatus'
import { useStore } from '../../store'
import { t } from '../../i18n'
import { Modal, Segmented } from '../ui'

/**
 * `to`/`name`: a friend or group member (from a chat). Without them (from the Store) it asks for a player's name and
 * the gift goes straight to that account, friend or not; `preset` opens it on credit or on a Store item.
 */
export default function GiftModal({
  to,
  anyone,
  name,
  conv,
  preset,
  onClose
}: {
  to?: string
  /** A group gift for whoever takes it first (`conv` is the group). */
  anyone?: boolean
  name?: string
  conv?: string
  preset?: { kind: 'credit' } | { kind: 'item'; product: string }
  onClose: () => void
}) {
  const [toName, setToName] = useState('')
  const byName = !to && !anyone
  const { toast, go, activeAccount } = useStore()
  const [opts, setOpts] = useState<ChatGiftOptions>()
  const [error, setError] = useState('')
  const [kind, setKind] = useState<'credit' | 'item'>(preset?.kind ?? 'credit')
  const [amount, setAmount] = useState('')
  const [product, setProduct] = useState('')
  const [yearly, setYearly] = useState(false)
  const [note, setNote] = useState('')
  const [busy, setBusy] = useState(false)
  useEffect(() => {
    chat.giftOptions().then(
      (o) => {
        setOpts(o)
        if (!o.credit && o.items.length) setKind('item')
        setProduct(preset?.kind === 'item' && o.items.some((i) => i.id === preset.product) ? preset.product : o.items[0]?.id ?? '')
      },
      (e) => setError((e as Error).message)
    )
  }, [])
  const cur = opts?.currency ?? 'thb'
  const minor = Math.round(Number(amount.replace(/,/g, '')) * 100)
  const item = opts?.items.find((i) => i.id === product)
  const price = item ? (yearly ? item.yearly : item.monthly) : 0
  const creditOk = !!opts && minor >= opts.minGift && minor <= opts.sendable
  const itemOk = !!opts && !!item && price > 0 && price <= opts.balance
  const send = async () => {
    if (!opts) return
    setBusy(true)
    setError('')
    try {
      const what = kind === 'credit' ? { kind, amount: minor, note: note.trim() || undefined } : { kind, product, yearly, note: note.trim() || undefined }
      const r = byName ? await chat.giftByName(toName.trim(), what) : await chat.gift(to ?? '', what, conv)
      if (activeAccount?.type === 'msa') setWalletBalance(activeAccount.uuid, r.balance, cur)
      toast(t('chat.gift.sent', { name: byName ? toName.trim() : name ?? '' }), 'success')
      onClose()
    } catch (e) {
      setError((e as Error).message)
    } finally {
      setBusy(false)
    }
  }
  const short = kind === 'credit' ? !!opts && minor > opts.sendable : !!opts && price > opts.balance
  return (
    <Modal
      title={byName ? t('chat.gift.titleStore') : t('chat.gift.title', { name: name ?? '' })}
      subtitle={opts ? t('chat.gift.balance', { amount: money(opts.balance, cur) }) : undefined}
      onClose={onClose}
      footer={
        <>
          {short && (
            <button
              className="btn ghost"
              onClick={() => {
                onClose()
                go({ page: 'store' })
              }}
            >
              <Wallet /> {t('chat.gift.topUp')}
            </button>
          )}
          <div className="spacer" />
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={busy || !opts || !!opts.refused || (byName && !toName.trim()) || (kind === 'credit' ? !creditOk : !itemOk)} onClick={() => void send()}>
            <Gift /> {t('chat.gift.send')}
          </button>
        </>
      }
    >
      {!opts ? (
        error ? <p className="hint bad">{error}</p> : <p className="hint">{t('common.loading')}</p>
      ) : opts.refused ? (
        <p className="hint bad">{opts.refused}</p>
      ) : (
        <div className="chat-gift">
          {byName && (
            <label className="field">
              <span>{t('chat.gift.to')}</span>
              <input className="input" autoFocus maxLength={36} value={toName} onChange={(e) => setToName(e.target.value)} placeholder={t('chat.gift.toNameOrCode')} />
              <span className="hint">{t('chat.gift.toCodeHint')}</span>
            </label>
          )}
          {opts.credit && opts.items.length > 0 && (
            <Segmented
              value={kind}
              onChange={setKind}
              options={[
                { value: 'credit', label: t('chat.gift.credit') },
                { value: 'item', label: t('chat.gift.item') }
              ]}
            />
          )}
          {kind === 'credit' ? (
            <>
              <label className="field">
                <span>{t('chat.gift.amount', { currency: cur.toUpperCase() })}</span>
                <input className="input" inputMode="decimal" autoFocus value={amount} onChange={(e) => setAmount(e.target.value.replace(/[^\d.,]/g, ''))} placeholder={(opts.minGift / 100).toFixed(0)} />
              </label>
              <p className="hint">
                {t('chat.gift.sendable', { amount: money(opts.sendable, cur) })}
                {opts.given > 0 && !opts.givenSendable && <> {t('chat.gift.givenNote', { amount: money(opts.given, cur) })}</>}
                {opts.dailyLeft !== null && <> {t('chat.gift.dailyLeft', { amount: money(opts.dailyLeft, cur) })}</>}
              </p>
              {minor > 0 && minor < opts.minGift && <p className="hint bad">{t('chat.gift.min', { amount: money(opts.minGift, cur) })}</p>}
              {minor > opts.sendable && <p className="hint bad">{t('chat.gift.short', { need: money(minor, cur), have: money(opts.sendable, cur) })}</p>}
            </>
          ) : (
            <>
              <div className="chat-gift-items" role="radiogroup">
                {opts.items.map((i) => (
                  <button key={i.id} type="button" role="radio" aria-checked={i.id === product} className={`chat-gift-item ${i.id === product ? 'on' : ''}`} onClick={() => setProduct(i.id)}>
                    <b>{i.name}</b>
                    <span className="hint">{i.description}</span>
                    <span className="data">
                      {i.monthly > 0 && t('chat.gift.perMonth', { amount: money(i.monthly, cur) })}
                      {i.monthly > 0 && i.yearly > 0 && ' · '}
                      {i.yearly > 0 && t('chat.gift.perYear', { amount: money(i.yearly, cur) })}
                    </span>
                  </button>
                ))}
              </div>
              {item && item.monthly > 0 && item.yearly > 0 && (
                <Segmented
                  value={yearly ? 'year' : 'month'}
                  onChange={(v) => setYearly(v === 'year')}
                  options={[
                    { value: 'month', label: t('chat.gift.oneMonth') },
                    { value: 'year', label: t('chat.gift.oneYear') }
                  ]}
                />
              )}
              {item && <p className="hint">{t('chat.gift.itemCost', { item: item.name, amount: money(price, cur), name })}</p>}
              {item && price > opts.balance && <p className="hint bad">{t('chat.gift.short', { need: money(price, cur), have: money(opts.balance, cur) })}</p>}
            </>
          )}
          <label className="field">
            <span>{t('chat.gift.note')}</span>
            <input className="input" maxLength={200} value={note} onChange={(e) => setNote(e.target.value)} placeholder={t('chat.gift.notePlaceholder')} />
          </label>
          {error && <p className="hint bad">{error}</p>}
        </div>
      )}
    </Modal>
  )
}
