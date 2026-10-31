import { useState } from 'react'
import { Download, Loader2, PackageCheck, Puzzle } from 'lucide-react'
import type { DepInstallResult, DepReport } from '@shared/types'
import { useStore } from '../store'
import { t, type Key } from '../i18n'
import { Modal } from './ui'

/**
 * "Check dependencies" for a mods or plugins folder: the engine reads the jars and lists the required mods that
 * aren't there, with one button to install them from Modrinth. What can't be matched exactly stays listed.
 */
export default function DepCheck({
  check,
  install,
  onChanged
}: {
  check: () => Promise<DepReport>
  install: () => Promise<DepInstallResult>
  onChanged: () => void
}) {
  const { toast } = useStore()
  const [checking, setChecking] = useState(false)
  const [installing, setInstalling] = useState(false)
  const [report, setReport] = useState<DepReport>()
  const [result, setResult] = useState<DepInstallResult>()

  const run = async () => {
    setChecking(true)
    setResult(undefined)
    try {
      const r = await check()
      if (r.missing.length || r.unread.length) setReport(r)
      else toast(t('servers.deps.allGood', { count: r.checked }), 'info')
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setChecking(false)
    }
  }

  const fix = async () => {
    setInstalling(true)
    try {
      const r = await install()
      setResult(r)
      if (r.installed.length) {
        toast(t('servers.deps.installed', { count: r.installed.length, names: r.installed.join(', ') }))
        onChanged()
      }
      // Read the folder again: what's left is what really is still missing.
      setReport(await check())
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setInstalling(false)
    }
  }

  const close = () => {
    if (!installing) setReport(undefined)
  }
  const missing = report?.missing ?? []
  // Tried and left: the button isn't offered again for those alone.
  const tried = (id: string) => result?.notFound.some((n) => n.id === id) || result?.failed.some((f) => f.id === id)
  const installable = missing.filter((m) => !m.disabled && !tried(m.id))

  return (
    <>
      <button className="btn" disabled={checking} onClick={() => void run()}>
        {checking ? <Loader2 className="spin" /> : <PackageCheck />} {t('servers.deps.check')}
      </button>
      {report && (
        <Modal
          title={missing.length ? t('servers.deps.title', { count: missing.length }) : t('servers.deps.titleUnread')}
          subtitle={t('servers.deps.subtitle', { count: report.checked })}
          onClose={close}
          footer={
            <>
              <button className="btn ghost" disabled={installing} onClick={close}>
                {t('common.close')}
              </button>
              {installable.length > 0 && (
                <button className="btn primary" disabled={installing} onClick={() => void fix()}>
                  {installing ? <Loader2 className="spin" /> : <Download />} {t('servers.deps.install')}
                </button>
              )}
            </>
          }
        >
          {missing.length > 0 && (
            <div className="list">
              {missing.map((m) => {
                const notFound = result?.notFound.find((n) => n.id === m.id)
                const failed = result?.failed.find((f) => f.id === m.id)
                return (
                  <div className="list-row" key={m.id}>
                    <div className="ico">
                      <Puzzle />
                    </div>
                    <div className="grow" style={{ minWidth: 0 }}>
                      <div className="title ellipsis">{m.id}</div>
                      <div className="file">{t('servers.deps.neededBy', { names: m.requiredBy.join(', ') })}</div>
                      {m.disabled && !notFound && <div className="file">{t('servers.deps.turnedOff', { file: m.disabled })}</div>}
                      {notFound && <div className="file">{t(`servers.deps.reason.${notFound.reason}` as Key)}</div>}
                      {failed && <div className="file">{t('servers.deps.failed', { error: failed.error })}</div>}
                    </div>
                  </div>
                )
              })}
            </div>
          )}
          {report.unread.length > 0 && (
            <p className="hint" style={{ marginTop: 12 }}>
              {t('servers.deps.unread', { count: report.unread.length, names: report.unread.join(', ') })}
            </p>
          )}
          <p className="hint" style={{ marginTop: 12 }}>
            {t('servers.deps.versionsHint')}
          </p>
        </Modal>
      )}
    </>
  )
}
