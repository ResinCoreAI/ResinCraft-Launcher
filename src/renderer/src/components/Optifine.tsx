import { useMemo } from 'react'
import { Check, ExternalLink, FileUp, Loader2 } from 'lucide-react'
import type { LoaderType, OptifineJar } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { t } from '../i18n'
import { useDropTarget } from './DropImport'

/** OptiFine's official download page. Its terms forbid mirroring the jar, so players download it there themselves. */
export const OPTIFINE_DOWNLOADS = 'https://optifine.net/downloads'

/** OptiFine doesn't run on NeoForge. */
export const optifineFits = (loader: LoaderType) => loader !== 'neoforge'

/** How OptiFine is set up for each loader, shown under the steps. */
export function optifineHow(loader: LoaderType): string {
  if (loader === 'forge') return t('optifine.howForge')
  if (loader === 'fabric' || loader === 'quilt') return t('optifine.howFabric', { loader: loader === 'quilt' ? 'Quilt' : 'Fabric' })
  return t('optifine.howVanilla')
}

/**
 * The two steps to add OptiFine: open optifine.net for the instance's Minecraft version, then choose
 * (or drop) the jar the player downloaded. `jar` is the checked jar once one is picked.
 */
export function OptifineSteps({
  mc,
  loader,
  jar,
  busy,
  onPick
}: {
  mc: string
  loader: LoaderType
  jar?: OptifineJar
  busy?: boolean
  onPick: (path: string) => void
}) {
  const { toast } = useStore()
  const browse = async () => {
    try {
      const path = await api.optifine.browse()
      if (path) onPick(path)
    } catch (e) {
      toast((e as Error).message, 'error')
    }
  }
  // While the steps are on screen, a jar dropped on the window is taken as the OptiFine download.
  const drop = useMemo(
    () => ({
      label: t('optifine.drop'),
      onDrop: (paths: string[]) => {
        const jarPath = paths.find((p) => /\.jar$/i.test(p))
        if (jarPath) onPick(jarPath)
        else toast(t('optifine.dropWrongType'), 'info')
      }
    }),
    [onPick, toast]
  )
  useDropTarget(busy ? null : drop)

  return (
    <div className="optifine">
      <div className="steps">
        <div className="step">
          <div className="num">1</div>
          <div className="grow">
            <b>{t('optifine.step1', { version: mc })}</b>
            <span>{t('optifine.step1Hint')}</span>
          </div>
          <button className="btn sm" onClick={() => void api.app.openExternal(OPTIFINE_DOWNLOADS)}>
            <ExternalLink /> optifine.net
          </button>
        </div>
        <div className={`step ${jar ? 'done' : ''}`}>
          <div className="num">{jar ? <Check size={16} /> : 2}</div>
          <div className="grow">
            <b>{jar ? t('optifine.picked', { edition: jar.edition, version: jar.mcVersion }) : t('optifine.step2')}</b>
            <span>{optifineHow(loader)}</span>
          </div>
          <button className={`btn sm ${jar ? '' : 'primary'}`} disabled={busy} onClick={() => void browse()}>
            {busy ? <Loader2 className="spin" /> : <FileUp />} {jar ? t('optifine.change') : t('optifine.choose')}
          </button>
        </div>
      </div>
      <span className="hint">{t('optifine.alternative')}</span>
    </div>
  )
}
