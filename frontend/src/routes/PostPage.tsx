import { useParams } from 'react-router'

// Public post view / permalink (SPEC.md §4.1, §11).
function PostPage() {
  const { username, slug } = useParams()

  return (
    <main>
      <h1>Post</h1>
      <p>
        @{username}/{slug} isn't wired up yet.
      </p>
    </main>
  )
}

export default PostPage
