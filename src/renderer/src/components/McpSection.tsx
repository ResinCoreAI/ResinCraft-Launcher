import { useEffect, useState } from 'react'
import { Bot, Copy, RefreshCw } from 'lucide-react'
import type { McpInfo } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { t } from '../i18n'
import { Switch } from './ui'

const DEFAULT_PORT = 47615

/**
 * Settings → AI assistants: the MCP server (mcp.rs) that lets Claude Code, Claude Desktop, Cursor, VS Code and other
 * MCP clients on this PC list instances, start games, read logs and install mods. Off by default; 127.0.0.1 only,
 * with a token.
 */
export function McpSection() {
  const { settings, updateSettings, toast } = useStore()
  const [info, setInfo] = useState<McpInfo>()
  const [show, setShow] = useState(false)
  const on = !!settings?.mcp

  useEffect(() => {
    // The server starts a moment after the switch; ask again so the status line catches up.
    let live = true
    const load = () => void api.app.mcpInfo().then((i) => live && setInfo(i), () => {})
    load()
    const again = setTimeout(load, 800)
    return () => {
      live = false
      clearTimeout(again)
    }
  }, [on, settings?.mcpPort])

  if (!settings) return null

  const copy = (text: string) =>
    void navigator.clipboard.writeText(text).then(
      () => toast(t('settings.mcp.copied')),
      () => {}
    )
  const claude = info ? `claude mcp add --transport http resincraft ${info.url} --header "Authorization: Bearer ${info.token}"` : ''
  const json = info
    ? JSON.stringify({ mcpServers: { resincraft: { type: 'http', url: info.url, headers: { Authorization: `Bearer ${info.token}` } } } }, null, 2)
    : ''
  const hide = (s: string) => (show || !info ? s : s.replace(info.token, '•'.repeat(12)))
  const port = settings.mcpPort || DEFAULT_PORT

  return (
    <section className="settings-section" id="mcp">
      <h3>{t('settings.mcp')}</h3>
      <p>{t('settings.mcp.lead')}</p>
      <div className="card">
        <div className="setting">
          <div className="text">
            <b>{t('settings.mcp.switch')}</b>
            <span>{t('settings.mcp.hint')}</span>
          </div>
          <Switch
            on={on}
            onChange={(v) => void updateSettings({ mcp: v }).catch((e) => toast((e as Error).message, 'error'))}
            label={t('settings.mcp.switch')}
          />
        </div>
        {on && info && (
          <>
            <div className="setting">
              <div className="text">
                <b className="row" style={{ gap: 8 }}>
                  <span className={`remote-dot ${info.state === 'on' ? 'on' : info.state === 'error' ? 'off' : 'wait'}`} aria-hidden />
                  {info.state === 'error' ? t('settings.mcp.error', { port: info.port }) : info.url}
                </b>
                {info.error && <span>{info.error}</span>}
              </div>
              <label className="row" style={{ gap: 6 }}>
                <span className="hint">{t('settings.mcp.port')}</span>
                <input
                  className="input"
                  type="number"
                  min={1024}
                  max={65535}
                  style={{ width: 96 }}
                  defaultValue={port}
                  onBlur={(e) => {
                    const next = Number(e.target.value)
                    if (next >= 1024 && next <= 65535 && next !== port) void updateSettings({ mcpPort: next })
                  }}
                />
              </label>
            </div>
            <div className="setting mcp-setup">
              <div className="text grow">
                <b>
                  <Bot size={15} /> {t('settings.mcp.claude')}
                </b>
                <pre className="mcp-code">{hide(claude)}</pre>
                <b>{t('settings.mcp.json')}</b>
                <pre className="mcp-code">{hide(json)}</pre>
                <span>{t('settings.mcp.tokenHint')}</span>
              </div>
              <div className="mcp-actions">
                <button className="btn sm" onClick={() => copy(claude)}>
                  <Copy size={14} /> {t('settings.mcp.copyClaude')}
                </button>
                <button className="btn sm" onClick={() => copy(json)}>
                  <Copy size={14} /> {t('settings.mcp.copyJson')}
                </button>
                <button className="btn sm ghost" onClick={() => setShow((v) => !v)}>
                  {show ? t('settings.mcp.hideToken') : t('settings.mcp.showToken')}
                </button>
                <button
                  className="btn sm ghost"
                  onClick={() =>
                    void api.app.mcpResetToken().then((i) => {
                      setInfo(i)
                      toast(t('settings.mcp.tokenReset'))
                    })
                  }
                >
                  <RefreshCw size={14} /> {t('settings.mcp.resetToken')}
                </button>
              </div>
            </div>
          </>
        )}
      </div>
    </section>
  )
}
