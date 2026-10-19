// ResinCraft Relay admin dashboard. Plain JavaScript, no build step; the relay embeds this file.
// Everything from the relay is shown with textContent, never as HTML: names come from players.
'use strict'

const state = { csrf: null, loginToken: null, baseDomain: '', version: '', tab: 'live', timer: null }
const app = document.getElementById('app')

// ---------- Small helpers ----------

function h(tag, props, ...children) {
  const el = document.createElement(tag)
  for (const [k, v] of Object.entries(props || {})) {
    if (v == null || v === false) continue
    if (k === 'class') el.className = v
    // Through the DOM: the page's Content-Security-Policy blocks inline style attributes.
    else if (k === 'style') Object.assign(el.style, v)
    else if (k.startsWith('on')) el.addEventListener(k.slice(2), v)
    else if (k === 'value') el.value = v
    else el.setAttribute(k, v === true ? '' : v)
  }
  for (const c of children.flat()) {
    if (c == null || c === false) continue
    el.append(c instanceof Node ? c : document.createTextNode(String(c)))
  }
  return el
}

function bytes(n) {
  if (!n) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let i = 0
  while (n >= 1000 && i < units.length - 1) {
    n /= 1000
    i++
  }
  return `${n >= 100 || i === 0 ? Math.round(n) : n.toFixed(1)} ${units[i]}`
}

function ago(unix) {
  if (!unix) return '–'
  const s = Math.max(0, Math.floor(Date.now() / 1000 - unix))
  if (s < 60) return `${s}s ago`
  if (s < 3600) return `${Math.floor(s / 60)}m ago`
  if (s < 86400) return `${Math.floor(s / 3600)}h ${Math.floor((s % 3600) / 60)}m ago`
  return `${Math.floor(s / 86400)}d ago`
}

function when(unix) {
  return unix ? new Date(unix * 1000).toLocaleString() : '–'
}

function left(secs) {
  if (secs == null) return 'no limit'
  if (secs <= 0) return 'ending'
  const hr = Math.floor(secs / 3600)
  const m = Math.ceil((secs % 3600) / 60)
  return hr ? `${hr}h ${m}m` : `${m}m`
}

function duration(secs) {
  const d = Math.floor(secs / 86400)
  const hr = Math.floor((secs % 86400) / 3600)
  const m = Math.floor((secs % 3600) / 60)
  return d ? `${d}d ${hr}h` : hr ? `${hr}h ${m}m` : `${m}m`
}

let toastTimer
function toast(text, bad) {
  const t = document.getElementById('toast')
  t.textContent = text
  t.className = bad ? 'show bad' : 'show'
  clearTimeout(toastTimer)
  toastTimer = setTimeout(() => (t.className = ''), 3500)
}

async function api(path, body) {
  const opts = body === undefined ? {} : { method: 'POST', headers: { 'Content-Type': 'application/json', 'X-CSRF-Token': state.csrf || '' }, body: JSON.stringify(body) }
  const r = await fetch(`/admin${path}`, { credentials: 'same-origin', ...opts })
  const data = await r.json().catch(() => ({}))
  if (r.status === 401 && path !== '/api/login') {
    state.csrf = null
    await start()
    throw new Error('Signed out')
  }
  if (!r.ok) {
    const err = new Error(data.error || r.statusText)
    err.data = data
    err.status = r.status
    throw err
  }
  return data
}

async function act(path, body, done) {
  try {
    await api(path, body)
    if (done) toast(done)
    await render()
  } catch (e) {
    toast(e.message, true)
  }
}

// ---------- Sign-in ----------

async function start() {
  clearInterval(state.timer)
  const s = await api('/api/session')
  state.baseDomain = s.baseDomain
  state.version = s.version
  if (s.loggedIn) {
    state.csrf = s.csrf
    shell()
  } else {
    state.loginToken = s.loginToken
    login(s.passwordSet)
  }
}

function login(passwordSet) {
  const msg = h('p', { class: 'msg' })
  const pw = h('input', { type: 'password', autocomplete: 'current-password', placeholder: 'Admin password', required: true, autofocus: true })
  const btn = h('button', { class: 'primary', type: 'submit' }, 'Sign in')
  const form = h(
    'form',
    {
      class: 'login card',
      onsubmit: async (e) => {
        e.preventDefault()
        btn.disabled = true
        msg.textContent = ''
        try {
          const r = await api('/api/login', { password: pw.value, token: state.loginToken })
          state.csrf = r.csrf
          shell()
        } catch (err) {
          msg.textContent =
            err.status === 429
              ? `Too many wrong passwords. Try again in ${Math.ceil((err.data.retryAfter || 60) / 60)} min.`
              : err.status === 401
                ? `Wrong password.${err.data.attemptsLeft != null ? ` ${err.data.attemptsLeft} tries left before a 15-minute lock.` : ''}`
                : err.message
          const s = await api('/api/session').catch(() => null)
          if (s) state.loginToken = s.loginToken
          btn.disabled = false
          pw.select()
        }
      }
    },
    h('div', { class: 'brand' }, h('img', { src: '/favicon.svg', alt: '' }), h('div', {}, h('b', {}, 'ResinCraft Relay'), h('span', {}, state.baseDomain))),
    passwordSet ? null : h('p', { class: 'warn' }, 'No admin password is set yet. On the server, run: resincraft-relay --config relay.toml set-admin-password'),
    pw,
    btn,
    msg
  )
  app.replaceChildren(form)
  pw.focus()
}

// ---------- Layout ----------

const TABS = [
  ['live', 'Live tunnels'],
  ['names', 'Names'],
  ['bans', 'Bans'],
  ['usage', 'Usage'],
  ['settings', 'Settings'],
  ['events', 'Events']
]

function shell() {
  const nav = h(
    'nav',
    {},
    TABS.map(([id, label]) =>
      h(
        'button',
        {
          class: id === state.tab ? 'on' : null,
          onclick: () => {
            state.tab = id
            shell()
          }
        },
        label
      )
    )
  )
  const main = h('main', { id: 'main' }, h('p', { class: 'muted' }, 'Loading…'))
  app.replaceChildren(
    h(
      'div',
      { class: 'layout' },
      h(
        'aside',
        {},
        h('div', { class: 'brand' }, h('img', { src: '/favicon.svg', alt: '' }), h('div', {}, h('b', {}, 'ResinCraft Relay'), h('span', {}, `${state.baseDomain} · v${state.version}`))),
        nav,
        h('div', { id: 'service', class: 'service card' }),
        h('button', { class: 'ghost', onclick: () => act('/api/logout', {}).then(start) }, 'Sign out')
      ),
      main
    )
  )
  clearInterval(state.timer)
  render()
  renderService()
  if (state.tab === 'live' || state.tab === 'events') state.timer = setInterval(() => render(true), 5000)
}

const SERVICE = {
  on: ['On', 'ok', 'Tunnels come and go as usual.'],
  paused: ['Paused', 'warn', 'No new tunnels; running ones go on until their session ends.'],
  off: ['Off', 'bad', 'No tunnels at all.']
}

async function renderService() {
  const box = document.getElementById('service')
  if (!box) return
  let svc
  try {
    svc = await api('/api/service')
  } catch {
    return
  }
  const [label, cls, hint] = SERVICE[svc.state] || SERVICE.on
  const message = h('input', { placeholder: 'Message for hosts (optional)', value: svc.message || '', maxlength: 200 })
  const flip = (next) => async () => {
    if (next === 'off' && !confirm('Switch the relay off? Every tunnel closes now, and their players are dropped.')) return
    try {
      await api('/api/service', { state: next, message: message.value.trim() })
      toast(`Relay ${SERVICE[next][0].toLowerCase()}`)
      renderService()
      render(true)
    } catch (e) {
      toast(e.message, true)
    }
  }
  box.replaceChildren(
    h('div', { class: 'service-head' }, h('span', {}, 'Relay'), h('b', { class: cls }, label)),
    h('p', { class: 'muted small' }, hint, svc.since ? ` Since ${ago(svc.since)}.` : ''),
    message,
    h(
      'div',
      { class: 'service-buttons' },
      ['on', 'paused', 'off'].map((st) => h('button', { class: `sm${st === svc.state ? ' on' : ''}`, onclick: flip(st) }, st === 'on' ? 'On' : st === 'paused' ? 'Pause' : 'Off'))
    )
  )
}

async function render(quiet) {
  const main = document.getElementById('main')
  if (!main) return
  try {
    const view = { live, names, bans, usage, settings, events }[state.tab]
    const content = await view()
    // Don't wipe what the admin is typing during a background refresh.
    if (quiet && main.contains(document.activeElement) && document.activeElement.tagName === 'INPUT') return
    main.replaceChildren(content)
  } catch (e) {
    if (!quiet) main.replaceChildren(h('p', { class: 'bad' }, e.message))
  }
}

function stat(label, value, cls) {
  return h('div', { class: 'stat card' }, h('span', {}, label), h('b', { class: cls || 'data' }, value))
}

function table(head, rows, empty) {
  if (!rows.length) return h('p', { class: 'muted empty' }, empty)
  return h('div', { class: 'table-wrap' }, h('table', {}, h('thead', {}, h('tr', {}, head.map((c) => h('th', {}, c)))), h('tbody', {}, rows)))
}

function section(title, ...children) {
  return h('section', {}, h('h2', {}, title), ...children)
}

// ---------- Views ----------

async function live() {
  const [o, list] = await Promise.all([api('/api/overview'), api('/api/tunnels')])
  return h(
    'div',
    {},
    h(
      'div',
      { class: 'stats' },
      stat('Relay', (SERVICE[o.service.state] || SERVICE.on)[0], (SERVICE[o.service.state] || SERVICE.on)[1]),
      stat('Tunnels online', o.tunnels, 'ok'),
      stat('Players', o.players),
      stat('Launchers connected', o.launchers),
      stat(`Traffic ${o.month}`, bytes(o.trafficMonth)),
      stat('Names reserved', o.names),
      stat('Up for', duration(o.uptimeSecs))
    ),
    section(
      'Live tunnels',
      table(
        ['Address', 'Minecraft account', 'Online since', 'Time left', 'Players', 'In / out', ''],
        list.map((t) =>
          h(
            'tr',
            {},
            h('td', {}, h('span', { class: 'mono data' }, t.bedrockPort ? `${t.address}:${t.bedrockPort}` : t.address), t.bedrockPort ? h('div', { class: 'faint' }, 'Bedrock') : null),
            h('td', {}, h('div', {}, t.ownerName), h('div', { class: 'mono faint' }, t.owner)),
            h('td', { title: when(t.since) }, ago(t.since)),
            h('td', { class: 'num' }, h('span', { class: t.secsLeft != null && t.secsLeft <= 600 ? 'warn' : 'data' }, left(t.secsLeft)), t.continues ? h('div', { class: 'faint' }, `continued ×${t.continues}`) : null),
            h('td', { class: 'num' }, t.players),
            h('td', { class: 'num' }, `${bytes(t.bytesIn)} / ${bytes(t.bytesOut)}`),
            h(
              'td',
              { class: 'actions' },
              t.secsLeft != null ? h('button', { class: 'sm', onclick: () => act('/api/tunnels/extend', { name: t.name }, `Extended ${t.name}`) }, 'Extend') : null,
              t.secsLeft != null
                ? h('button', { class: 'sm', onclick: () => confirm(`End ${t.address}'s session now? Its players are dropped; the host can start it again.`) && act('/api/tunnels/end', { name: t.name }, `Ended ${t.name}'s session`) }, 'End now')
                : null,
              h('button', { class: 'danger sm', onclick: () => confirm(`Disconnect ${t.address}? Its players are dropped and the host's launcher won't reconnect by itself.`) && act('/api/tunnels/kick', { name: t.name }, `Disconnected ${t.name}`) }, 'Kick')
            )
          )
        ),
        'Nobody is online through the relay right now.'
      )
    )
  )
}

async function names() {
  const q = state.namesQuery || ''
  const d = await api(`/api/names?q=${encodeURIComponent(q)}`)
  const search = h('input', {
    type: 'search',
    placeholder: 'Search names, accounts or UUIDs',
    value: q,
    onkeydown: (e) => {
      if (e.key === 'Enter') {
        state.namesQuery = search.value
        render()
      }
    }
  })
  const word = h('input', { placeholder: 'Add a reserved word', maxlength: 24 })
  return h(
    'div',
    {},
    section(
      `Names (${d.total})`,
      h('div', { class: 'row' }, search, h('button', { onclick: () => ((state.namesQuery = search.value), render()) }, 'Search')),
      table(
        ['Name', 'Owner', 'Reserved', 'Last online', ''],
        d.names.map((n) =>
          h(
            'tr',
            {},
            h('td', {}, h('span', { class: 'mono data' }, n.name), n.online ? h('span', { class: 'chip ok' }, 'online') : null),
            h('td', {}, h('div', {}, n.ownerName), h('div', { class: 'mono faint' }, n.owner)),
            h('td', { title: when(n.created) }, ago(n.created)),
            h('td', { title: when(n.lastUsed) }, ago(n.lastUsed)),
            h(
              'td',
              { class: 'actions' },
              h(
                'button',
                {
                  class: 'sm',
                  onclick: () => {
                    const to = prompt(`Rename ${n.name} to:`, n.name)
                    if (to && to !== n.name) act('/api/names/rename', { name: n.name, to }, `Renamed to ${to}`)
                  }
                },
                'Rename'
              ),
              h('button', { class: 'danger sm', onclick: () => confirm(`Release ${n.name}? Anyone can take it afterwards.`) && act('/api/names/release', { name: n.name }, `Released ${n.name}`) }, 'Release')
            )
          )
        ),
        q ? 'No names match.' : 'No names are reserved yet.'
      )
    ),
    section(
      'Reserved words',
      h('p', { class: 'muted' }, `Nobody can reserve these. ${d.builtinReserved.length} more are built in (www, admin, relay, resincraft…).`),
      h(
        'div',
        { class: 'chips' },
        d.reserved.length
          ? d.reserved.map((w) => h('span', { class: 'chip' }, w, h('button', { class: 'x', title: `Remove ${w}`, onclick: () => act('/api/reserved/remove', { word: w }, `${w} can be reserved again`) }, '×')))
          : h('span', { class: 'muted' }, 'None besides the built-in list.')
      ),
      h(
        'form',
        {
          class: 'row',
          onsubmit: (e) => {
            e.preventDefault()
            if (word.value.trim()) act('/api/reserved/add', { word: word.value.trim() }, `${word.value.trim()} is reserved`)
          }
        },
        word,
        h('button', { type: 'submit' }, 'Reserve word')
      )
    )
  )
}

async function bans() {
  const d = await api('/api/bans')
  const kind = h('select', {}, h('option', { value: 'uuid' }, 'Minecraft UUID'), h('option', { value: 'ip' }, 'IP or CIDR'))
  const value = h('input', { placeholder: '069a79f4-44e9-4726-a5be-fca90e38aaf5 or 203.0.113.0/24', required: true })
  const reason = h('input', { placeholder: 'Reason (optional)' })
  const hours = h('input', { type: 'number', min: 0, step: 1, placeholder: 'Hours (empty: forever)' })
  const until = (b) => (b.expires ? `until ${when(b.expires)}` : 'forever')
  const unban = (body, label) => h('button', { class: 'sm', onclick: () => act('/api/bans/remove', body, `Unbanned ${label}`) }, 'Unban')
  return h(
    'div',
    {},
    section(
      'Ban',
      h(
        'form',
        {
          class: 'row wrap',
          onsubmit: (e) => {
            e.preventDefault()
            const body = { [kind.value]: value.value.trim(), reason: reason.value.trim() }
            if (Number(hours.value) > 0) body.hours = Number(hours.value)
            act('/api/bans/add', body, `Banned ${value.value.trim()}`)
          }
        },
        kind,
        value,
        reason,
        hours,
        h('button', { class: 'danger', type: 'submit' }, 'Ban')
      ),
      h('p', { class: 'muted' }, 'Banning an account disconnects its tunnels at once. IP bans apply to players and launchers.')
    ),
    section(
      'Banned accounts',
      table(
        ['Account', 'Reason', 'Since', 'Ends', ''],
        d.uuids.map((b) => h('tr', {}, h('td', {}, h('div', {}, b.name || '–'), h('div', { class: 'mono faint' }, b.uuid)), h('td', {}, b.reason || '–'), h('td', {}, ago(b.at)), h('td', {}, until(b)), h('td', { class: 'actions' }, unban({ uuid: b.uuid }, b.name || b.uuid)))),
        'No banned accounts.'
      )
    ),
    section(
      'Banned IPs',
      table(
        ['IP / range', 'Reason', 'Since', 'Ends', ''],
        d.ips.map((b) => h('tr', {}, h('td', { class: 'mono' }, b.ip), h('td', {}, b.reason || '–'), h('td', {}, ago(b.at)), h('td', {}, until(b)), h('td', { class: 'actions' }, unban({ ip: b.ip }, b.ip)))),
        'No banned IPs.'
      ),
      d.fromConfig.uuids.length || d.fromConfig.ips.length
        ? h('p', { class: 'muted' }, `Also banned in relay.toml (edit the file to change): ${[...d.fromConfig.uuids, ...d.fromConfig.ips].join(', ')}`)
        : null
    )
  )
}

async function usage() {
  const d = await api('/api/usage')
  return h(
    'div',
    {},
    h('div', { class: 'stats' }, stat(`Total ${d.month}`, bytes(d.total)), stat('Cap per account', d.cap ? bytes(d.cap) : 'none'), stat('Accounts with traffic', d.accounts.length)),
    section(
      'Bandwidth per account this month',
      table(
        ['Account', 'Used', d.cap ? 'Of the cap' : '', 'Online'],
        d.accounts.map((a) =>
          h(
            'tr',
            {},
            h('td', {}, h('div', {}, a.name || '–'), h('div', { class: 'mono faint' }, a.uuid)),
            h('td', { class: 'num data' }, bytes(a.bytes)),
            h('td', {}, d.cap ? h('div', { class: 'meter' }, h('i', { class: a.percent >= 90 ? 'bad' : a.percent >= 70 ? 'warn' : '', style: { width: `${a.percent.toFixed(1)}%` } }), h('span', {}, `${a.percent.toFixed(0)}%`)) : null),
            h('td', {}, a.online ? h('span', { class: 'chip ok' }, `${a.online} online`) : '–')
          )
        ),
        'No traffic yet this month.'
      )
    )
  )
}

const LIMITS = [
  ['tunnels_per_account', 'Servers online per account', 1],
  ['names_per_account', 'Names per account', 1],
  ['connections_per_tunnel', 'Players per server', 1],
  ['connections_per_ip', 'Players per IP (all servers)', 0],
  ['agents_per_ip', 'Launchers per IP', 0],
  ['monthly_gb_per_account', 'Monthly traffic per account (GB, 0 = none)', 0],
  ['handshake_timeout_secs', 'Handshake timeout (s)', 1],
  ['idle_timeout_secs', 'Idle timeout (s)', 10],
  ['logins_per_ip_per_hour', 'Sign-ins per IP per hour', 1],
  ['name_expiry_days', 'Free unused names after (days)', 1],
  ['session_days', 'Launcher sessions last (days)', 1],
  ['max_pending_handshakes', 'Pending handshakes (after a restart)', 1],
  ['session_minutes', 'Session length (minutes, 0 = no limit)', 0]
]

async function settings() {
  const d = await api('/api/settings')
  const inputs = {}
  const rows = LIMITS.map(([key, label, min]) => {
    inputs[key] = h('input', { type: 'number', min, step: key === 'monthly_gb_per_account' || key === 'session_minutes' ? 'any' : 1, value: d.limits[key] })
    const changed = d.limits[key] !== d.fileLimits[key]
    return h('label', { class: 'field' }, h('span', {}, label, changed ? h('em', {}, ` (relay.toml: ${d.fileLimits[key]})`) : null), inputs[key])
  })
  const warn = h('input', { value: d.limits.session_warn_minutes.join(', '), placeholder: '10, 1' })
  const warnChanged = d.limits.session_warn_minutes.join(',') !== d.fileLimits.session_warn_minutes.join(',')
  rows.push(h('label', { class: 'field' }, h('span', {}, 'Warn the host before the end (minutes left, comma-separated)', warnChanged ? h('em', {}, ` (relay.toml: ${d.fileLimits.session_warn_minutes.join(', ')})`) : null), warn))
  return h(
    'div',
    {},
    section(
      'Limits',
      h('p', { class: 'muted' }, d.limitsOverridden ? 'Changed here: these win over relay.toml until you reset them.' : 'These come from relay.toml. Changes made here apply at once and are kept in the data file.'),
      h(
        'form',
        {
          class: 'grid',
          onsubmit: (e) => {
            e.preventDefault()
            const limits = {}
            for (const [key] of LIMITS) limits[key] = Number(inputs[key].value)
            limits.session_warn_minutes = warn.value
              .split(',')
              .map((x) => Number(x.trim()))
              .filter((x) => x > 0)
            act('/api/settings', { limits }, 'Limits saved')
          }
        },
        rows,
        h('div', { class: 'row' }, h('button', { class: 'primary', type: 'submit' }, 'Save limits'), d.limitsOverridden || d.reservedOverridden ? h('button', { type: 'button', onclick: () => confirm('Go back to the values in relay.toml (limits and reserved words)?') && act('/api/settings/reset', {}, 'Back to relay.toml') }, 'Reset to relay.toml') : null)
      )
    ),
    section('Reserved words', h('p', { class: 'muted' }, 'Managed on the Names page.'))
  )
}

async function events() {
  const list = (await api('/api/events')).slice().reverse()
  const filter = state.eventFilter || ''
  const kinds = [...new Set(list.map((e) => e.kind))].sort()
  const select = h(
    'select',
    {
      onchange: () => {
        state.eventFilter = select.value
        render()
      }
    },
    h('option', { value: '' }, 'All events'),
    kinds.map((k) => h('option', { value: k, selected: k === filter }, k))
  )
  const shown = filter ? list.filter((e) => e.kind === filter) : list
  return section(
    'Events',
    h('div', { class: 'row' }, select, h('span', { class: 'muted' }, `The last ${list.length} (up to 500, cleared when the relay restarts). IP addresses appear only here.`)),
    table(
      ['Time', 'Event', 'What', 'IP'],
      shown.map((e) => h('tr', {}, h('td', { title: when(e.at) }, ago(e.at)), h('td', {}, h('span', { class: `chip ${e.kind}` }, e.kind)), h('td', {}, e.text), h('td', { class: 'mono faint' }, e.ip || ''))),
      'Nothing has happened yet.'
    )
  )
}

start().catch((e) => app.replaceChildren(h('p', { class: 'bad boot' }, `Couldn't reach the relay: ${e.message}`)))
