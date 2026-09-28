import { useState } from 'react'
import { startLogin } from '../lib/auth'

// Kicks off the PKCE flow (SPEC.md §7, §11): a full-page redirect to the
// backend's own /oauth/authorize, which renders its own login form
// (email/password) and redirects back to /oauth/callback with a code.
function LoginPage() {
  const [error, setError] = useState<string | null>(null)

  async function handleLogin() {
    setError(null)
    try {
      await startLogin()
    } catch (err) {
      setError(err instanceof Error ? err.message : 'failed to start login')
    }
  }

  return (
    <main>
      <h1>Log in</h1>
      <p>You'll be redirected to this server's own sign-in page.</p>
      {error && <p role="alert">{error}</p>}
      <button type="button" onClick={handleLogin}>
        Log in
      </button>
    </main>
  )
}

export default LoginPage
