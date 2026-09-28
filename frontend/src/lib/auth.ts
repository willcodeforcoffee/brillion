import { API_BASE } from './config'

const CLIENT_ID_KEY = 'brillion_client_id'
const CLIENT_SECRET_KEY = 'brillion_client_secret'
const ACCESS_TOKEN_KEY = 'brillion_access_token'
const REFRESH_TOKEN_KEY = 'brillion_refresh_token'
const PKCE_VERIFIER_KEY = 'brillion_pkce_verifier'
const PKCE_STATE_KEY = 'brillion_pkce_state'

const REDIRECT_URI = `${window.location.origin}/oauth/callback`
const SCOPES = 'read write follow'

function base64UrlEncode(bytes: Uint8Array): string {
  let binary = ''
  for (const byte of bytes) binary += String.fromCharCode(byte)
  return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '')
}

function randomString(byteLength: number): string {
  const bytes = new Uint8Array(byteLength)
  crypto.getRandomValues(bytes)
  return base64UrlEncode(bytes)
}

async function sha256(input: string): Promise<Uint8Array> {
  const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(input))
  return new Uint8Array(digest)
}

export function getAccessToken(): string | null {
  return localStorage.getItem(ACCESS_TOKEN_KEY)
}

export function isLoggedIn(): boolean {
  return getAccessToken() !== null
}

export function logout() {
  localStorage.removeItem(ACCESS_TOKEN_KEY)
  localStorage.removeItem(REFRESH_TOKEN_KEY)
}

/**
 * Registers this SPA as an OAuth app on first use (Mastodon-shaped
 * `POST /api/v1/apps` — SPEC.md §7) and caches the credentials.
 * `client_secret` living in localStorage is fine here: this is a
 * single-tenant, self-hosted instance, not a multi-tenant app store.
 */
async function getOrRegisterApp(): Promise<{ clientId: string; clientSecret: string }> {
  const cachedId = localStorage.getItem(CLIENT_ID_KEY)
  const cachedSecret = localStorage.getItem(CLIENT_SECRET_KEY)
  if (cachedId && cachedSecret) {
    return { clientId: cachedId, clientSecret: cachedSecret }
  }

  const response = await fetch(`${API_BASE}/api/v1/apps`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      name: 'Brillion Web',
      redirect_uris: [REDIRECT_URI],
      scopes: SCOPES,
    }),
  })
  if (!response.ok) {
    throw new Error(`failed to register OAuth app: ${response.status}`)
  }
  const app = (await response.json()) as { client_id: string; client_secret: string }
  localStorage.setItem(CLIENT_ID_KEY, app.client_id)
  localStorage.setItem(CLIENT_SECRET_KEY, app.client_secret)
  return { clientId: app.client_id, clientSecret: app.client_secret }
}

/** Kicks off the PKCE flow with a full-page redirect to `/oauth/authorize`. */
export async function startLogin() {
  const { clientId } = await getOrRegisterApp()

  const verifier = randomString(32)
  const state = randomString(16)
  const challenge = base64UrlEncode(await sha256(verifier))

  sessionStorage.setItem(PKCE_VERIFIER_KEY, verifier)
  sessionStorage.setItem(PKCE_STATE_KEY, state)

  const url = new URL('/oauth/authorize', API_BASE)
  url.searchParams.set('client_id', clientId)
  url.searchParams.set('redirect_uri', REDIRECT_URI)
  url.searchParams.set('code_challenge', challenge)
  url.searchParams.set('code_challenge_method', 'S256')
  url.searchParams.set('state', state)
  url.searchParams.set('scope', SCOPES)

  window.location.href = url.toString()
}

/** Exchanges the `?code=`/`?state=` redirect back from `/oauth/authorize`. */
export async function completeLogin(code: string, state: string): Promise<void> {
  const expectedState = sessionStorage.getItem(PKCE_STATE_KEY)
  const verifier = sessionStorage.getItem(PKCE_VERIFIER_KEY)
  sessionStorage.removeItem(PKCE_STATE_KEY)
  sessionStorage.removeItem(PKCE_VERIFIER_KEY)

  if (!expectedState || !verifier || state !== expectedState) {
    throw new Error('OAuth state mismatch — possible CSRF or a stale/duplicate callback')
  }

  const { clientId, clientSecret } = await getOrRegisterApp()

  const body = new URLSearchParams({
    grant_type: 'authorization_code',
    code,
    code_verifier: verifier,
    redirect_uri: REDIRECT_URI,
    client_id: clientId,
    client_secret: clientSecret,
  })

  const response = await fetch(`${API_BASE}/oauth/token`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body,
  })
  if (!response.ok) {
    throw new Error(`token exchange failed: ${response.status}`)
  }
  const token = (await response.json()) as { access_token: string; refresh_token: string }
  localStorage.setItem(ACCESS_TOKEN_KEY, token.access_token)
  localStorage.setItem(REFRESH_TOKEN_KEY, token.refresh_token)
}
