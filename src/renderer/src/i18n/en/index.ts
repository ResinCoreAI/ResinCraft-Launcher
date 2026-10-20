// English, the source language. Each file holds one area of the UI; keys are "area.name".
// engine.ts is generated from the Rust engine's tr!() calls (tools/engine-strings.ts).
import accounts from './accounts'
import art from './art'
import app from './app'
import bedrock from './bedrock'
import common from './common'
import discover from './discover'
import engine from './engine'
import files from './files'
import home from './home'
import instance from './instance'
import library from './library'
import menu from './menu'
import migrate from './migrate'
import quickjoin from './quickjoin'
import servers from './servers'
import settings from './settings'
import tabs from './tabs'
import updates from './updates'

const en = {
  ...common,
  ...app,
  ...accounts,
  ...settings,
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
  ...quickjoin,
  ...updates,
  ...engine
}
export default en
