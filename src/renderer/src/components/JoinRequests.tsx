import { useEffect, useState } from 'react'
import { Check, Copy, ExternalLink, Globe, Loader2, MessageCircle, Trash2, X } from 'lucide-react'
import type { JoinRequestsConfig, RequestsInfo, ServerInfo } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { timeAgo } from '../util'
import { Switch, useCopy } from './ui'

const OFF: JoinRequestsConfig = { enabled: false, web: true, discord: false }

/**
 * Whitelist requests for a hosted server: players ask through the host's Discord bot or the web page at the server's
 * relay address, and the host accepts (puts them on the whitelist) or rejects them here.
 */
export default function JoinRequests({ s, reload, onAccepted }: { s: ServerInfo; reload: () => Promise<void>; onAccepted: () => void }) {
  const { toast } = useStore()
  const copy = useCopy()
  const [info, setInfo] = useState<RequestsInfo>()
  const [token, setToken] = useState('')
  const [busy, setBusy] = useState<string>()
  const cfg = s.joinRequests ?? OFF

  useEffect(() => {
    let live = true
    void api.server.requests(s.id).then((i) => live && setInfo(i))
    const off = api.on.serverRequests((e) => e.serverId === s.id && setInfo(e.info))
    return () => {
      live = false
      off()
    }
  }, [s.id, cfg.enabled, cfg.discord, cfg.web])

  const save = async (patch: Partial<JoinRequestsConfig>) => {
    try {
      await api.server.update(s.id, { joinRequests: { ...cfg, ...patch } })
      await reload()
    } catch (e) {
      toast((e as Error).message, 'error')
    }
  }
  const act = async (key: string, f: () => Promise<RequestsInfo>, done?: string) => {
    setBusy(key)
    try {
      setInfo(await f())
      if (done) toast(done, 'success')
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(undefined)
    }
  }

  const bot = info?.bot
  const waiting = info?.requests ?? []

  return (
    <div className="settings-section join-requests">
      <div className="setting">
        <div className="text">
          <b>Whitelist requests {cfg.enabled && waiting.length > 0 && <span className="chip accent">{waiting.length}</span>}</b>
          <span>Players ask to join with their Minecraft name; you accept or reject them here. Accepting puts them on the whitelist.</span>
        </div>
        <Switch on={cfg.enabled} onChange={(v) => void save({ enabled: v })} label="Whitelist requests" />
      </div>

      {cfg.enabled && (
        <>
          {info && !info.whitelistOn && (
            <div className="hint warn">The whitelist is off, so anyone can join anyway. Turn on white-list in the Properties tab.</div>
          )}

          <div className="jr-ways">
            <label className={`jr-way ${cfg.web ? 'on' : ''}`}>
              <input type="checkbox" checked={cfg.web} onChange={(e) => void save({ web: e.target.checked })} />
              <Globe size={16} />
              <div className="grow">
                <b>Web page</b>
                {info?.webUrl ? (
                  <>
                    <span className="hint">Open while the server is online through ResinCraft Relay.</span>
                    <div className="row wrap" style={{ gap: 6 }}>
                      {[info.webUrl, info.webUrlFallback].filter(Boolean).map((u) => (
                        <button key={u} type="button" className="chip mono" title="Copy" onClick={(e) => (e.preventDefault(), void copy(u!, 'Link copied'))}>
                          {u} <Copy size={12} />
                        </button>
                      ))}
                    </div>
                  </>
                ) : (
                  <span className="hint">Needs ResinCraft Relay: share the server through it (Settings → Internet) to get the page.</span>
                )}
              </div>
            </label>

            <label className={`jr-way ${cfg.discord ? 'on' : ''}`}>
              <input type="checkbox" checked={cfg.discord} onChange={(e) => void save({ discord: e.target.checked })} />
              <MessageCircle size={16} />
              <div className="grow">
                <b>Your Discord bot</b>
                <span className="hint">
                  Players type <code>/whitelist</code> in your Discord server. Runs while ResinCraft is open.
                </span>
              </div>
              {cfg.discord && bot && (
                <span className={`chip ${bot.state === 'online' ? 'success' : bot.state === 'error' ? 'danger' : ''}`}>
                  {bot.state === 'connecting' && <Loader2 size={12} className="spin" />}
                  {bot.state === 'online' ? `Online as ${bot.name ?? 'bot'}` : bot.state === 'connecting' ? 'Connecting' : bot.state === 'error' ? 'Problem' : 'Off'}
                </span>
              )}
            </label>
          </div>

          {cfg.discord && info && (
            <div className="jr-bot">
              {bot?.message && <div className={`hint ${bot.state === 'error' ? 'warn' : ''}`}>{bot.message}</div>}
              {info.hasBotToken ? (
                <div className="row wrap" style={{ gap: 8 }}>
                  {bot?.inviteUrl && (
                    <button className="btn sm primary" onClick={() => void api.app.openExternal(bot.inviteUrl!)}>
                      <ExternalLink /> Add the bot to your Discord
                    </button>
                  )}
                  <button className="btn sm ghost" disabled={busy === 'token'} onClick={() => void act('token', () => api.server.setBotToken(s.id, null), 'Bot token removed')}>
                    <Trash2 /> Remove token
                  </button>
                </div>
              ) : (
                <>
                  <ol className="jr-steps">
                    <li>
                      Open the{' '}
                      <button className="link" onClick={() => void api.app.openExternal('https://discord.com/developers/applications')}>
                        Discord Developer Portal
                      </button>{' '}
                      and make a New Application.
                    </li>
                    <li>In Bot, press Reset Token and copy it.</li>
                    <li>Paste it here. ResinCraft keeps it sealed on this PC and never shows it again.</li>
                  </ol>
                  <form
                    className="row"
                    style={{ gap: 8 }}
                    onSubmit={(e) => {
                      e.preventDefault()
                      if (token.trim()) void act('token', () => api.server.setBotToken(s.id, token.trim()), 'Bot token saved').then(() => setToken(''))
                    }}
                  >
                    <input className="input grow" type="password" autoComplete="off" placeholder="Bot token" value={token} onChange={(e) => setToken(e.target.value)} />
                    <button className="btn primary" disabled={!token.trim() || busy === 'token'}>
                      {busy === 'token' ? <Loader2 className="spin" /> : <Check />} Save
                    </button>
                  </form>
                </>
              )}
            </div>
          )}

          <div className="list jr-list">
            {waiting.length === 0 ? (
              <div className="hint">No requests waiting.</div>
            ) : (
              waiting.map((r) => (
                <div key={r.id} className="list-row">
                  <div className="ico">
                    <img
                      src={`https://mc-heads.net/avatar/${encodeURIComponent(r.username.replace(/^\./, ''))}/40`}
                      alt=""
                      onError={(e) => ((e.target as HTMLImageElement).style.display = 'none')}
                    />
                  </div>
                  <div className="grow">
                    <div className="title">{r.username}</div>
                    <div className="hint">
                      {r.source === 'discord' ? `Discord${r.from ? ` · ${r.from}` : ''}` : 'Web page'} · {timeAgo(r.at)}
                    </div>
                  </div>
                  <button
                    className="btn sm primary"
                    disabled={!!busy}
                    onClick={() =>
                      void act(r.id, () => api.server.requestAccept(s.id, r.id), `${r.username} is on the whitelist`).then(onAccepted)
                    }
                  >
                    {busy === r.id ? <Loader2 className="spin" /> : <Check />} Accept
                  </button>
                  <button className="btn sm" disabled={!!busy} onClick={() => void act(r.id, () => api.server.requestReject(s.id, r.id))}>
                    <X /> Reject
                  </button>
                </div>
              ))
            )}
          </div>
        </>
      )}
    </div>
  )
}
