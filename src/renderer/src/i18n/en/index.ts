// English, the source language. Each file holds one area of the UI; keys are "area.name".
// engine.ts is generated from the Rust engine's tr!() calls (tools/engine-strings.ts).
import accounts from './accounts'
import admin from './admin'
import relayadmin from './relayadmin'
import art from './art'
import backups from './backups'
import app from './app'
import bedrock from './bedrock'
import commands from './commands'
import common from './common'
import discover from './discover'
import engine from './engine'
import files from './files'
import home from './home'
import instance from './instance'
import library from './library'
import menu from './menu'
import native from './native'
import migrate from './migrate'
import friends from './friends'
import quickjoin from './quickjoin'
import reinstall from './reinstall'
import servers from './servers'
import settings from './settings'
import storage from './storage'
import tabs from './tabs'
import updates from './updates'

const en = {
  ...common,
  ...app,
  ...accounts,
  ...settings,
  ...reinstall,
  ...storage,
  ...backups,
  ...home,
  ...library,
  ...bedrock,
  ...discover,
  ...native,
  ...instance,
  ...art,
  ...tabs,
  ...files,
  ...menu,
  ...migrate,
  ...servers,
  ...admin,
  ...relayadmin,
  ...quickjoin,
  ...friends,
  ...commands,
  ...updates,
  ...engine
}
export default en
