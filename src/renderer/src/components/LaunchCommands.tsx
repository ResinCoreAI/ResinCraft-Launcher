// Environment variables and the player's own pre-launch, wrapper and post-exit commands (Settings → Commands and
// each instance's settings). The engine runs them: see src-tauri/src/game/commands.rs.
import { useEffect, useRef, useState } from 'react'
import { Plus, ShieldAlert, X } from 'lucide-react'
import type { EnvVar, LaunchCommands } from '@shared/types'
import { t, type Key } from '../i18n'

export const NO_COMMANDS: LaunchCommands = { preLaunch: '', wrapper: '', postExit: '' }
const NO_ENV: EnvVar[] = []

const NAME = /^[A-Za-z_][A-Za-z0-9_]*$/
/** Same idea as the engine's looks_secret: such values are hidden in the log, and typed like a password here. */
const SECRET = /TOKEN|SECRET|PASS|PWD|KEY|AUTH|CREDENTIAL|COOKIE|SESSION|PRIVATE/i
const isWindows = navigator.userAgent.includes('Windows')

function problem(row: EnvVar, rows: EnvVar[], i: number): Key | null {
  if (!row.name) return row.value ? 'commands.env.badName' : null
  if (!NAME.test(row.name)) return 'commands.env.badName'
  if (row.name.toUpperCase().startsWith('INST_')) return 'commands.env.reserved'
  const same = (a: string) => (isWindows ? a.toUpperCase() === row.name.toUpperCase() : a === row.name)
  if (rows.some((r, j) => j < i && same(r.name))) return 'commands.env.duplicate'
  return null
}

const cleaned = (rows: EnvVar[]) => rows.map((r) => ({ name: r.name.trim(), value: r.value })).filter((r) => r.name || r.value)

/** The "commands run on this PC" warning shown above the command fields. */
export function CommandsWarning() {
  return (
    <div className="callout" style={{ margin: '0 0 12px' }}>
      <ShieldAlert />
      <span>{t('commands.warning')}</span>
    </div>
  )
}

/** A name = value list. Saves when a field loses focus and every row is valid (empty rows are dropped). */
export function EnvEditor({ value: given, onSave, hint }: { value?: EnvVar[]; onSave: (env: EnvVar[]) => Promise<unknown> | void; hint: Key }) {
  const value = given ?? NO_ENV
  const [rows, setRows] = useState<EnvVar[]>(value)
  const saving = useRef(0)
  // Saved from here: keep the rows being typed (an empty one isn't saved, and an earlier save's echo can arrive
  // after a newer edit); changed elsewhere: show the new list.
  useEffect(() => {
    if (!saving.current) setRows((cur) => (JSON.stringify(cleaned(cur)) === JSON.stringify(value) ? cur : value))
  }, [value])
  const problems = rows.map((r, i) => problem(r, rows, i))

  const commit = (next: EnvVar[]) => {
    const clean = cleaned(next)
    if (clean.some((r, i) => problem(r, clean, i))) return
    if (JSON.stringify(clean) === JSON.stringify(value)) return
    saving.current++
    void Promise.resolve(onSave(clean)).finally(() => saving.current--)
  }
  const edit = (i: number, patch: Partial<EnvVar>) => setRows(rows.map((r, j) => (j === i ? { ...r, ...patch } : r)))
  const remove = (i: number) => {
    const next = rows.filter((_, j) => j !== i)
    setRows(next)
    commit(next)
  }

  return (
    <div className="setting stack">
      <div className="row">
        <div className="text grow">
          <b>{t('commands.env')}</b>
          <span>{t(hint)}</span>
        </div>
        <button className="btn sm" onClick={() => setRows([...rows, { name: '', value: '' }])}>
          <Plus /> {t('commands.env.add')}
        </button>
      </div>
      {rows.map((r, i) => (
        <div key={i}>
          <div className="row" style={{ gap: 6 }}>
            <input
              className="input mono"
              style={{ flex: '0 0 38%' }}
              value={r.name}
              placeholder={t('commands.env.name')}
              aria-label={t('commands.env.name')}
              aria-invalid={problems[i] != null}
              spellCheck={false}
              onChange={(e) => edit(i, { name: e.target.value })}
              onBlur={() => commit(rows)}
            />
            <span className="faint">=</span>
            <input
              className="input mono grow"
              type={SECRET.test(r.name) ? 'password' : 'text'}
              value={r.value}
              placeholder={t('commands.env.value')}
              aria-label={t('commands.env.value')}
              spellCheck={false}
              onChange={(e) => edit(i, { value: e.target.value })}
              onBlur={() => commit(rows)}
            />
            <button className="btn sm ghost" onClick={() => remove(i)} aria-label={t('commands.env.remove', { name: r.name || '…' })}>
              <X />
            </button>
          </div>
          {problems[i] && (
            <div className="hint" style={{ color: 'var(--danger)', marginTop: 4 }}>
              {t(problems[i])}
            </div>
          )}
        </div>
      ))}
    </div>
  )
}

/** The pre-launch, wrapper and post-exit fields; each saves when it loses focus. */
export function CommandFields({ value, onSave, disabled }: { value: LaunchCommands; onSave: (c: LaunchCommands) => void; disabled?: boolean }) {
  const [draft, setDraft] = useState(value)
  useEffect(() => setDraft(value), [value])
  const field = (key: keyof LaunchCommands, label: Key, hint: string, placeholder: string) => (
    <div className="setting stack" key={key}>
      <div className="text">
        <b>{t(label)}</b>
        <span>{hint}</span>
      </div>
      <input
        className="input mono"
        value={draft[key]}
        placeholder={placeholder}
        disabled={disabled}
        spellCheck={false}
        onChange={(e) => setDraft({ ...draft, [key]: e.target.value })}
        onBlur={() => draft[key] !== value[key] && onSave(draft)}
      />
    </div>
  )
  return (
    <>
      {field('preLaunch', 'commands.preLaunch', t('commands.preLaunch.hint'), isWindows ? 'echo Starting %INST_NAME%' : 'echo "Starting $INST_NAME"')}
      {field('wrapper', 'commands.wrapper', t('commands.wrapper.hint', { java: '{java}' }), isWindows ? '' : 'prime-run')}
      {field('postExit', 'commands.postExit', t('commands.postExit.hint'), isWindows ? 'echo Exit code %INST_EXIT_CODE%' : 'echo "Exit code $INST_EXIT_CODE"')}
      <div className="setting">
        <span className="hint">{t('commands.variables', { shell: isWindows ? 'cmd' : 'sh' })}</span>
      </div>
    </>
  )
}
