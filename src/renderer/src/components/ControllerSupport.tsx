import { useEffect, useState } from 'react'
import { Check, Gamepad2, Loader2 } from 'lucide-react'
import type { ContentItem, Instance, ProjectVersion } from '@shared/types'
import { api } from '../api'
import { useInstanceTask, useStore } from '../store'
import { describe } from '../util'
import { t } from '../i18n'
import { Switch } from './ui'

/**
 * The controller mods ResinCraft knows, best first. Controlify (Fabric, NeoForge; 1.19.4 and newer) is the most used;
 * MidnightControls covers older versions and Quilt. Forge has neither: Controllable, found by name on CurseForge.
 */
const MODRINTH = [
  { id: 'DOUdJVEm', name: 'Controlify' },
  { id: 'bXX9h73M', name: 'MidnightControls' }
]
const KNOWN = /controlify|midnightcontrols|controllable/i

type Pick = { projectId: string; name: string; versionId: string }

/** A release first, else the newest beta: the versions are already the ones for this instance's game and loader. */
function best(versions: ProjectVersion[], inst: Instance): ProjectVersion | undefined {
  const fits = versions.filter((v) => v.gameVersions.includes(inst.mcVersion))
  return fits.find((v) => v.type === 'release') ?? fits[0]
}

async function findControllerMod(inst: Instance): Promise<Pick | undefined> {
  for (const m of MODRINTH) {
    const v = best(await api.content.versions(m.id, inst.id).catch(() => []), inst)
    if (v) return { projectId: m.id, name: m.name, versionId: v.id }
  }
  if (inst.loader === 'forge' || inst.loader === 'neoforge') {
    const found = await api.content
      .search({ query: 'Controllable', kind: 'mod', source: 'curseforge', gameVersion: inst.mcVersion, loader: inst.loader, limit: 10 })
      .catch(() => undefined)
    const hit = found?.hits.find((h) => h.slug === 'controllable')
    const v = hit && best(await api.content.versions(hit.id, inst.id).catch(() => []), inst)
    if (hit && v) return { projectId: hit.id, name: hit.title, versionId: v.id }
  }
  return undefined
}

/** The controller mod switch's state and action (Instance → Settings, and big screen mode's instance page). */
export function useControllerMod(inst: Instance) {
  const { toast, games } = useStore()
  const task = useInstanceTask(inst.id)
  // undefined: still looking; null: none installed.
  const [mod, setMod] = useState<ContentItem | null>()
  const [busy, setBusy] = useState<'on' | 'off'>()
  const modded = inst.loader !== 'vanilla' && inst.loader !== 'horizon'
  const running = games[inst.id] === 'running' || games[inst.id] === 'preparing'
  const on = !!mod?.enabled

  const look = () =>
    api.content
      .list(inst.id, 'mod')
      .then((mods) => setMod(mods.find((x) => KNOWN.test(x.title ?? '') || KNOWN.test(x.filename)) ?? null))
      .catch(() => setMod(null))

  useEffect(() => {
    if (modded) void look()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [inst.id, modded, task])

  const set = async (want: boolean) => {
    setBusy(want ? 'on' : 'off')
    try {
      if (mod) {
        // Already there: just switch it.
        if (mod.enabled !== want) await api.content.toggle(inst.id, 'mod', mod.filename)
        toast(want ? t('controller.turnedOn', { name: mod.title ?? mod.filename }) : t('controller.turnedOff', { name: mod.title ?? mod.filename }), 'success')
      } else if (want) {
        const pick = await findControllerMod(inst)
        if (!pick) {
          toast(t('controller.none', { what: describe(inst) }), 'error')
          return
        }
        await api.content.install(inst.id, pick.projectId, 'mod', pick.versionId)
        toast(t('controller.added', { name: pick.name }), 'success')
      }
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(undefined)
      await look()
    }
  }
  /** Whether the switch can be used now. */
  const usable = modded && !running && !busy && !task && mod !== undefined
  return { mod, busy, modded, running, on, set, usable }
}

/**
 * Instance → Settings: "Play with a controller", one switch. On: adds a controller mod made for this instance's version
 * and loader (Controlify, or the closest one), or turns back on the one it already has. Off: turns that mod off without
 * deleting it, so switching back is instant.
 */
export function InstanceController({ inst }: { inst: Instance }) {
  const { mod, busy, modded, running, on, set, usable } = useControllerMod(inst)

  const status = !modded
    ? t('controller.needsLoader')
    : running
      ? t('controller.running')
      : busy === 'on' && !mod
        ? t('controller.installing')
        : mod
          ? on
            ? t('controller.on', { name: mod.title ?? mod.filename })
            : t('controller.off', { name: mod.title ?? mod.filename })
          : t('controller.hint')

  return (
    <div className="settings-section">
      <h3>{t('controller.title')}</h3>
      <div className="card">
        <div className="setting">
          <div className="text">
            <b className="row" style={{ gap: 8 }}>
              <Gamepad2 size={16} /> {t('controller.switch')}
            </b>
            <span>{status}</span>
          </div>
          <div className="row" style={{ gap: 10 }}>
            {busy && <Loader2 className="spin" size={16} aria-hidden />}
            {on && !busy && (
              <span className="chip success compact">
                <Check /> {t('controller.ready')}
              </span>
            )}
            <Switch
              on={busy ? busy === 'on' : on}
              disabled={!usable}
              onChange={(v) => void set(v)}
              label={t('controller.switch')}
            />
          </div>
        </div>
      </div>
    </div>
  )
}
