import { useMutation, useQuery } from '@apollo/client/react'
import { useEffect, useState } from 'react'
import { useNavigate, useParams } from 'react-router'
import {
  CREATE_POST_MUTATION,
  POST_BY_ID_QUERY,
  PUBLISH_POST_MUTATION,
  UPDATE_POST_MUTATION,
} from '../lib/graphql'

interface PostByIdData {
  postById: {
    id: string
    slug: string
    title: string
    summary: string | null
    bodyMarkdown: string
    status: string
  } | null
}

// Post editor (SPEC.md §11): Markdown in, sanitized HTML out server-side
// (backend/src/markdown.rs). Image attachment (requestImageUpload +
// attachImage) is left for a follow-up — this covers create/edit/publish.
function PostEditorPage() {
  const { postId } = useParams()
  const navigate = useNavigate()
  const isEditing = Boolean(postId)

  const [title, setTitle] = useState('')
  const [summary, setSummary] = useState('')
  const [bodyMarkdown, setBodyMarkdown] = useState('')
  const [status, setStatus] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [savedId, setSavedId] = useState<string | null>(postId ?? null)

  const { data, loading: loadingPost } = useQuery<PostByIdData>(POST_BY_ID_QUERY, {
    variables: { id: postId },
    skip: !isEditing,
  })

  useEffect(() => {
    if (data?.postById) {
      setTitle(data.postById.title)
      setSummary(data.postById.summary ?? '')
      setBodyMarkdown(data.postById.bodyMarkdown)
      setStatus(data.postById.status)
    }
  }, [data])

  const [createPost, { loading: creating }] = useMutation<{
    createPost: { id: string; slug: string; status: string }
  }>(CREATE_POST_MUTATION)
  const [updatePost, { loading: updating }] = useMutation<{
    updatePost: { id: string; slug: string; status: string }
  }>(UPDATE_POST_MUTATION)
  const [publishPost, { loading: publishing }] = useMutation<{
    publishPost: { id: string; slug: string; status: string; publishedAt: string | null }
  }>(PUBLISH_POST_MUTATION)

  async function handleSave() {
    setError(null)
    const input = { title, summary: summary || null, bodyMarkdown }
    try {
      if (savedId) {
        const result = await updatePost({ variables: { id: savedId, input } })
        setStatus(result.data?.updatePost.status ?? status)
      } else {
        const result = await createPost({ variables: { input } })
        const created = result.data?.createPost
        if (created) {
          setSavedId(created.id)
          setStatus(created.status)
          navigate(`/posts/${created.id}/edit`, { replace: true })
        }
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'failed to save post')
    }
  }

  async function handlePublish() {
    if (!savedId) return
    setError(null)
    try {
      const result = await publishPost({ variables: { id: savedId } })
      setStatus(result.data?.publishPost.status ?? status)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'failed to publish post')
    }
  }

  if (isEditing && loadingPost) return <main>Loading…</main>

  const saving = creating || updating

  return (
    <main>
      <h1>{isEditing ? 'Edit post' : 'New post'}</h1>
      {status && <p>Status: {status}</p>}
      {error && <p role="alert">{error}</p>}
      <form
        onSubmit={(event) => {
          event.preventDefault()
          void handleSave()
        }}
      >
        <div>
          <label htmlFor="title">Title</label>
          <input
            id="title"
            value={title}
            onChange={(event) => setTitle(event.target.value)}
            required
          />
        </div>
        <div>
          <label htmlFor="summary">Summary</label>
          <input id="summary" value={summary} onChange={(event) => setSummary(event.target.value)} />
        </div>
        <div>
          <label htmlFor="body">Body (Markdown)</label>
          <textarea
            id="body"
            rows={16}
            value={bodyMarkdown}
            onChange={(event) => setBodyMarkdown(event.target.value)}
            required
          />
        </div>
        <button type="submit" disabled={saving}>
          {savedId ? 'Save' : 'Save draft'}
        </button>
        {savedId && status !== 'published' && (
          <button type="button" onClick={() => void handlePublish()} disabled={publishing}>
            Publish
          </button>
        )}
      </form>
    </main>
  )
}

export default PostEditorPage
