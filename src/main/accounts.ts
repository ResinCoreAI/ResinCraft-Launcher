import { createHash, randomUUID } from 'node:crypto'
import { app, safeStorage } from 'electron'
import type { AccountView, DeviceCode, LoginEvent } from '@shared/types'
import { emit } from './events'
import { errorMessage, readJson, writeJson } from './fsutil'
import type { LaunchAuth } from './launch'
import { fetchJson, HttpError, postJson } from './net'
import { paths } from './paths'
import { getSettings } from './settings'

// Microsoft sign-in: device-code flow → Xbox Live → XSTS → Minecraft services.
const MS_BASE = 'https://login.microsoftonline.com/consumers/oauth2/v2.0'
const SCOPE = 'XboxLive.signin offline_access'

interface StoredAccount {
  id: string
  type: 'msa' | 'offline'
  username: string
  uuid: string
  skinUrl?: string
  active: boolean
  /** Encrypted with the OS keychain when available. */
  refreshToken?: string
  mcToken?: string
  mcTokenExpires?: number
  xuid?: string
}

// ---------- Storage ----------

const seal = (s: string) =>
  safeStorage.isEncryptionAvailable() ? `enc:${safeStorage.encryptString(s).toString('base64')}` : `raw:${Buffer.from(s).toString('base64')}`

const unseal = (s?: string) => {
  if (!s) return ''
  const [kind, data] = [s.slice(0, 4), s.slice(4)]
  return kind === 'enc:' ? safeStorage.decryptString(Buffer.from(data, 'base64')) : Buffer.from(data, 'base64').toString()
}

const load = () => readJson<StoredAccount[]>(paths.accounts, [])
const save = (list: StoredAccount[]) => writeJson(paths.accounts, list)

const view = ({ id, type, username, uuid, skinUrl, active }: StoredAccount): AccountView => ({ id, type, username, uuid, skinUrl, active })

export async function listAccounts(): Promise<AccountView[]> {
  return (await load()).map(view)
}

export async function setActiveAccount(id: string): Promise<void> {
  await save((await load()).map((a) => ({ ...a, active: a.id === id })))
}

export async function removeAccount(id: string): Promise<void> {
  const list = (await load()).filter((a) => a.id !== id)
  if (list.length && !list.some((a) => a.active)) list[0].active = true
  await save(list)
}

async function upsert(acc: StoredAccount): Promise<StoredAccount> {
  const list = (await load()).filter((a) => a.uuid !== acc.uuid || a.type !== acc.type).map((a) => ({ ...a, active: false }))
  list.unshift({ ...acc, active: true })
  await save(list)
  return acc
}

// ---------- Microsoft / Xbox / Minecraft ----------

async function clientId(): Promise<string> {
  const id = (await getSettings()).msClientId.trim()
  if (!id) throw new Error('Microsoft sign-in needs an Azure Client ID. Add yours under Settings → Microsoft sign-in.')
  return id
}

async function msForm<T>(path: string, body: Record<string, string>): Promise<T> {
  const res = await fetch(`${MS_BASE}/${path}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body: new URLSearchParams(body)
  })
  const json = (await res.json()) as T & { error?: string; error_description?: string }
  if (!res.ok && !json.error) throw new HttpError(res.status, path, '')
  return json
}

const XSTS_ERRORS: Record<string, string> = {
  '2148916227': 'This Xbox account is banned.',
  '2148916233': "This Microsoft account doesn't have an Xbox profile yet. Sign in once at xbox.com, then try again.",
  '2148916235': "Xbox Live isn't available in this account's country.",
  '2148916236': 'This account needs adult verification on xbox.com (South Korea).',
  '2148916237': 'This account needs adult verification on xbox.com (South Korea).',
  '2148916238': 'This is a child account. An adult needs to add it to a Microsoft Family group first.'
}

async function minecraftLogin(msAccessToken: string) {
  const xbl = await postJson<{ Token: string; DisplayClaims: { xui: { uhs: string }[] } }>('https://user.auth.xboxlive.com/user/authenticate', {
    Properties: { AuthMethod: 'RPS', SiteName: 'user.auth.xboxlive.com', RpsTicket: `d=${msAccessToken}` },
    RelyingParty: 'http://auth.xboxlive.com',
    TokenType: 'JWT'
  })

  let xsts: { Token: string; DisplayClaims: { xui: { uhs: string; xid?: string }[] } }
  try {
    xsts = await postJson('https://xsts.auth.xboxlive.com/xsts/authorize', {
      Properties: { SandboxId: 'RETAIL', UserTokens: [xbl.Token] },
      RelyingParty: 'rp://api.minecraftservices.com/',
      TokenType: 'JWT'
    })
  } catch (e) {
    const code = e instanceof HttpError ? /"XErr"\s*:\s*(\d+)/.exec(e.body)?.[1] : undefined
    throw new Error((code && XSTS_ERRORS[code]) || `Xbox sign-in failed: ${errorMessage(e)}`)
  }
  const uhs = xsts.DisplayClaims.xui[0].uhs

  let mc: { access_token: string; expires_in: number }
  try {
    mc = await postJson('https://api.minecraftservices.com/authentication/login_with_xbox', { identityToken: `XBL3.0 x=${uhs};${xsts.Token}` })
  } catch (e) {
    if (e instanceof HttpError && e.status === 403)
      throw new Error("Minecraft rejected this app's Client ID. New Azure apps must be approved by Mojang before they can sign in to Minecraft.")
    throw e
  }

  let profile: { id: string; name: string; skins?: { url: string; state: string }[] }
  try {
    profile = await fetchJson('https://api.minecraftservices.com/minecraft/profile', { headers: { Authorization: `Bearer ${mc.access_token}` } })
  } catch (e) {
    if (e instanceof HttpError && e.status === 404) throw new Error("This Microsoft account doesn't own Minecraft: Java Edition.")
    throw e
  }

  return {
    username: profile.name,
    uuid: profile.id,
    skinUrl: profile.skins?.find((s) => s.state === 'ACTIVE')?.url,
    mcToken: mc.access_token,
    mcTokenExpires: Date.now() + mc.expires_in * 1000,
    xuid: xsts.DisplayClaims.xui[0].xid
  }
}

let loginAbort: AbortController | undefined

/** Starts the device-code flow; the result arrives later as a `login` event. */
export async function beginMicrosoftLogin(): Promise<DeviceCode> {
  loginAbort?.abort()
  const abort = (loginAbort = new AbortController())
  const client = await clientId()
  const dc = await msForm<{
    device_code: string
    user_code: string
    verification_uri: string
    expires_in: number
    interval: number
    error_description?: string
  }>('devicecode', { client_id: client, scope: SCOPE })
  if (!dc.device_code) throw new Error(dc.error_description ?? 'Microsoft sign-in could not start')

  void (async () => {
    let interval = (dc.interval || 5) * 1000
    const deadline = Date.now() + dc.expires_in * 1000
    try {
      while (Date.now() < deadline) {
        await new Promise((r) => setTimeout(r, interval))
        if (abort.signal.aborted) return
        const tok = await msForm<{ access_token?: string; refresh_token?: string; error?: string; error_description?: string }>('token', {
          grant_type: 'urn:ietf:params:oauth:grant-type:device_code',
          client_id: client,
          device_code: dc.device_code
        })
        if (tok.error === 'authorization_pending') continue
        if (tok.error === 'slow_down') {
          interval += 5000
          continue
        }
        if (tok.error === 'authorization_declined') throw new Error('Sign-in was cancelled in the browser.')
        if (tok.error || !tok.access_token) throw new Error(tok.error_description ?? 'Microsoft sign-in failed')
        const mc = await minecraftLogin(tok.access_token)
        if (abort.signal.aborted) return
        const acc = await upsert({ id: randomUUID(), type: 'msa', active: true, ...mc, mcToken: seal(mc.mcToken), refreshToken: seal(tok.refresh_token ?? '') })
        emit('login', { state: 'success', account: view(acc) } satisfies LoginEvent)
        return
      }
      throw new Error('The sign-in code expired. Please try again.')
    } catch (e) {
      if (!abort.signal.aborted) emit('login', { state: 'error', error: errorMessage(e) } satisfies LoginEvent)
    }
  })()

  return { userCode: dc.user_code, verificationUri: dc.verification_uri, expiresIn: dc.expires_in }
}

export function cancelMicrosoftLogin(): void {
  loginAbort?.abort()
}

async function refresh(acc: StoredAccount): Promise<StoredAccount> {
  const tok = await msForm<{ access_token?: string; refresh_token?: string; error_description?: string }>('token', {
    grant_type: 'refresh_token',
    client_id: await clientId(),
    refresh_token: unseal(acc.refreshToken),
    scope: SCOPE
  })
  if (!tok.access_token) throw new Error(`Your Microsoft session expired. Remove and re-add ${acc.username}. (${tok.error_description ?? 'no token'})`)
  const mc = await minecraftLogin(tok.access_token)
  const updated: StoredAccount = {
    ...acc,
    ...mc,
    mcToken: seal(mc.mcToken),
    refreshToken: tok.refresh_token ? seal(tok.refresh_token) : acc.refreshToken
  }
  await save((await load()).map((a) => (a.id === acc.id ? updated : a)))
  return updated
}

// ---------- Offline ----------

/** Same scheme Java's UUID.nameUUIDFromBytes uses, so servers in offline mode agree with us. */
function offlineUuid(name: string): string {
  const b = createHash('md5').update(`OfflinePlayer:${name}`).digest()
  b[6] = (b[6] & 0x0f) | 0x30
  b[8] = (b[8] & 0x3f) | 0x80
  const h = b.toString('hex')
  return `${h.slice(0, 8)}-${h.slice(8, 12)}-${h.slice(12, 16)}-${h.slice(16, 20)}-${h.slice(20)}`
}

export async function addOfflineAccount(username: string): Promise<AccountView> {
  const name = username.trim()
  if (!/^[A-Za-z0-9_]{3,16}$/.test(name)) throw new Error('Usernames are 3–16 characters: letters, numbers and _')
  // Like Prism, offline play is only for people who own the game. Dev builds skip this so the
  // launcher can be tested before Mojang approves the Azure app.
  if (app.isPackaged && !(await load()).some((a) => a.type === 'msa'))
    throw new Error('Add a Microsoft account that owns Minecraft before creating offline profiles.')
  return view(await upsert({ id: randomUUID(), type: 'offline', username: name, uuid: offlineUuid(name), active: true }))
}

// ---------- Launch ----------

export async function getLaunchAuth(): Promise<LaunchAuth> {
  const list = await load()
  let acc = list.find((a) => a.active) ?? list[0]
  if (!acc) throw new Error('Sign in with your Microsoft account before playing.')
  if (acc.type === 'offline') return { username: acc.username, uuid: acc.uuid, accessToken: '0', userType: 'legacy' }
  if (!acc.mcTokenExpires || acc.mcTokenExpires - Date.now() < 5 * 60_000) acc = await refresh(acc)
  return { username: acc.username, uuid: acc.uuid, accessToken: unseal(acc.mcToken), userType: 'msa', xuid: acc.xuid }
}
