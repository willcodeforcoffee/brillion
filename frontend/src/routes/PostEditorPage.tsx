import { useParams } from 'react-router'

// Post editor (Markdown, image upload via presigned RustFS URLs —
// SPEC.md §5.2, §11). Needs the GraphQL client's createPost/updatePost
// mutations and requestImageUpload/attachImage.
function PostEditorPage() {
  const { postId } = useParams()

  return (
    <main>
      <h1>{postId ? 'Edit post' : 'New post'}</h1>
      <p>Editor isn't wired up yet.</p>
    </main>
  )
}

export default PostEditorPage
