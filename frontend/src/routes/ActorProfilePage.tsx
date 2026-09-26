import { useParams } from 'react-router'

// Actor profile: a user's posts + follower/following counts (SPEC.md
// §11). The backend already serves a content-negotiated version of
// this URL directly (routes/actor.rs) — see the open decision in
// SPEC.md §12 on whether this page stays server-rendered or moves here.
function ActorProfilePage() {
  const { username } = useParams()

  return (
    <main>
      <h1>@{username}</h1>
      <p>Profile page isn't wired up yet.</p>
    </main>
  )
}

export default ActorProfilePage
