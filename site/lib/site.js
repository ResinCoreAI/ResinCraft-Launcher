// Shared facts for every page. Set SITE_URL when building for your domain:
//   SITE_URL=https://your.domain npm run build
export const SITE_URL = (process.env.SITE_URL || 'https://resincraft.resincore.tech').replace(/\/+$/, '')

export const REPO = 'ResinCoreAI/ResinCraft-Launcher'
export const GITHUB = `https://github.com/${REPO}`
export const LATEST = `${GITHUB}/releases/latest`
export const EMAIL = 'puirkle@resincore.ai'
export const RELAY_EXAMPLE = 'yourname.rscraft.resincore.tech'

export const DISCLAIMER = 'NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.'
