// English, the source language. Each file holds one area of the UI; keys are "area.name".
import accounts from './accounts'
import app from './app'
import common from './common'
import discover from './discover'
import home from './home'
import instance from './instance'
import library from './library'
import settings from './settings'
import tabs from './tabs'

const en = { ...common, ...app, ...accounts, ...settings, ...home, ...library, ...discover, ...instance, ...tabs }
export default en
