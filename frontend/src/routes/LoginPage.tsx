// Login / OAuth consent: kicks off the PKCE flow against the backend's
// own /oauth/authorize + /oauth/token (SPEC.md §7, §11). Not wired up
// yet — the backend doesn't have an OAuth provider to redirect to.
function LoginPage() {
  return (
    <main>
      <h1>Log in</h1>
      <p>OAuth login isn't wired up yet.</p>
    </main>
  )
}

export default LoginPage
