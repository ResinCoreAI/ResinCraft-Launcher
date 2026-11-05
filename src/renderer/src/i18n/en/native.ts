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
  'native.build.title': 'Build native mods for {name}',
  'native.need.firstBuild': 'This instance has native mods that aren\'t built into its game yet.',
  'native.need.modsChanged': 'Its native mods changed since the last build.',
  'native.need.baseChanged': 'It plays another Horizon build now, so its native mods are built again.',
  'native.firstBuildHint':
    "The first build downloads Horizon's own Rust compiler and the game's source and compiles the whole game: expect 10 to 30 minutes and several GB of disk space. Later builds only compile what changed.",
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
    "Horizon native mods are compiled with Horizon's own launcher. Its compile cache makes later builds quick; it's cleared after a build when it grows past the limit.",
  'native.settings.size': 'Compile cache',
  'native.settings.sizeValue': '{size} GiB in {path}',
  'native.settings.none': 'Nothing yet: no native mod has been built here.',
  'native.settings.clear': 'Clear native build cache',
  'native.settings.cleared': 'Cleared {size} GiB. The next build compiles from scratch.',
  'native.settings.limit': 'Cache limit',
  'native.settings.limitHint': 'In GiB, checked after each build. 0 never clears it.'
}
