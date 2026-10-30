'use strict'

const POLL_MS = 2000
const $ = (id) => document.getElementById(id)
let servers = []
let selected = null
let timer = null

async function post(path, body) {
  const res = await fetch(path, {
    method: 'POST',
    headers: { 'content-type': 'application/json', 'x-resincraft-panel': '1' },
    body: JSON.stringify(body ?? {})
  })
  const data = await res.json().catch(() => ({}))
  if (res.status === 401 && path === '/api/call') showLogin()
  if (!res.ok) throw new Error(data.error || `The panel answered ${res.status}`)
  return data
}

const call = async (method, ...params) => (await post('/api/call', { method, params })).ok

function showLogin() {
  clearTimeout(timer)
  $('app').hidden = true
  $('login').hidden = false
  $('password').focus()
}

function showApp() {
  $('login').hidden = true
  $('app').hidden = false
  refresh()
}

function say(text) {
  $('message').textContent = text || ''
}

function drawList() {
  const list = $('servers')
  list.replaceChildren(
    ...servers.map((s) => {
      const b = document.createElement('button')
      b.className = s.id === selected ? 'server chosen' : 'server'
      const name = document.createElement('span')
      name.textContent = s.name
      const state = document.createElement('small')
      state.textContent = s.state
      b.append(name, state)
      b.addEventListener('click', () => {
        selected = s.id
        $('log').textContent = ''
        say('')
        drawList()
        refresh()
      })
      return b
    })
  )
  $('empty').hidden = servers.length > 0
}

function drawDetail(s) {
  $('detail').hidden = !s
  if (!s) return
  $('name').textContent = s.name
  $('about').textContent = `${s.type} ${s.mcVersion || ''} · port ${s.port}`
  $('state').textContent = s.state
  $('state').dataset.state = s.state
  const on = s.state === 'running' || s.state === 'starting'
  $('start').disabled = on || s.state === 'stopping'
  $('stop').disabled = !on
  $('restart').disabled = !on
  $('kill').disabled = s.state === 'stopped' || s.state === 'crashed'
  $('command').disabled = !on
  const players = s.players || []
  $('players').textContent = players.length ? `Online: ${players.join(', ')}` : ''
}

async function drawLog(id) {
  const out = await call('server.log', id)
  const log = $('log')
  const atEnd = log.scrollHeight - log.scrollTop - log.clientHeight < 40
  log.textContent = out.lines.map((l) => l.text).join('\n')
  if (atEnd) log.scrollTop = log.scrollHeight
}

async function refresh() {
  clearTimeout(timer)
  try {
    servers = await call('server.list')
    if (!servers.some((s) => s.id === selected)) selected = servers[0]?.id ?? null
    drawList()
    const s = servers.find((x) => x.id === selected)
    drawDetail(s)
    if (s) await drawLog(s.id)
  } catch (e) {
    if ($('app').hidden) return
    say(e.message)
  }
  if (!$('app').hidden) timer = setTimeout(refresh, document.hidden ? POLL_MS * 5 : POLL_MS)
}

async function act(method) {
  const id = selected
  say('')
  try {
    await call(method, id)
  } catch (e) {
    if (/EULA/.test(e.message)) $('eula').showModal()
    else say(e.message)
  }
  refresh()
}

$('login-form').addEventListener('submit', async (e) => {
  e.preventDefault()
  $('login-error').textContent = ''
  try {
    await post('/api/login', { password: $('password').value })
    $('password').value = ''
    showApp()
  } catch (err) {
    $('login-error').textContent = err.message
  }
})

$('logout').addEventListener('click', async () => {
  await post('/api/logout').catch(() => {})
  showLogin()
})

for (const [button, method] of [
  ['start', 'server.start'],
  ['stop', 'server.stop'],
  ['restart', 'server.restart'],
  ['kill', 'server.kill']
]) {
  $(button).addEventListener('click', () => act(method))
}

$('eula-accept').addEventListener('click', async () => {
  $('eula').close()
  try {
    await call('server.acceptEula', selected)
    await act('server.start')
  } catch (e) {
    say(e.message)
  }
})
$('eula-cancel').addEventListener('click', () => $('eula').close())

$('command-form').addEventListener('submit', async (e) => {
  e.preventDefault()
  const text = $('command').value
  if (!text.trim()) return
  $('command').value = ''
  try {
    await call('server.command', selected, text)
  } catch (err) {
    say(err.message)
  }
})

async function loadVersions() {
  const kind = $('c-kind').value
  const select = $('c-version')
  select.replaceChildren()
  try {
    let list = await call('server.versions', kind)
    if (!list.length) list = (await call('versions.list')).filter((v) => v.type === 'release').map((v) => v.id)
    select.replaceChildren(...list.map((v) => new Option(v, v)))
  } catch (e) {
    $('create-error').textContent = e.message
  }
}

$('new-server').addEventListener('click', async () => {
  $('create-error').textContent = ''
  $('create').showModal()
  try {
    const o = await call('create.options')
    if (!$('c-kind').options.length) $('c-kind').replaceChildren(...o.kinds.map((k) => new Option(k[0].toUpperCase() + k.slice(1), k)))
    $('c-memory').value = o.memoryMb
    await loadVersions()
  } catch (e) {
    $('create-error').textContent = e.message
  }
})
$('c-kind').addEventListener('change', loadVersions)
$('create-cancel').addEventListener('click', () => $('create').close())

$('create-form').addEventListener('submit', async (e) => {
  e.preventDefault()
  const submit = $('create-submit')
  submit.disabled = true
  submit.textContent = 'Creating…'
  $('create-error').textContent = ''
  try {
    const port = Number($('c-port').value) || undefined
    const made = await call('server.create', {
      name: $('c-name').value.trim(),
      type: $('c-kind').value,
      mcVersion: $('c-version').value,
      memoryMb: Number($('c-memory').value),
      port
    })
    selected = made.server.id
    $('create').close()
    $('create-form').reset()
    refresh()
  } catch (err) {
    $('create-error').textContent = err.message
  }
  submit.disabled = false
  submit.textContent = 'Create'
})

document.addEventListener('visibilitychange', () => {
  if (!document.hidden && !$('app').hidden) refresh()
})

fetch('/api/session')
  .then((r) => r.json())
  .then((s) => (s.signedIn ? showApp() : showLogin()))
  .catch(showLogin)
