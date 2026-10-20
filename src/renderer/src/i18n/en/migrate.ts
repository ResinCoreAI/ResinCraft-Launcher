// Strings for this area of the UI (see ../index.tsx for the format).
// Move from another launcher (components/MigrateModal.tsx).
export default {
  'migrate.button': 'Move from another launcher',
  'migrate.title': 'Move from another launcher',
  'migrate.subtitle':
    'Copies instances from Prism, MultiMC, Modrinth, CurseForge, ATLauncher, GDLauncher or the Minecraft Launcher into ResinCraft, with their mods, worlds, packs and settings. The originals stay as they are.',
  'migrate.serverButton': 'Import a server',
  'migrate.serversTitle': 'Import a server',
  'migrate.serversSubtitle': 'Copies a server folder into ResinCraft with its worlds, plugins, mods and settings. The original stays as it is.',
  'migrate.scanning': 'Looking for other launchers…',
  'migrate.none.title': 'No other launchers found',
  'migrate.none.body':
    'Nothing turned up in the usual places. If you keep another launcher somewhere else (a portable install, for example), choose its folder.',
  'migrate.none.serversTitle': 'Choose a server folder',
  'migrate.none.serversBody': 'Pick the folder of a server you ran before: the one with server.properties or the server jar in it.',
  'migrate.noServerThere': "There's no server in that folder",
  'migrate.chooseFolder': 'Choose a folder…',
  'migrate.selectAll': 'Select everything from {name}',
  'migrate.folder': 'Folder',
  'migrate.found': { one: '{count} found', other: '{count} found' },
  'migrate.server': 'Server',
  'migrate.imported': 'Moved before',
  'migrate.unknownVersion': 'Unknown version',
  'migrate.mods': { one: '{count} mod', other: '{count} mods' },
  'migrate.plugins': { one: '{count} plugin', other: '{count} plugins' },
  'migrate.worlds': { one: '{count} world', other: '{count} worlds' },
  'migrate.import': { one: 'Import {count}', other: 'Import {count}' },
  'migrate.importing': 'Copying… Follow along, or cancel, in the task list.',
  'migrate.tip': 'Close the other launcher first, so nothing is in use while it copies.',
  'migrate.done.title': 'Moved to ResinCraft',
  'migrate.done.toast': { one: 'Moved {count} item to ResinCraft', other: 'Moved {count} items to ResinCraft' },
  'migrate.done.skipped': { one: "{count} file couldn't be read and was left behind", other: "{count} files couldn't be read and were left behind" }
}
