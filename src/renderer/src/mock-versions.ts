// Web preview: loader version lists, jar mods and the resolved version (instance settings → Version).
import type { Instance, JarMod, LoaderType, LoaderVersion, ResinApi, ResolvedVersion } from '@shared/types'

const wait = <T>(v: T, ms = 250) => new Promise<T>((r) => setTimeout(() => r(v), ms))

function loaderList(loader: LoaderType, mc: string): LoaderVersion[] {
  const minor = Number(/^1\.(\d+)/.exec(mc)?.[1] ?? 99)
  switch (loader) {
    case 'liteloader':
      if (minor < 5 || minor > 12) return []
      return minor >= 9 ? [{ version: `${mc}-SNAPSHOT`, stable: false }] : [{ version: `${mc}_01`, stable: true }]
    case 'forge':
      if (mc === '1.12.2') return [{ version: '14.23.5.2860', stable: true }, { version: '14.23.5.2859', stable: true }]
      if (mc === '1.5.2') return [{ version: '7.8.1.738', stable: true }]
      if (mc === '1.2.5') return [{ version: '3.4.9.171', stable: true }]
      if (minor < 2 || (minor === 2 && mc !== '1.2.5')) return []
      return [{ version: '47.3.0', stable: true }]
    default:
      return [
        { version: '0.16.10', stable: true },
        { version: '0.16.9', stable: true }
      ]
  }
}

export function mockVersions(get: () => Instance[], set: (list: Instance[]) => void): Omit<ResinApi['versions'], 'list'> {
  const change = (id: string, f: (i: Instance) => Instance) => {
    set(get().map((i) => (i.id === id ? f(i) : i)))
    return get().find((i) => i.id === id)!
  }
  const picks = ['C:\\Users\\you\\Downloads\\OptiFine_1.5.2_HD_U_D5.zip', 'C:\\Users\\you\\Downloads\\TooManyItems2013_05_02_1.5.2.zip']
  return {
    loaders: (loader, mc) => wait(loaderList(loader, mc)),
    resolved: async (id) => {
      const inst = get().find((i) => i.id === id)!
      const p = inst.versionPatch ?? {}
      const legacy = /^1\.[0-5](\.|$)/.test(inst.mcVersion)
      const tweakers = [
        ...(inst.loader === 'forge' && !legacy ? ['net.minecraftforge.fml.common.launcher.FMLTweaker'] : []),
        ...(inst.loader === 'liteloader' || inst.liteloader ? ['com.mumfrey.liteloader.launch.LiteLoaderTweaker'] : []),
        ...(p.tweakers ?? [])
      ]
      const out: ResolvedVersion = {
        mainClass: p.mainClass || (tweakers.length || legacy ? 'net.minecraft.launchwrapper.Launch' : 'net.minecraft.client.main.Main'),
        libraries: [
          ...(p.libraries ?? []).map((l) => l.name),
          ...(inst.liteloader || inst.loader === 'liteloader' ? [`com.mumfrey:liteloader:${inst.liteloader ?? inst.loaderVersion}`] : []),
          ...(inst.loader === 'forge' && !legacy ? [`net.minecraftforge:forge:${inst.mcVersion}-${inst.loaderVersion}`, 'net.minecraft:launchwrapper:1.12'] : []),
          'com.mojang:patchy:1.3.9',
          'oshi-project:oshi-core:1.1',
          'net.java.jinput:jinput:2.0.5',
          'com.google.guava:guava:21.0',
          'org.lwjgl.lwjgl:lwjgl:2.9.4-nightly-20150209'
        ],
        tweakers,
        jvmArgs: p.jvmArgs ?? [],
        gameArgs: ['--username', '${auth_player_name}', '--version', '${version_name}', '--gameDir', '${game_directory}', ...(p.gameArgs ?? [])],
        jarMods: [
          ...(inst.loader === 'forge' && legacy ? [`forge-${inst.mcVersion}-${inst.loaderVersion}-universal.zip`] : []),
          ...(inst.jarMods ?? []).filter((m) => !m.disabled).map((m) => m.name)
        ]
      }
      return wait(out, 700)
    },
    browseJars: () => wait(picks, 200),
    addJarMods: async (id, paths) =>
      change(id, (i) => {
        const added: JarMod[] = paths.map((p) => {
          const name = p.split(/[\\/]/).pop()!
          return { file: name, name }
        })
        return { ...i, jarMods: [...(i.jarMods ?? []), ...added.filter((a) => !i.jarMods?.some((m) => m.file === a.file))] }
      }),
    removeJarMod: async (id, file) => change(id, (i) => ({ ...i, jarMods: (i.jarMods ?? []).filter((m) => m.file !== file) })),
    setJarMods: async (id, list) => change(id, (i) => ({ ...i, jarMods: list })),
    addLibrary: () => wait({ name: 'local:mixin-0.7.11', file: '3f9a1c0d22be/mixin-0.7.11.jar' }, 200)
  }
}
