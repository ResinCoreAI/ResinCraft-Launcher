// Strings for this area of the UI (see ../index.tsx for the format).
// The mod and plugin dependency check. Keyed under servers.* so other languages fall back to English (Dict's NewKey).
export default {
  'servers.deps.check': 'Check dependencies',
  'servers.deps.title': { one: '{count} dependency is missing', other: '{count} dependencies are missing' },
  'servers.deps.allGood': { one: 'The {count} file checked has everything it needs', other: 'All {count} files checked have everything they need' },
  'servers.deps.titleUnread': 'Nothing is missing',
  'servers.deps.subtitle': { one: '{count} file checked', other: '{count} files checked' },
  'servers.deps.neededBy': 'Needed by {names}',
  'servers.deps.turnedOff': 'Turned off in this folder: {file}. Turn it on to fix this.',
  'servers.deps.install': 'Install missing',
  'servers.deps.installed': { one: 'Installed {names}', other: 'Installed {count}: {names}' },
  'servers.deps.reason.notOnModrinth': 'Not on Modrinth under this exact name, so nothing was installed.',
  'servers.deps.reason.noVersion': 'On Modrinth, but with no version for this loader and game version.',
  'servers.deps.reason.notSameMod': "A Modrinth project has this name, but its file doesn't declare this id, so it wasn't installed.",
  'servers.deps.reason.disabled': 'Left alone: a copy is already in this folder, turned off.',
  'servers.deps.failed': 'Failed: {error}',
  'servers.deps.unread': {
    one: "{count} file had nothing to read for this loader, so it wasn't checked: {names}",
    other: "{count} files had nothing to read for this loader, so they weren't checked: {names}"
  },
  'servers.deps.versionsHint': "Only names are compared: a dependency that's there in the wrong version isn't listed."
} as const
