// The Files tab of an instance or a hosted server: browse the folder, organise files and edit text files.
// Everything goes through api.files, which keeps every path inside the root.
import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState, type CSSProperties, type ReactNode } from 'react'
import { createPortal } from 'react-dom'
import {
  AlertTriangle,
  ArrowDown,
  ArrowLeft,
  ArrowUp,
  ChevronRight,
  ClipboardPaste,
  Copy,
  Database,
  File,
  FileArchive,
  FileCode,
  FileCog,
  FileImage,
  FileJson,
  FilePlus,
  FileText,
  Folder,
  FolderOpen,
  FolderPlus,
  FolderSymlink,
  HardDrive,
  Loader2,
  Pencil,
  RefreshCw,
  Save,
  Scissors,
  ScrollText,
  Search,
  Trash2,
  X
} from 'lucide-react'
import type { FileEntry, FileRoot, TextFile } from '@shared/types'
import { api } from '../api'
import { useStore } from '../store'
import { bytes, timeAgo } from '../util'
import { rich, t } from '../i18n'
import { ConfirmModal, Empty, Modal } from './ui'
import { useDropTarget, type DropTarget } from './DropImport'
import type { CodeEditorHandle } from './CodeEditor'
import '../files.css'

const CodeEditor = lazy(() => import('./CodeEditor'))

type SortKey = 'name' | 'size' | 'modified'
type Clip = { mode: 'copy' | 'cut'; paths: string[] }
type Draft = { file: TextFile; text: string }

// Kept per root while the app runs, so leaving the tab and coming back restores where you were.
const clipboards = new Map<string, Clip>()
const lastDir = new Map<string, string>()
/** Unsaved edits of an editor that was closed by leaving the tab; reopened with the tab. */
const drafts = new Map<string, Draft>()

const rootKey = (r: FileRoot) => `${r.kind}:${r.id}`
const join = (dir: string, name: string) => (dir ? `${dir}/${name}` : name)
const parentOf = (path: string) => path.split('/').slice(0, -1).join('/')
const baseName = (path: string) => path.split('/').pop() ?? path
/** `path` is `dir` or somewhere inside it. */
const within = (path: string, dir: string) => path === dir || path.startsWith(`${dir}/`)

/** Mirrors the engine's name check (files::valid_name), to explain a bad name before sending it. */
export function validName(name: string): boolean {
  return !!name && name.length <= 255 && !/[<>:"/\\|?*\u0000-\u001f]/.test(name) && !/[. ]$/.test(name) && !/^(con|prn|aux|nul|com[1-9]|lpt[1-9])(\.|$)/i.test(name)
}

const ICONS: [RegExp, ReactNode][] = [
  [/\.(json5?|mcmeta)$/i, <FileJson key="j" />],
  [/\.(properties|toml|ya?ml|cfg|conf|ini|env)$/i, <FileCog key="c" />],
  [/\.(js|mjs|zs|snbt|sh|bat|cmd|ps1|mcfunction)$/i, <FileCode key="s" />],
  [/\.(log|log\.gz)$/i, <ScrollText key="l" />],
  [/\.(jar|zip|mrpack|gz|7z|rar|tar|disabled)$/i, <FileArchive key="a" />],
  [/\.(png|jpe?g|gif|webp|bmp|ico)$/i, <FileImage key="i" />],
  [/\.(dat|dat_old|mca|mcr|nbt|mcc)$/i, <Database key="d" />],
  [/\.(txt|md|lang)$/i, <FileText key="t" />]
]

function EntryIcon({ e }: { e: FileEntry }) {
  if (e.dir) return e.link ? <FolderSymlink /> : <Folder />
  return <>{ICONS.find(([re]) => re.test(e.name))?.[1] ?? <File />}</>
}

function sortEntries(list: FileEntry[], key: SortKey, desc: boolean): FileEntry[] {
  const byName = (a: FileEntry, b: FileEntry) => a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: 'base' })
  return [...list].sort((a, b) => {
    if (a.dir !== b.dir) return a.dir ? -1 : 1
    const d = key === 'size' ? (a.dir ? (a.items ?? 0) - (b.items ?? 0) : a.size - b.size) : key === 'modified' ? a.modified - b.modified : byName(a, b)
    return (desc ? -d : d) || byName(a, b)
  })
}

export default function FileManager({ root, rootLabel, running }: { root: FileRoot; rootLabel: string; running?: boolean }) {
  const { toast } = useStore()
  const key = rootKey(root)
  const [dir, setDir] = useState(() => lastDir.get(key) ?? '')
  const [entries, setEntries] = useState<FileEntry[]>()
  const [loadError, setLoadError] = useState<string>()
  const [sort, setSort] = useState<{ key: SortKey; desc: boolean }>({ key: 'name', desc: false })
  const [filter, setFilter] = useState('')
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const anchor = useRef<string>(null)
  const [renaming, setRenaming] = useState<string>()
  const [creating, setCreating] = useState<'file' | 'folder'>()
  const [trashing, setTrashing] = useState<string[]>()
  const [menu, setMenu] = useState<{ x: number; y: number; entry?: FileEntry }>()
  const [clip, setClipState] = useState<Clip | undefined>(() => clipboards.get(key))
  const [open, setOpen] = useState<{ file: TextFile; draft?: string }>()
  const [opening, setOpening] = useState<string>()
  const [drag, setDrag] = useState<{ paths: string[]; x: number; y: number; over: string | null }>()
  const dragRef = useRef(drag)
  dragRef.current = drag
  const dragged = useRef(false)
  const listRef = useRef<HTMLDivElement>(null)
  const filterRef = useRef<HTMLInputElement>(null)

  const setClip = (c: Clip | undefined) => {
    if (c) clipboards.set(key, c)
    else clipboards.delete(key)
    setClipState(c)
  }

  const load = useCallback(
    async (d: string, select?: string[]) => {
      try {
        const list = await api.files.list(root, d)
        setEntries(list)
        setLoadError(undefined)
        if (select) setSelected(new Set(select))
      } catch (e) {
        setEntries([])
        setLoadError((e as Error).message)
      }
    },
    [key]
  )

  useEffect(() => {
    lastDir.set(key, dir)
    setEntries(undefined)
    setSelected(new Set())
    setFilter('')
    void load(dir)
  }, [key, dir, load])

  // A draft left behind when the tab was closed with unsaved edits opens again.
  useEffect(() => {
    const d = drafts.get(key)
    if (!d) return
    drafts.delete(key)
    setOpen({ file: d.file, draft: d.text })
  }, [key])

  const shown = useMemo(() => {
    const q = filter.trim().toLowerCase()
    return sortEntries((entries ?? []).filter((e) => !q || e.name.toLowerCase().includes(q)), sort.key, sort.desc)
  }, [entries, filter, sort])

  const fail = (e: unknown) => toast((e as Error).message, 'error')

  const openEntry = async (e: FileEntry) => {
    if (e.dir) return setDir(e.path)
    setOpening(e.path)
    try {
      setOpen({ file: await api.files.readText(root, e.path) })
    } catch (err) {
      fail(err)
    } finally {
      setOpening(undefined)
    }
  }

  const selection = () => shown.filter((e) => selected.has(e.path)).map((e) => e.path)

  const clickRow = (ev: React.MouseEvent, e: FileEntry) => {
    if (dragged.current) {
      dragged.current = false
      return
    }
    listRef.current?.focus({ preventScroll: true })
    if (ev.shiftKey && anchor.current) {
      const a = shown.findIndex((x) => x.path === anchor.current)
      const b = shown.findIndex((x) => x.path === e.path)
      if (a >= 0 && b >= 0) {
        const [from, to] = a < b ? [a, b] : [b, a]
        return setSelected(new Set(shown.slice(from, to + 1).map((x) => x.path)))
      }
    }
    anchor.current = e.path
    if (ev.ctrlKey || ev.metaKey) {
      const next = new Set(selected)
      if (next.has(e.path)) next.delete(e.path)
      else next.add(e.path)
      return setSelected(next)
    }
    setSelected(new Set([e.path]))
  }

  // ---- moving by drag (pointer events: HTML5 drag-and-drop doesn't work in the Tauri window) ----

  const onPointerDown = (ev: React.PointerEvent, e: FileEntry) => {
    if (ev.button !== 0 || renaming || (ev.target as HTMLElement).closest('input, button')) return
    const sx = ev.clientX
    const sy = ev.clientY
    dragged.current = false
    const move = (m: PointerEvent) => {
      if (Math.hypot(m.clientX - sx, m.clientY - sy) < 6) return
      cleanup()
      dragged.current = true
      const paths = selected.has(e.path) ? selection() : [e.path]
      if (!selected.has(e.path)) setSelected(new Set([e.path]))
      setDrag({ paths, x: m.clientX, y: m.clientY, over: null })
    }
    const cleanup = () => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', cleanup)
    }
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', cleanup)
  }

  const dragging = !!drag
  useEffect(() => {
    if (!dragging) return
    const target = (x: number, y: number) => {
      const el = document.elementFromPoint(x, y)?.closest<HTMLElement>('[data-drop-dir]')
      const dest = el?.dataset.dropDir
      const paths = dragRef.current?.paths ?? []
      // Not onto one of the dragged folders, into one of them, or the folder they're already in.
      if (dest == null || paths.some((p) => within(dest, p)) || paths.every((p) => parentOf(p) === dest)) return null
      return dest
    }
    const move = (e: PointerEvent) => setDrag((d) => (d ? { ...d, x: e.clientX, y: e.clientY, over: target(e.clientX, e.clientY) } : d))
    const up = async () => {
      const d = dragRef.current
      setDrag(undefined)
      // The click that follows pointerup lands on whatever is under the pointer; let it through next time.
      setTimeout(() => (dragged.current = false), 0)
      if (!d || d.over == null) return
      try {
        await api.files.move(root, d.paths, d.over)
        toast(t('files.moved', { count: d.paths.length, folder: baseName(d.over) || rootLabel }))
      } catch (e) {
        fail(e)
      }
      void load(dir)
    }
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && setDrag(undefined)
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', up, { once: true })
    window.addEventListener('keydown', onKey)
    document.body.classList.add('dragging')
    return () => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', up)
      window.removeEventListener('keydown', onKey)
      document.body.classList.remove('dragging')
    }
  }, [dragging])

  // ---- actions ----

  const paste = async () => {
    if (!clip) return
    try {
      const out = clip.mode === 'cut' ? await api.files.move(root, clip.paths, dir) : await api.files.copy(root, clip.paths, dir)
      if (clip.mode === 'cut') setClip(undefined)
      await load(dir, out)
    } catch (e) {
      fail(e)
      void load(dir)
    }
  }

  // The rename box unmounts on Enter / Escape; keep the keyboard on the list.
  const endRename = () => {
    setRenaming(undefined)
    setTimeout(() => listRef.current?.focus({ preventScroll: true }), 0)
  }

  const commitRename = async (e: FileEntry, name: string) => {
    endRename()
    name = name.trim()
    if (!name || name === e.name) return
    if (!validName(name)) return toast(t('files.badName', { name }), 'error')
    try {
      const done = await api.files.rename(root, e.path, name)
      await load(dir, [done.path])
    } catch (err) {
      fail(err)
    }
  }

  const trash = async (paths: string[]) => {
    try {
      await api.files.trash(root, paths)
      toast(paths.length === 1 ? t('files.trashed.one', { name: baseName(paths[0]) }) : t('files.trashed.many', { count: paths.length }))
    } catch (e) {
      fail(e)
    }
    await load(dir)
  }

  const reveal = (path: string) => void api.files.reveal(root, path).catch(fail)

  const drop = useMemo<DropTarget | null>(
    () =>
      open
        ? null
        : {
            label: t('files.drop.label', { folder: baseName(dir) || rootLabel }),
            onDrop: async (paths) => {
              try {
                const out = await api.files.import(root, dir, paths)
                toast(t('files.imported', { count: out.length }))
                await load(dir, out)
              } catch (e) {
                fail(e)
              }
            }
          },
    [open, dir, key, rootLabel]
  )
  useDropTarget(drop)

  const onListKey = (e: React.KeyboardEvent) => {
    if (renaming || (e.target as HTMLElement).closest('input')) return
    const ctrl = e.ctrlKey || e.metaKey
    const k = e.key.toLowerCase()
    const sel = selection()
    const one = sel.length === 1 ? shown.find((x) => x.path === sel[0]) : undefined
    if (k === 'arrowdown' || k === 'arrowup') {
      e.preventDefault()
      const i = shown.findIndex((x) => x.path === (anchor.current ?? ''))
      const next = shown[Math.max(0, Math.min(shown.length - 1, i < 0 ? 0 : i + (k === 'arrowdown' ? 1 : -1)))]
      if (!next) return
      anchor.current = next.path
      setSelected(new Set([next.path]))
      listRef.current?.querySelector(`[data-path="${CSS.escape(next.path)}"]`)?.scrollIntoView({ block: 'nearest' })
    } else if (k === 'enter' && one) void openEntry(one)
    else if (k === 'backspace' && dir) setDir(parentOf(dir))
    else if (k === 'delete' && sel.length) setTrashing(sel)
    else if (k === 'f2' && one) setRenaming(one.path)
    else if (k === 'escape') setSelected(new Set())
    else if (ctrl && k === 'a') setSelected(new Set(shown.map((x) => x.path)))
    else if (ctrl && (k === 'c' || k === 'x') && sel.length) setClip({ mode: k === 'c' ? 'copy' : 'cut', paths: sel })
    else if (ctrl && k === 'v') void paste()
    else if (ctrl && k === 'f') filterRef.current?.focus()
    else return
    e.preventDefault()
  }

  if (open)
    return (
      <EditorPanel
        key={`${open.file.path}:${open.file.modified}`}
        root={root}
        rootLabel={rootLabel}
        running={running}
        file={open.file}
        draft={open.draft}
        onClose={(reload) => {
          setOpen(undefined)
          if (reload) void load(dir)
        }}
        onCrumb={(d) => {
          setOpen(undefined)
          setDir(d)
        }}
      />
    )

  const sel = selection()
  const crumbs = dir ? dir.split('/') : []
  const sortHead = (k: SortKey, label: string) => (
    <button className={sort.key === k ? 'on' : ''} onClick={() => setSort((s) => ({ key: k, desc: s.key === k ? !s.desc : k !== 'name' }))}>
      {label}
      {sort.key === k && (sort.desc ? <ArrowDown /> : <ArrowUp />)}
    </button>
  )

  return (
    <div className="fm">
      <div className="toolbar fm-bar">
        <Crumbs rootLabel={rootLabel} crumbs={crumbs} onGo={setDir} dropOver={drag?.over} />
        <div className="grow" />
        <div className="search fm-filter">
          <Search />
          <input ref={filterRef} className="input" placeholder={t('files.filter')} value={filter} onChange={(e) => setFilter(e.target.value)} />
        </div>
        <button className="btn icon" title={t('files.newFile')} onClick={() => setCreating('file')}>
          <FilePlus />
        </button>
        <button className="btn icon" title={t('files.newFolder')} onClick={() => setCreating('folder')}>
          <FolderPlus />
        </button>
        {clip && (
          <button className="btn" title={t('files.pasteHint')} onClick={() => void paste()}>
            <ClipboardPaste /> {t('files.paste', { count: clip.paths.length })}
          </button>
        )}
        <div className="toolbar-divider" />
        <button className="btn icon" title={t('common.refresh')} onClick={() => void load(dir)}>
          <RefreshCw />
        </button>
        <button className="btn icon" title={t('files.showInExplorer')} onClick={() => reveal(dir)}>
          <FolderOpen />
        </button>
      </div>

      <div
        className="fm-list"
        ref={listRef}
        tabIndex={0}
        role="grid"
        aria-label={t('files.label')}
        aria-multiselectable
        onKeyDown={onListKey}
        onClick={(e) => e.target === e.currentTarget && setSelected(new Set())}
        onContextMenu={(e) => {
          e.preventDefault()
          if (!(e.target as HTMLElement).closest('.fm-row')) setMenu({ x: e.clientX, y: e.clientY })
        }}
      >
        <div className="fm-head" role="row">
          <span className="fm-ico" />
          {sortHead('name', t('files.col.name'))}
          {sortHead('size', t('files.col.size'))}
          {sortHead('modified', t('files.col.modified'))}
        </div>
        {dir && (
          <div className={`fm-row up ${drag?.over === parentOf(dir) ? 'drop' : ''}`} data-drop-dir={parentOf(dir)} onDoubleClick={() => setDir(parentOf(dir))} role="row">
            <span className="fm-ico">
              <ArrowUp />
            </span>
            <span className="fm-name">..</span>
          </div>
        )}
        {!entries && !loadError && (
          <div className="fm-empty">
            <Loader2 className="spin" />
          </div>
        )}
        {loadError && (
          <div className="callout danger fm-callout">
            <AlertTriangle />
            <span>{loadError}</span>
          </div>
        )}
        {entries && !loadError && !shown.length && (
          <Empty icon={<HardDrive />} title={filter ? t('files.noMatch') : t('files.emptyFolder')}>
            {!filter && t('files.drop.hint')}
          </Empty>
        )}
        {shown.map((e) => {
          const on = selected.has(e.path)
          const cut = clip?.mode === 'cut' && clip.paths.includes(e.path)
          return (
            <div
              key={e.path}
              className={`fm-row ${on ? 'on' : ''} ${cut ? 'cut' : ''} ${drag?.over === e.path ? 'drop' : ''} ${drag?.paths.includes(e.path) ? 'lifted' : ''}`}
              data-path={e.path}
              data-drop-dir={e.dir ? e.path : undefined}
              role="row"
              aria-selected={on}
              onPointerDown={(ev) => onPointerDown(ev, e)}
              onClick={(ev) => clickRow(ev, e)}
              onDoubleClick={() => void openEntry(e)}
              onContextMenu={(ev) => {
                ev.preventDefault()
                ev.stopPropagation()
                if (!selected.has(e.path)) setSelected(new Set([e.path]))
                anchor.current = e.path
                setMenu({ x: ev.clientX, y: ev.clientY, entry: e })
              }}
            >
              <span className={`fm-ico ${e.dir ? 'dir' : ''}`}>{opening === e.path ? <Loader2 className="spin" /> : <EntryIcon e={e} />}</span>
              {renaming === e.path ? (
                <RenameInput entry={e} onDone={(name) => void commitRename(e, name)} onCancel={endRename} />
              ) : (
                <span className="fm-name ellipsis" title={e.name}>
                  {e.name}
                </span>
              )}
              <span className="fm-size">{e.dir ? t('files.items', { count: e.items ?? 0 }) : bytes(e.size)}</span>
              <span className="fm-time" title={new Date(e.modified).toLocaleString()}>
                {timeAgo(e.modified)}
              </span>
            </div>
          )
        })}
      </div>
      <p className="hint fm-hint">{t('files.hint')}</p>

      {sel.length > 1 && (
        <div className="select-bar">
          <span>{rich('files.selected', { b: (s) => <b>{s}</b> }, { count: sel.length })}</span>
          <button className="btn sm ghost" onClick={() => setClip({ mode: 'copy', paths: sel })}>
            <Copy /> {t('common.copy')}
          </button>
          <button className="btn sm ghost" onClick={() => setClip({ mode: 'cut', paths: sel })}>
            <Scissors /> {t('files.cut')}
          </button>
          <button className="btn sm ghost danger" onClick={() => setTrashing(sel)}>
            <Trash2 /> {t('common.delete')}
          </button>
          <button className="btn sm ghost icon" title={t('files.clearSelection')} onClick={() => setSelected(new Set())}>
            <X />
          </button>
        </div>
      )}

      {menu && (
        <ContextMenu x={menu.x} y={menu.y} onClose={() => setMenu(undefined)}>
          {menu.entry ? (
            <>
              <MenuButton icon={menu.entry.dir ? <FolderOpen /> : <Pencil />} label={menu.entry.dir ? t('files.open') : t('files.edit')} onClick={() => void openEntry(menu.entry!)} />
              <MenuButton icon={<Pencil />} label={t('common.rename')} disabled={sel.length > 1} onClick={() => setRenaming(menu.entry!.path)} />
              <hr />
              <MenuButton icon={<Copy />} label={t('common.copy')} onClick={() => setClip({ mode: 'copy', paths: selection() })} />
              <MenuButton icon={<Scissors />} label={t('files.cut')} onClick={() => setClip({ mode: 'cut', paths: selection() })} />
              {clip && menu.entry.dir && (
                <MenuButton
                  icon={<ClipboardPaste />}
                  label={t('files.pasteInto', { folder: menu.entry.name })}
                  onClick={async () => {
                    const into = menu.entry!.path
                    try {
                      if (clip.mode === 'cut') {
                        await api.files.move(root, clip.paths, into)
                        setClip(undefined)
                      } else await api.files.copy(root, clip.paths, into)
                    } catch (e) {
                      fail(e)
                    }
                    void load(dir)
                  }}
                />
              )}
              <MenuButton icon={<FolderOpen />} label={t('files.showInExplorer')} onClick={() => reveal(menu.entry!.path)} />
              <hr />
              <MenuButton icon={<Trash2 />} label={t('common.delete')} danger onClick={() => setTrashing(selection())} />
            </>
          ) : (
            <>
              <MenuButton icon={<FilePlus />} label={t('files.newFile')} onClick={() => setCreating('file')} />
              <MenuButton icon={<FolderPlus />} label={t('files.newFolder')} onClick={() => setCreating('folder')} />
              {clip && <MenuButton icon={<ClipboardPaste />} label={t('files.paste', { count: clip.paths.length })} onClick={() => void paste()} />}
              <hr />
              <MenuButton icon={<FolderOpen />} label={t('files.showInExplorer')} onClick={() => reveal(dir)} />
            </>
          )}
        </ContextMenu>
      )}

      {drag && <DragGhost x={drag.x} y={drag.y} paths={drag.paths} />}

      {creating && (
        <NewEntryModal
          kind={creating}
          taken={(name) => !!entries?.some((e) => e.name.toLowerCase() === name.toLowerCase())}
          onClose={() => setCreating(undefined)}
          onCreate={async (name) => {
            const path = join(dir, name)
            if (creating === 'folder') {
              await api.files.mkdir(root, path)
              await load(dir, [path])
            } else {
              const made = await api.files.writeText(root, path, '', { create: true })
              await load(dir, [path])
              setOpen({ file: { ...made, kind: 'text', text: '' } })
            }
          }}
        />
      )}

      {trashing && (
        <ConfirmModal
          title={trashing.length === 1 ? t('files.trash.titleOne') : t('files.trash.titleMany', { count: trashing.length })}
          body={
            trashing.length === 1
              ? rich('files.trash.bodyOne', { file: (s) => <b className="mono">{s}</b> }, { name: baseName(trashing[0]) })
              : t('files.trash.bodyMany', { count: trashing.length })
          }
          confirm={t('files.trash.confirm')}
          onConfirm={() => trash(trashing)}
          onClose={() => setTrashing(undefined)}
        />
      )}
    </div>
  )
}

function Crumbs({ rootLabel, crumbs, onGo, dropOver }: { rootLabel: string; crumbs: string[]; onGo: (dir: string) => void; dropOver?: string | null }) {
  return (
    <nav className="fm-crumbs" aria-label={t('files.path')}>
      <button className={dropOver === '' ? 'drop' : ''} data-drop-dir="" onClick={() => onGo('')}>
        <HardDrive /> {rootLabel}
      </button>
      {crumbs.map((c, i) => {
        const path = crumbs.slice(0, i + 1).join('/')
        const last = i === crumbs.length - 1
        return (
          <span key={path} className="fm-crumb">
            <ChevronRight />
            {last ? (
              <b data-drop-dir={path}>{c}</b>
            ) : (
              <button className={dropOver === path ? 'drop' : ''} data-drop-dir={path} onClick={() => onGo(path)}>
                {c}
              </button>
            )}
          </span>
        )
      })}
    </nav>
  )
}

function RenameInput({ entry, onDone, onCancel }: { entry: FileEntry; onDone: (name: string) => void; onCancel: () => void }) {
  const ref = useRef<HTMLInputElement>(null)
  const finished = useRef(false)
  useEffect(() => {
    const el = ref.current
    if (!el) return
    el.focus()
    // Select the name without its extension, like Explorer.
    const dot = entry.dir ? -1 : entry.name.lastIndexOf('.')
    el.setSelectionRange(0, dot > 0 ? dot : entry.name.length)
  }, [entry])
  const finish = (save: boolean) => {
    if (finished.current) return
    finished.current = true
    if (save) onDone(ref.current?.value ?? '')
    else onCancel()
  }
  return (
    <input
      ref={ref}
      className="input fm-rename"
      defaultValue={entry.name}
      aria-label={t('common.rename')}
      onClick={(e) => e.stopPropagation()}
      onDoubleClick={(e) => e.stopPropagation()}
      onKeyDown={(e) => {
        e.stopPropagation()
        if (e.key === 'Enter') finish(true)
        else if (e.key === 'Escape') finish(false)
      }}
      onBlur={() => finish(true)}
    />
  )
}

function NewEntryModal({
  kind,
  taken,
  onCreate,
  onClose
}: {
  kind: 'file' | 'folder'
  taken: (name: string) => boolean
  onCreate: (name: string) => Promise<void>
  onClose: () => void
}) {
  const [name, setName] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string>()
  const trimmed = name.trim()
  const problem = !trimmed ? undefined : !validName(trimmed) ? t('files.badName', { name: trimmed }) : taken(trimmed) ? t('files.taken', { name: trimmed }) : undefined
  const submit = async () => {
    if (!trimmed || problem || busy) return
    setBusy(true)
    try {
      await onCreate(trimmed)
      onClose()
    } catch (e) {
      setError((e as Error).message)
      setBusy(false)
    }
  }
  return (
    <Modal
      title={kind === 'file' ? t('files.newFile') : t('files.newFolder')}
      onClose={onClose}
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button className="btn primary" disabled={!trimmed || !!problem || busy} onClick={() => void submit()}>
            {t('files.create')}
          </button>
        </>
      }
    >
      <label className="field">
        <span className="label">{t('files.name')}</span>
        <input
          className="input"
          autoFocus
          value={name}
          placeholder={kind === 'file' ? 'config.json' : t('files.folderPlaceholder')}
          onChange={(e) => {
            setName(e.target.value)
            setError(undefined)
          }}
          onKeyDown={(e) => e.key === 'Enter' && void submit()}
        />
      </label>
      {(problem || error) && <p className="fm-error">{problem ?? error}</p>}
    </Modal>
  )
}

function ContextMenu({ x, y, children, onClose }: { x: number; y: number; children: ReactNode; onClose: () => void }) {
  const ref = useRef<HTMLDivElement>(null)
  const [pos, setPos] = useState({ left: x, top: y })
  useEffect(() => {
    const el = ref.current
    if (el) setPos({ left: Math.min(x, window.innerWidth - el.offsetWidth - 8), top: Math.min(y, window.innerHeight - el.offsetHeight - 8) })
    const close = (e: Event) => !ref.current?.contains(e.target as Node) && onClose()
    const key = (e: KeyboardEvent) => e.key === 'Escape' && onClose()
    window.addEventListener('pointerdown', close, true)
    window.addEventListener('keydown', key)
    window.addEventListener('blur', onClose)
    return () => {
      window.removeEventListener('pointerdown', close, true)
      window.removeEventListener('keydown', key)
      window.removeEventListener('blur', onClose)
    }
  }, [x, y, onClose])
  return createPortal(
    <div className="menu fm-menu" ref={ref} style={pos} role="menu" onClick={onClose} onContextMenu={(e) => e.preventDefault()}>
      {children}
    </div>,
    document.body
  )
}

function MenuButton({ icon, label, onClick, danger, disabled }: { icon: ReactNode; label: string; onClick: () => void; danger?: boolean; disabled?: boolean }) {
  return (
    <button className={danger ? 'danger' : ''} role="menuitem" disabled={disabled} onClick={onClick}>
      {icon}
      {label}
    </button>
  )
}

function DragGhost({ x, y, paths }: { x: number; y: number; paths: string[] }) {
  const style: CSSProperties = { transform: `translate(${x + 14}px, ${y + 10}px)` }
  return createPortal(
    <div className="drag-ghost fm-ghost" style={style} aria-hidden>
      {paths.length === 1 ? baseName(paths[0]) : t('files.dragCount', { count: paths.length })}
    </div>,
    document.body
  )
}

// ---------- Editor ----------

function EditorPanel({
  root,
  rootLabel,
  running,
  file: opened,
  draft,
  onClose,
  onCrumb
}: {
  root: FileRoot
  rootLabel: string
  running?: boolean
  file: TextFile
  draft?: string
  onClose: (reload: boolean) => void
  onCrumb: (dir: string) => void
}) {
  const { toast } = useStore()
  const [file, setFile] = useState(opened)
  const [dirty, setDirty] = useState(false)
  const [saving, setSaving] = useState(false)
  const [cursor, setCursor] = useState({ line: 1, col: 1 })
  const [confirmClose, setConfirmClose] = useState<() => void>()
  const [conflict, setConflict] = useState(false)
  const editor = useRef<CodeEditorHandle>(null)
  const changed = useRef(false)
  const crlf = useMemo(() => /\r\n/.test(opened.text ?? ''), [opened])
  const initial = useMemo(() => opened.text ?? '', [opened])
  // Closing on purpose drops the edits; leaving the tab keeps them as a draft.
  const leaving = useRef(false)
  const fileRef = useRef(file)
  fileRef.current = file

  const save = async (force = false) => {
    if (!editor.current || saving || file.kind !== 'text') return
    let text = editor.current.text()
    if (crlf) text = text.replace(/\r?\n/g, '\r\n')
    setSaving(true)
    try {
      const saved = await api.files.writeText(root, file.path, text, { expectModified: force ? undefined : file.modified, bom: file.bom })
      setFile((f) => ({ ...f, ...saved }))
      editor.current?.markSaved()
      changed.current = true
      toast(t('files.saved', { name: file.name }))
    } catch (e) {
      const key = (e as { key?: string }).key ?? ''
      if (key.endsWith('files.changedOnDisk')) setConflict(true)
      else toast((e as Error).message, 'error')
    } finally {
      setSaving(false)
    }
  }
  const saveRef = useRef(save)
  saveRef.current = save

  const guard = (then: () => void) => {
    const go = () => {
      leaving.current = true
      then()
    }
    if (dirty) setConfirmClose(() => go)
    else go()
  }
  const close = () => guard(() => onClose(changed.current))

  const dirParts = parentOf(file.path) ? parentOf(file.path).split('/') : []
  const notice =
    running && root.kind === 'server'
      ? file.path === 'server.properties'
        ? t('files.notice.serverProperties')
        : t('files.notice.serverRunning')
      : running
        ? t('files.notice.gameRunning')
        : undefined

  return (
    <div className="fm fm-editor">
      <div className="toolbar fm-bar">
        <button className="btn icon" title={t('files.backToFiles')} onClick={close}>
          <ArrowLeft />
        </button>
        <nav className="fm-crumbs" aria-label={t('files.path')}>
          <button onClick={() => guard(() => onCrumb(''))}>
            <HardDrive /> {rootLabel}
          </button>
          {dirParts.map((c, i) => {
            const path = dirParts.slice(0, i + 1).join('/')
            return (
              <span key={path} className="fm-crumb">
                <ChevronRight />
                <button onClick={() => guard(() => onCrumb(path))}>{c}</button>
              </span>
            )
          })}
          <span className="fm-crumb">
            <ChevronRight />
            <b>{file.name}</b>
          </span>
        </nav>
        {dirty && (
          <span className="fm-dirty" title={t('files.editor.unsavedHint')}>
            <i /> {t('files.editor.unsaved')}
          </span>
        )}
        <div className="grow" />
        {file.kind === 'text' && (
          <button className="btn icon" title={t('files.editor.searchHint')} onClick={() => editor.current?.openSearch()}>
            <Search />
          </button>
        )}
        <button className="btn icon" title={t('files.showInExplorer')} onClick={() => void api.files.reveal(root, file.path).catch((e) => toast((e as Error).message, 'error'))}>
          <FolderOpen />
        </button>
        {file.kind === 'text' && (
          <button className="btn primary" disabled={!dirty || saving} title={t('files.editor.saveHint')} onClick={() => void save()}>
            {saving ? <Loader2 className="spin" /> : <Save />} {t('common.save')}
          </button>
        )}
      </div>

      {notice && file.kind === 'text' && (
        <div className="callout fm-callout">
          <AlertTriangle />
          <span>{notice}</span>
        </div>
      )}

      {file.kind === 'text' ? (
        <>
          <div className="fm-code-wrap">
            <Suspense
              fallback={
                <div className="fm-empty">
                  <Loader2 className="spin" />
                </div>
              }
            >
              <CodeEditor
                ref={editor}
                initial={initial}
                draft={draft}
                name={file.name}
                onDirty={setDirty}
                onSave={() => void saveRef.current()}
                onCursor={(line, col) => setCursor({ line, col })}
                // Unsaved edits survive switching tabs: they come back when the Files tab opens again.
                onDispose={(text) => !leaving.current && drafts.set(rootKey(root), { file: fileRef.current, text })}
              />
            </Suspense>
          </div>
          <div className="fm-status">
            <span>{t('files.editor.position', { line: cursor.line, col: cursor.col })}</span>
            <span>{crlf ? 'CRLF' : 'LF'}</span>
            <span>UTF-8{file.bom ? ' BOM' : ''}</span>
            <span>{bytes(file.size)}</span>
            <span className="grow" />
            <span>{t('files.editor.keys')}</span>
          </div>
        </>
      ) : (
        <div className="fm-info">
          <div className="fm-info-ico">
            <EntryIcon e={file} />
          </div>
          <h3>{file.name}</h3>
          <p className="muted">{file.kind === 'tooLarge' ? t('files.tooLarge', { size: bytes(file.size) }) : t('files.binary')}</p>
          <dl>
            <dt>{t('files.col.size')}</dt>
            <dd>{bytes(file.size)}</dd>
            <dt>{t('files.col.modified')}</dt>
            <dd>{new Date(file.modified).toLocaleString()}</dd>
            <dt>{t('files.path')}</dt>
            <dd className="mono">{file.path}</dd>
          </dl>
          <button className="btn" onClick={() => void api.files.reveal(root, file.path).catch((e) => toast((e as Error).message, 'error'))}>
            <FolderOpen /> {t('files.showInExplorer')}
          </button>
        </div>
      )}

      {confirmClose && (
        <ConfirmModal
          title={t('files.discard.title')}
          body={rich('files.discard.body', { file: (s) => <b className="mono">{s}</b> }, { name: file.name })}
          confirm={t('files.discard.confirm')}
          onConfirm={confirmClose}
          onClose={() => setConfirmClose(undefined)}
        />
      )}
      {conflict && (
        <ConfirmModal
          title={t('files.conflict.title')}
          body={rich('files.conflict.body', { file: (s) => <b className="mono">{s}</b> }, { name: file.name })}
          confirm={t('files.conflict.confirm')}
          onConfirm={() => save(true)}
          onClose={() => setConflict(false)}
        />
      )}
    </div>
  )
}
