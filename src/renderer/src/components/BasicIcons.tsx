// The basic icons: a small set of ready-made pictures for an instance, for when it has no modpack logo and you
// don't want to go and find a picture. Each is a 12×12 drawing in up to three colours on a plain ground.

interface Basic {
  id: string
  /** The ground, then colours 1, 2 and 3. */
  colors: [string, string, string, string]
  /** Twelve rows of twelve: `.` the ground, `1`–`3` a colour. */
  rows: string[]
}

export const BASIC_ICONS: Basic[] = [
  {
    id: 'heart',
    colors: ['#2a0f18', '#ff5c7a', '#ffc2cf', '#ff5c7a'],
    rows: ['............', '..11...11...', '.1221.1111..', '.121111111..', '.111111111..', '.111111111..', '..1111111...', '...11111....', '....111.....', '.....1......', '............', '............']
  },
  {
    id: 'star',
    colors: ['#2a2208', '#ffd447', '#fff3b8', '#ffd447'],
    rows: ['............', '.....11.....', '.....11.....', '....1221....', '.1111221111.', '..11111111..', '...111111...', '...111111...', '..111..111..', '..11....11..', '............', '............']
  },
  {
    id: 'gem',
    colors: ['#08222a', '#22d3ee', '#b9f4fc', '#0e8ea3'],
    rows: ['............', '............', '...111111...', '..12222113..', '.1222211133.', '.1111111333.', '..11111333..', '...111333...', '....1133....', '.....13.....', '............', '............']
  },
  {
    id: 'sword',
    colors: ['#161a22', '#d7dde8', '#ffffff', '#b07a3c'],
    rows: ['.....12.....', '.....12.....', '.....12.....', '.....12.....', '.....12.....', '.....12.....', '.....12.....', '..33333333..', '.....33.....', '.....33.....', '....3333....', '............']
  },
  {
    id: 'tree',
    colors: ['#0c1f12', '#3ddc84', '#9ff5c2', '#8a5a2b'],
    rows: ['............', '....1111....', '...121111...', '..12111111..', '..11111111..', '..11111111..', '...111111...', '.....33.....', '.....33.....', '.....33.....', '....3333....', '............']
  },
  {
    id: 'flame',
    colors: ['#2a1206', '#ff7a3d', '#ffd447', '#ff7a3d'],
    rows: ['............', '.....1......', '....11......', '....111.....', '...1111.1...', '..1111111...', '..11221111..', '.111222111..', '.112222211..', '.112222211..', '..1122211...', '............']
  },
  {
    id: 'potion',
    colors: ['#1c1030', '#d7dde8', '#b26bff', '#8a5a2b'],
    rows: ['............', '....3333....', '.....11.....', '.....11.....', '....1111....', '...111111...', '..12222221..', '..12222221..', '..12222221..', '...122221...', '....1111....', '............']
  },
  {
    id: 'crown',
    colors: ['#241a06', '#ffd447', '#ff5c7a', '#22d3ee'],
    rows: ['............', '............', '.1...11...1.', '.1...11...1.', '.11.1111.11.', '.1111111111.', '.1111111111.', '.1211331121.', '.1111111111.', '.1111111111.', '............', '............']
  },
  {
    id: 'bolt',
    colors: ['#141230', '#ffd447', '#fff3b8', '#ffd447'],
    rows: ['......111...', '.....121....', '....121.....', '...121......', '..1111111...', '.....121....', '....121.....', '...121......', '..111.......', '..11........', '..1.........', '............']
  },
  {
    id: 'peaks',
    colors: ['#0d1a2e', '#f1f1f4', '#5b7fb8', '#ffd447'],
    rows: ['............', '........33..', '........33..', '....1.......', '...111......', '..12221..1..', '.1222221111.', '122222222221', '222222222222', '222222222222', '............', '............']
  },
  {
    id: 'house',
    colors: ['#1d1410', '#e0683c', '#f1e2c6', '#6ea8ff'],
    rows: ['............', '.....11.....', '....1111....', '...111111...', '..11111111..', '.1111111111.', '..22222222..', '..22322322..', '..22222222..', '..22211222..', '..22211222..', '............']
  },
  {
    id: 'shield',
    colors: ['#10182a', '#d7dde8', '#6ea8ff', '#d7dde8'],
    rows: ['............', '..11111111..', '..12222221..', '..12222221..', '..12211221..', '..12211221..', '..12222221..', '...122221...', '...122221...', '....1221....', '.....11.....', '............']
  }
]

/** A project's own logo, as it publishes it on GitHub (its organisation's picture), by organisation name. */
const logoOf = (org: string) => `https://avatars.githubusercontent.com/${org}?s=256`

/**
 * Icons for what an instance runs on. Each loader's real logo is loaded from the project itself when the chooser
 * shows it (nothing of theirs ships with ResinCraft); the 12×12 drawing stands in until it loads, and stays when
 * it can't (offline). Vanilla has no logo to load, so it keeps its drawing.
 */
export const LOADER_ICONS: (Basic & { name: string; logo?: string })[] = [
  {
    id: 'vanilla',
    name: 'Vanilla',
    colors: ['#141a12', '#5fbf4a', '#8b5a2b', '#6e4520'],
    rows: ['............', '..11111111..', '..11111111..', '..11111111..', '..22122122..', '..22222222..', '..22322232..', '..22222222..', '..23222322..', '..22222222..', '............', '............']
  },
  {
    id: 'fabric',
    name: 'Fabric',
    logo: logoOf('FabricMC'),
    colors: ['#1f1a14', '#dbd0b4', '#b0a17e', '#8a7a58'],
    rows: ['............', '..11111111..', '.1212121211.', '.1121212121.', '.1212121211.', '.1121212121.', '.1212121211.', '.1121212121.', '.1212121211.', '.1133333311.', '..11111111..', '............']
  },
  {
    id: 'quilt',
    name: 'Quilt',
    logo: logoOf('QuiltMC'),
    colors: ['#17121f', '#9b6bff', '#e06ad0', '#5ac8fa'],
    rows: ['............', '.11111.2222.', '.11111.2222.', '.11111.2222.', '.11111.2222.', '............', '.3333.11111.', '.3333.11111.', '.3333.11111.', '.3333.11111.', '............', '............']
  },
  {
    id: 'forge',
    name: 'Forge',
    logo: logoOf('MinecraftForge'),
    colors: ['#161a22', '#8d99ae', '#c9d1dc', '#5a6475'],
    rows: ['............', '............', '.2222222222.', '.1111111111.', '..11111111..', '....1111....', '....1111....', '....1111....', '...111111...', '..33333333..', '..33333333..', '............']
  },
  {
    id: 'neoforge',
    name: 'NeoForge',
    logo: logoOf('neoforged'),
    colors: ['#1d130c', '#f08a2c', '#fff1dc', '#2b1a10'],
    rows: ['............', '.11......11.', '.111....111.', '.1111111111.', '.1111111111.', '.1131111311.', '.1111111111.', '.2211111122.', '..22211222..', '...223322...', '....2222....', '............']
  }
]

/** The icon an RTX Remix instance wears: NVIDIA's own logo when it can be loaded, else the letters RTX in its green. */
export const RTX_ICON: Basic & { name: string; logo?: string } = {
  id: 'rtx',
  name: 'RTX Remix',
  logo: logoOf('NVIDIAGameWorks'),
  colors: ['#070a06', '#76b900', '#a4e000', '#76b900'],
  rows: ['............', '............', '............', '.11..111.1.1', '.1.1..1..1.1', '.11...1...1.', '.1.1..1..1.1', '.1.1..1..1.1', '............', '.2222222222.', '............', '............']
}

/**
 * A picture from the web (a modpack's logo) as a 256 px PNG data URL, the form an instance's icon is saved from;
 * undefined when there is none or its site doesn't let the page read it.
 */
export function pictureAsPng(url: string | undefined): Promise<string | undefined> {
  return new Promise((resolve) => {
    if (!url) return resolve(undefined)
    const img = new Image()
    if (!url.startsWith('data:')) img.crossOrigin = 'anonymous'
    img.onerror = () => resolve(undefined)
    img.onload = () => {
      try {
        const canvas = document.createElement('canvas')
        canvas.width = canvas.height = 256
        const side = Math.min(img.naturalWidth, img.naturalHeight)
        const g = canvas.getContext('2d')!
        // Small pixel-art logos stay sharp when enlarged.
        g.imageSmoothingEnabled = side >= 128
        g.drawImage(img, (img.naturalWidth - side) / 2, (img.naturalHeight - side) / 2, side, side, 0, 0, 256, 256)
        resolve(canvas.toDataURL('image/png'))
      } catch {
        resolve(undefined)
      }
    }
    img.src = url
  })
}

/** One basic icon, drawn sharp at any size. */
export function BasicIcon({ icon }: { icon: Basic }) {
  return (
    <svg viewBox="0 0 14 14" shapeRendering="crispEdges" aria-hidden>
      <rect width="14" height="14" fill={icon.colors[0]} />
      {icon.rows.flatMap((row, y) =>
        [...row].map((c, x) => (c === '.' ? null : <rect key={`${x},${y}`} x={x + 1} y={y + 1} width="1" height="1" fill={icon.colors[Number(c)]} />))
      )}
    </svg>
  )
}

/** The icon as a PNG data URL, the form instance pictures are saved from (the engine re-encodes it). */
export function basicIconUrl(icon: Basic): string {
  const cell = 18
  const canvas = document.createElement('canvas')
  canvas.width = canvas.height = cell * 14
  const g = canvas.getContext('2d')!
  g.fillStyle = icon.colors[0]
  g.fillRect(0, 0, canvas.width, canvas.height)
  icon.rows.forEach((row, y) =>
    [...row].forEach((c, x) => {
      if (c === '.') return
      g.fillStyle = icon.colors[Number(c)]
      g.fillRect((x + 1) * cell, (y + 1) * cell, cell, cell)
    })
  )
  return canvas.toDataURL('image/png')
}

/** An icon as a PNG data URL to save: the project's real logo when it loads, else its drawing. */
export async function iconPng(icon: Basic & { logo?: string }): Promise<string> {
  return (await pictureAsPng(icon.logo)) ?? basicIconUrl(icon)
}
