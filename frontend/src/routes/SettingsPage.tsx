import { useMutation, useQuery } from '@apollo/client/react'
import { useState } from 'react'
import { API_BASE } from '../lib/config'
import { ME_QUERY, REQUEST_IMAGE_UPLOAD_MUTATION, UPDATE_ACTOR_IMAGE_MUTATION } from '../lib/graphql'

interface MeData {
  me: {
    id: string
    preferredUsername: string
    displayName: string
    domain: string
    avatarUrl: string | null
    headerUrl: string | null
  } | null
}

// The actual endpoint federated software/tools query to resolve this
// actor (SPEC.md §3.1) — `acct:user@domain`, not the actor's own AS2 id.
function webfingerUrl(preferredUsername: string, domain: string): string {
  const url = new URL('/.well-known/webfinger', API_BASE)
  url.searchParams.set('resource', `acct:${preferredUsername}@${domain}`)
  return url.toString()
}

type ImageKind = 'AVATAR' | 'HEADER'

// Avatar/header upload (SPEC.md §5.2, §11): request a presigned RustFS
// PUT URL, upload the file bytes directly to RustFS, then point the
// actor's avatar/header at the resulting media.
function SettingsPage() {
  const { data, loading, error, refetch } = useQuery<MeData>(ME_QUERY)
  const [requestImageUpload] = useMutation<{
    requestImageUpload: { uploadUrl: string; mediaId: string; publicUrl: string }
  }>(REQUEST_IMAGE_UPLOAD_MUTATION)
  const [updateActorImage] = useMutation(UPDATE_ACTOR_IMAGE_MUTATION)
  const [uploading, setUploading] = useState<ImageKind | null>(null)
  const [uploadError, setUploadError] = useState<string | null>(null)

  async function handleUpload(kind: ImageKind, file: File) {
    setUploadError(null)
    setUploading(kind)
    try {
      const { data: uploadTarget } = await requestImageUpload({
        variables: { contentType: file.type },
      })
      const target = uploadTarget?.requestImageUpload
      if (!target) throw new Error('no upload target returned')

      const putResponse = await fetch(target.uploadUrl, {
        method: 'PUT',
        headers: { 'Content-Type': file.type },
        body: file,
      })
      if (!putResponse.ok) {
        throw new Error(`upload to storage failed: ${putResponse.status}`)
      }

      await updateActorImage({ variables: { kind, mediaId: target.mediaId } })
      await refetch()
    } catch (err) {
      setUploadError(err instanceof Error ? err.message : 'upload failed')
    } finally {
      setUploading(null)
    }
  }

  if (loading) return <main>Loading…</main>
  if (error) return <main role="alert">Failed to load profile: {error.message}</main>
  if (!data?.me) return <main>Log in to manage your profile.</main>

  const { me } = data

  return (
    <main>
      <h1>Settings</h1>
      <p>{me.displayName || me.preferredUsername}</p>
      <p>
        <a href={webfingerUrl(me.preferredUsername, me.domain)}>Your WebFinger URL</a>
      </p>
      {uploadError && <p role="alert">{uploadError}</p>}

      <div>
        <h2>Avatar</h2>
        {me.avatarUrl && <img src={me.avatarUrl} alt="" width={96} height={96} />}
        <input
          type="file"
          accept="image/png,image/jpeg,image/webp,image/gif"
          disabled={uploading !== null}
          onChange={(event) => {
            const file = event.target.files?.[0]
            if (file) void handleUpload('AVATAR', file)
          }}
        />
      </div>

      <div>
        <h2>Header</h2>
        {me.headerUrl && <img src={me.headerUrl} alt="" width={320} height={96} />}
        <input
          type="file"
          accept="image/png,image/jpeg,image/webp,image/gif"
          disabled={uploading !== null}
          onChange={(event) => {
            const file = event.target.files?.[0]
            if (file) void handleUpload('HEADER', file)
          }}
        />
      </div>
    </main>
  )
}

export default SettingsPage
