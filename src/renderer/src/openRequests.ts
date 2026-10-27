// A backup file Windows asked ResinCraft to open (OpenFiles.tsx), waiting for Settings → Move to another PC to show
// its restore window: the Settings page may not have loaded yet, so the path waits here until it does.

const EVENT = 'resincraft:open-backup'
let backupPath: string | undefined

export function requestBackupOpen(path: string) {
  backupPath = path
  window.dispatchEvent(new Event(EVENT))
}

/** Calls `open` with a waiting backup now and whenever one arrives later; returns the unsubscribe. */
export function onBackupOpen(open: (path: string) => void): () => void {
  const take = () => {
    const p = backupPath
    backupPath = undefined
    if (p) open(p)
  }
  take()
  window.addEventListener(EVENT, take)
  return () => window.removeEventListener(EVENT, take)
}
