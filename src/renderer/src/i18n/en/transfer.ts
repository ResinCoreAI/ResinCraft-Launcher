// Cloning a server, and a server as a zip (components/ServerTransfer.tsx).
export default {
  'servers.transfer.clone': 'Clone…',
  'servers.transfer.export': 'Export as zip…',
  'servers.transfer.import': 'Import from zip',
  'servers.transfer.cloneTitle': 'Clone {name}',
  'servers.transfer.cloneBody':
    'Makes a new server with its own port, with the same mods, plugins and settings. Relay names, addresses, domains, server list entries and whitelist requests stay with the original. Its scheduled tasks come along switched off. You agree to the Minecraft EULA again before the first start.',
  'servers.transfer.name': 'Name',
  'servers.transfer.worlds': 'Include worlds',
  'servers.transfer.worldsHint': 'Off: the new server makes a fresh world from the same settings.',
  'servers.transfer.cloneButton': 'Clone',
  'servers.transfer.cloned': 'Made {name}',
  'servers.transfer.exportTitle': 'Export {name}',
  'servers.transfer.exportBody':
    'Saves the server files as a zip you can import into ResinCraft on another PC. Relay details, tokens, key files and the EULA agreement are left out, and passwords in config files are blanked. Check plugin configs by hand before sharing: one that keeps a password somewhere unusual can slip through.',
  'servers.transfer.logs': 'Include logs',
  'servers.transfer.logsHint': 'Logs can hold player addresses and chat.',
  'servers.transfer.backups': 'Include backups',
  'servers.transfer.caches': 'Include caches',
  'servers.transfer.cachesHint': 'Files the server can download again on its next start.',
  'servers.transfer.exportButton': 'Export',
  'servers.transfer.exported': 'Saved {file}',
  'servers.transfer.imported': 'Imported {name}. Agree to the Minecraft EULA when you first start it.',
  'servers.transfer.importedNoEula': 'Imported {name}',
  'servers.transfer.skipped': '{count} files couldn’t be copied: {first}'
} as const
