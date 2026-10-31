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
import depcheck from './depcheck'
import discover from './discover'
import engine from './engine'
import files from './files'
import graphs from './graphs'
import home from './home'
import instance from './instance'
import library from './library'
import menu from './menu'
import migrate from './migrate'
import quickjoin from './quickjoin'
import reinstall from './reinstall'
import schedule from './schedule'
import commandlist from './commandlist'
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
  ...instance,
  ...art,
  ...tabs,
  ...files,
  ...menu,
  ...migrate,
  ...servers,
  ...schedule,
  ...commandlist,
  ...depcheck,
  ...graphs,
  ...admin,
  ...relayadmin,
  ...quickjoin,
  ...commands,
  ...updates,
  ...engine
}
export default en
