import { join } from 'node:path'

let root = ''

/** Must be called once at startup, before any other module touches the disk. */
export function initPaths(dataDir: string): void {
  root = dataDir
}

export const paths = {
  get root() {
    return root
  },
  get instances() {
    return join(root, 'instances')
  },
  get libraries() {
    return join(root, 'libraries')
  },
  get assets() {
    return join(root, 'assets')
  },
  get versions() {
    return join(root, 'versions')
  },
  get java() {
    return join(root, 'java')
  },
  get cache() {
    return join(root, 'cache')
  },
  get settings() {
    return join(root, 'settings.json')
  },
  get accounts() {
    return join(root, 'accounts.json')
  }
}

export const instanceDir = (id: string) => join(paths.instances, id)
export const gameDir = (id: string) => join(instanceDir(id), 'minecraft')
export const nativesDir = (id: string) => join(instanceDir(id), 'natives')
