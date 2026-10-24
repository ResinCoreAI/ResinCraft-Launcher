// Reinstall: an instance's game or modpack files, a server's software or modpack files, and ResinCraft itself.
export default {
  'reinstall.menu': 'Reinstall',
  'reinstall.menu.game': 'Game files',
  'reinstall.menu.pack': 'Modpack files',
  'reinstall.menu.server': 'Server software',
  'reinstall.section': 'Repair',
  'reinstall.section.hint': "If the game won't start because files are missing or broken, reinstall them. Your worlds stay.",
  'reinstall.confirm': 'Reinstall',
  'reinstall.done': '{name} was reinstalled',
  'reinstall.game.title': 'Reinstall game files for {name}?',
  'reinstall.game.body':
    'ResinCraft downloads Minecraft, the mod loader, libraries and assets again and checks every file. Your worlds, mods, settings and screenshots stay.',
  'reinstall.pack.title': 'Reinstall modpack files for {name}?',
  'reinstall.pack.body':
    "ResinCraft puts the modpack's own mods, configs and scripts back as this version has them. Files you changed are backed up first. Your worlds, game settings, screenshots and the mods you added yourself stay.",
  'reinstall.server.title': 'Reinstall the server software for {name}?',
  'reinstall.server.body':
    'ResinCraft installs the Minecraft server and its mod loader or plugin platform again. The world, mods, plugins, server settings and player lists stay.',
  'reinstall.serverPack.title': 'Reinstall modpack files for {name}?',
  'reinstall.serverPack.body':
    "ResinCraft puts the modpack's server files back as this version has them. Files you changed are backed up first. The world, server settings, player lists and the mods you added stay.",
  'reinstall.app.title': 'Reinstall ResinCraft',
  'reinstall.app.hint': 'Installs ResinCraft again over itself, to repair the app. Instances, accounts and settings stay.',
  'reinstall.app.body':
    'ResinCraft downloads its newest release, installs it in place and restarts. Your instances, accounts, servers and settings stay where they are.',
  'reinstall.app.button': 'Reinstall',
  'reinstall.app.none': "Couldn't find a release to reinstall. Try again later.",
  'reinstall.app.failed': "Couldn't reach the update server. Check your internet connection and try again."
} as const
