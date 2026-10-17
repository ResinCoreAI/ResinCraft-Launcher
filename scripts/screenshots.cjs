// Captures crisp marketing screenshots of the UI (sample data from the mock API) for the website/README.
//   npm run build:web && npx electron scripts/screenshots.cjs
// Output: resources/screenshots/*.png at 2880×1800 (16:10, 2× scale), English UI, dark theme.
const { app, BrowserWindow } = require('electron')
const { mkdirSync, writeFileSync } = require('node:fs')
const { join } = require('node:path')

app.commandLine.appendSwitch('lang', 'en-US')
const OUT = join(__dirname, '..', 'resources', 'screenshots')
const wait = (ms) => new Promise((r) => setTimeout(r, ms))

app.whenReady().then(async () => {
  mkdirSync(OUT, { recursive: true })
  // A 2880×1800 window rendered at 200% zoom = a 1440×900 layout captured at 2× sharpness.
  const win = new BrowserWindow({ show: false, width: 2880, height: 1800, useContentSize: true, enableLargerThanScreen: true, backgroundColor: '#0a0912', webPreferences: { backgroundThrottling: false, zoomFactor: 2 } })
  win.setContentSize(2880, 1800)
  await win.loadFile(join(__dirname, '..', 'out', 'web', 'index.html'))
  const js = (code) => win.webContents.executeJavaScript(`(async () => { ${code} })()`)
  const nav = (label) => js(`[...document.querySelectorAll('.nav-item')].find(b => b.textContent.trim().startsWith(${JSON.stringify(label)})).click()`)
  const shot = async (name, settle = 900) => {
    await wait(settle)
    const img = await win.webContents.capturePage()
    writeFileSync(join(OUT, `${name}.png`), img.toPNG())
    console.log('saved', name, img.getSize())
  }
  win.webContents.setZoomFactor(2)
  await wait(3500)

  await shot('home')
  await nav('Library')
  await shot('library')
  await js(`document.querySelector('.icard').click()`)
  await shot('instance-mods', 1600)
  await js(`document.querySelector('.ver-btn').click()`)
  await shot('version-picker', 2200)
  await js(`document.querySelector('.modal-head .btn').click()`)
  await nav('Discover')
  await wait(800)
  await js(`const c = document.querySelectorAll('.check'); c[0]?.click(); c[2]?.click()`)
  await shot('discover', 1600)
  app.quit()
})
