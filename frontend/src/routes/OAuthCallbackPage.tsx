// Receives the ?code= redirect from /oauth/authorize and exchanges it
// for a token at /oauth/token (PKCE, SPEC.md §7). Not wired up yet.
function OAuthCallbackPage() {
  return (
    <main>
      <h1>Signing in…</h1>
      <p>OAuth callback isn't wired up yet.</p>
    </main>
  )
}

export default OAuthCallbackPage
