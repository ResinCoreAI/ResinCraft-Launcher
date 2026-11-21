import type { Key } from './i18n'

/**
 * What the Settings page holds, for its own side list and for the quick search (components/QuickSearch.tsx),
 * which jumps straight to a section or to one setting in it. Kept apart from the page so the search can list
 * settings without loading the page.
 */

/** The sections, in page order; `id` is the section's element id on the page. */
export const SETTING_SECTIONS: { id: string; label: Key }[] = [
  { id: 'appearance', label: 'settings.nav.appearance' },
  { id: 'game', label: 'settings.nav.game' },
  { id: 'java', label: 'settings.nav.java' },
  { id: 'commands', label: 'commands.nav' },
  { id: 'downloads', label: 'settings.nav.downloads' },
  { id: 'storage', label: 'storage.nav' },
  { id: 'native-mods', label: 'native.settings.title' },
  { id: 'autobackup', label: 'settings.autoBackup' },
  { id: 'backup', label: 'backup.nav' },
  { id: 'remote', label: 'settings.remote' },
  { id: 'mcp', label: 'settings.mcp' },
  { id: 'updates', label: 'updates.nav' },
  { id: 'about', label: 'settings.nav.about' }
]

/**
 * One setting: its label exactly as its row shows it (the search finds the row on the page by that text), its
 * hint (searched too, so "RAM" or "tray" find the right switch), and the id of the section it is in. A new
 * setting needs a line here to be found.
 */
export const SETTINGS_INDEX: { label: Key; hint?: Key; section: string }[] = [
  { label: 'settings.theme', section: 'appearance' },
  { label: 'settings.language', hint: 'settings.language.hint', section: 'appearance' },
  { label: 'memory.setting', hint: 'memory.settingHint', section: 'appearance' },
  { label: 'settings.tray', hint: 'settings.tray.hint', section: 'appearance' },
  { label: 'settings.freeMemory', hint: 'settings.freeMemory.hint', section: 'appearance' },
  { label: 'settings.guide', hint: 'settings.guide.hint', section: 'appearance' },
  { label: 'settings.winButtons', hint: 'settings.winButtons.hint', section: 'appearance' },

  { label: 'settings.memory', section: 'game' },
  { label: 'settings.windowSize', hint: 'settings.windowSize.hint', section: 'game' },
  { label: 'settings.onLaunch', hint: 'settings.onLaunch.hint', section: 'game' },
  { label: 'settings.launchScreen', hint: 'settings.launchScreen.hint', section: 'game' },
  { label: 'settings.snapshots', hint: 'settings.snapshots.hint', section: 'game' },
  { label: 'settings.gamepad', hint: 'settings.gamepad.hint', section: 'game' },
  { label: 'settings.discord', section: 'game' },
  { label: 'settings.discordServer', hint: 'settings.discordServer.hint', section: 'game' },
  { label: 'settings.discordLauncher', hint: 'settings.discordLauncher.hint', section: 'game' },

  { label: 'settings.autoJava', hint: 'settings.autoJava.hint', section: 'java' },
  { label: 'settings.javaToUse', section: 'java' },
  { label: 'settings.flags', hint: 'settings.flags.hint', section: 'java' },
  { label: 'settings.zgc', hint: 'settings.zgc.hint', section: 'java' },
  { label: 'settings.jvmArgs', hint: 'settings.jvmArgs.hint', section: 'java' },

  { label: 'settings.parallel', hint: 'settings.parallel.hint', section: 'downloads' },
  { label: 'settings.preferredSource', hint: 'settings.preferredSource.hint', section: 'downloads' },
  { label: 'settings.watchFolders', hint: 'settings.watchFolders.hint', section: 'downloads' },
  { label: 'packUpdates.setting', hint: 'packUpdates.settingHint', section: 'pack-updates' },

  { label: 'native.settings.size', section: 'native-mods' },
  { label: 'native.settings.keep', hint: 'native.settings.keepHint', section: 'native-mods' },
  { label: 'native.settings.builds', section: 'native-mods' },
  { label: 'native.settings.limit', hint: 'native.settings.limitHint', section: 'native-mods' },

  { label: 'settings.autoBackup.on', hint: 'settings.autoBackup.onHint', section: 'autobackup' },
  { label: 'settings.autoBackup.every', hint: 'settings.autoBackup.everyHint', section: 'autobackup' },
  { label: 'settings.autoBackup.keep', hint: 'settings.autoBackup.keepHint', section: 'autobackup' },

  { label: 'backup.export', hint: 'backup.exportHint', section: 'backup' },
  { label: 'backup.gameFiles', hint: 'backup.gameFilesHint', section: 'backup' },
  { label: 'backup.import', hint: 'backup.importHint', section: 'backup' },
  { label: 'backup.useSettings', hint: 'backup.useSettingsHint', section: 'backup' },

  { label: 'settings.remote.switch', hint: 'settings.remote.hint', section: 'remote' },
  { label: 'settings.mcp.switch', hint: 'settings.mcp.hint', section: 'mcp' },

  { label: 'updates.version', section: 'updates' },
  { label: 'updates.auto', hint: 'updates.auto.hint', section: 'updates' },
  { label: 'updates.silent', hint: 'updates.silent.hint', section: 'updates' },
  { label: 'updates.channel', hint: 'updates.channel.hint', section: 'updates' },

  { label: 'settings.reset', hint: 'settings.reset.hint', section: 'reset' },
  { label: 'settings.uninstall', hint: 'settings.uninstall.hint', section: 'reset' }
]
