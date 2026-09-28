import { useEffect, useRef, useState } from 'react'
import { useNavigate, useSearchParams } from 'react-router'
import { completeLogin } from '../lib/auth'

function OAuthCallbackPage() {
  const [searchParams] = useSearchParams()
  const navigate = useNavigate()
  const [error, setError] = useState<string | null>(null)
  // StrictMode double-invokes effects in dev; the auth code is single-use
  // server-side, so a second exchange attempt would just fail loudly.
  const started = useRef(false)

  const code = searchParams.get('code')
  const state = searchParams.get('state')
  const paramsError =
    searchParams.get('error') ?? (!code || !state ? 'missing code or state in callback URL' : null)

  useEffect(() => {
    if (started.current || paramsError || !code || !state) return
    started.current = true

    completeLogin(code, state)
      .then(() => navigate('/home', { replace: true }))
      .catch((err: unknown) => {
        setError(err instanceof Error ? err.message : 'sign-in failed')
      })
  }, [code, state, paramsError, navigate])

  const displayError = paramsError ?? error

  return (
    <main>
      <h1>Signing in…</h1>
      {displayError && <p role="alert">{displayError}</p>}
    </main>
  )
}

export default OAuthCallbackPage
