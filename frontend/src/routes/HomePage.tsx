import { useQuery } from '@apollo/client/react'
import { API_BASE } from '../lib/config'
import { POSTS_QUERY } from '../lib/graphql'

interface PostNode {
  id: string
  slug: string
  title: string
  summary: string | null
  publishedAt: string | null
  author: { preferredUsername: string; displayName: string }
}

interface PostsData {
  posts: {
    edges: { cursor: string; node: PostNode }[]
  }
}

// Authenticated home timeline (SPEC.md §11). No personalized "followed
// actors" timeline query exists yet (that's phase 4) — this shows the
// same site-wide published posts as the public homepage for now.
function HomePage() {
  // `cache-and-network`, not the default `cache-first`: publishing a
  // post elsewhere (the editor) doesn't touch this query's cached
  // list, so a plain remount would otherwise keep showing the stale
  // pre-publish result until something else invalidates it.
  const { data, loading, error } = useQuery<PostsData>(POSTS_QUERY, {
    variables: { first: 20 },
    fetchPolicy: 'cache-and-network',
  })

  if (loading) return <main>Loading…</main>
  if (error) return <main role="alert">Failed to load posts: {error.message}</main>

  const posts = data?.posts.edges ?? []

  return (
    <main>
      <h1>Timeline</h1>
      {posts.length === 0 && <p>No published posts yet.</p>}
      <ul>
        {posts.map(({ node }) => (
          <li key={node.id}>
            <a href={`${API_BASE}/users/${node.author.preferredUsername}/${node.slug}`}>
              {node.title}
            </a>
            <div>by {node.author.displayName || node.author.preferredUsername}</div>
            {node.summary && <p>{node.summary}</p>}
          </li>
        ))}
      </ul>
    </main>
  )
}

export default HomePage
