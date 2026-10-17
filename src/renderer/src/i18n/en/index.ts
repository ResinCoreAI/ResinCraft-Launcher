// English, the source language. Each file holds one area of the UI; keys are "area.name".
// engine.ts is generated from the Rust engine's tr!() calls (tools/engine-strings.ts).
import accounts from './accounts'
import app from './app'
import common from './common'
import discover from './discover'
import engine from './engine'
import home from './home'
import instance from './instance'
import library from './library'
import servers from './servers'
import settings from './settings'
import tabs from './tabs'

const en = { ...common, ...app, ...accounts, ...settings, ...home, ...library, ...discover, ...instance, ...tabs, ...servers, ...engine }
export default en
