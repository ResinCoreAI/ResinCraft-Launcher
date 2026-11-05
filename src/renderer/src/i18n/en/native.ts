// Horizon native mods: code compiled into the game with Horizon's own launcher before a Horizon instance plays.
export default {
  'native.chip': 'Native code',
  'native.chipHint': 'A native mod: code compiled into the game before the instance plays. It runs with full access to this computer.',
  'native.alwaysOn': 'Always on',
  'native.alwaysOnHint': 'Native mods are always on, as in Horizon. Remove one to stop using it.',
  'native.trust.title': 'Native mods run with full access to this computer',
  'native.trust.body':
    "They aren't sandboxed: once built, their code can read and change your files and use your network like any program you run. Build only mods you trust. These will be compiled into the game:",
  'native.trust.build': 'I trust these mods: build',
  'native.playWithout': 'Play without native mods',
  'native.buildAnyway': 'Build anyway',
  'native.buildAnywayHint':
    'Builds mods made for different Horizon versions together, on the newest of them. Some may not compile or may not work there.',
  'native.build.title': 'Build native mods for {name}',
  'native.need.firstBuild': 'This instance has native mods that aren\'t built into its game yet.',
  'native.need.modsChanged': 'Its native mods changed since the last build.',
  'native.need.baseChanged': 'It plays another Horizon build now, so its native mods are built again.',
  'native.firstBuildHint':
    "A build downloads Horizon's own Rust compiler and compiles the whole game: expect 10 to 30 minutes and a few GB of disk space while it runs. The compile files are deleted afterwards, unless Settings keeps them for quick rebuilds. The same mods never compile twice.",
  'native.busy': 'A native build is running. Try again when it finishes.',
  'native.building': 'Building… You can keep using ResinCraft; the build goes on in the background.',
  'native.logTail': 'The end of the build log:',
  'native.install.title': 'Install native code?',
  'native.install.body':
    "Native mods are code compiled into the game. They run with full access to this computer, outside any sandbox. Nothing is compiled now: ResinCraft asks again, in red, before every build.",
  'native.install.confirm': 'Install',
  'native.bar.upToDate': 'Native mods are built into this game',
  'native.bar.needsBuild': 'Native mods need building',
  'native.bar.hint': 'They are compiled into the game before the instance plays.',
  'native.bar.build': 'Build',
  'native.bar.rebuild': 'Build again',
  'native.settings.title': 'Native mods',
  'native.settings.lead':
    "Horizon native mods are compiled with Horizon's own launcher. Finished builds are kept, so the same mods never compile twice. The compile files are deleted after each build unless you keep them.",
  'native.settings.size': 'Compile files',
  'native.settings.sizeValue': '{size} GiB in {path}',
  'native.settings.none': 'Nothing yet: no native mod has been built here.',
  'native.settings.clear': 'Delete compile files',
  'native.settings.cleared': 'Freed {size} GiB. The next new build compiles the whole game.',
  'native.settings.limit': 'Cache limit',
  'native.settings.limitHint': 'In GiB, checked after each build. 0 never clears it.',
  'native.settings.keep': 'Keep compile files for quick rebuilds',
  'native.settings.keepHint':
    'On: a changed mod rebuilds in a few minutes, for several GB of disk space. Off: the files are deleted after each build, and each new set of mods compiles the whole game again.',
  'native.settings.builds': 'Finished builds',
  'native.settings.buildsValue': '{size} GiB, sharing their files with the Horizon builds they start from'
}
