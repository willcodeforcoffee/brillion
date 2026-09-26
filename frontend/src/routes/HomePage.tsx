// Home timeline: local + followed federated actors' posts (SPEC.md §11).
// Needs the GraphQL client and a `timeline` query — not wired up yet.
function HomePage() {
  return (
    <main>
      <h1>Timeline</h1>
      <p>Local and followed posts will show up here once the GraphQL API lands.</p>
    </main>
  )
}

export default HomePage
