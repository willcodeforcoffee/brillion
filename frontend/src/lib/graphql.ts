import { gql } from '@apollo/client'

export const POST_FIELDS = gql`
  fragment PostFields on PostGql {
    id
    slug
    title
    summary
    bodyHtml
    status
    publishedAt
    author {
      id
      preferredUsername
      displayName
    }
  }
`

export const POSTS_QUERY = gql`
  query Posts($first: Int, $after: String) {
    posts(first: $first, after: $after) {
      edges {
        cursor
        node {
          ...PostFields
        }
      }
      pageInfo {
        hasNextPage
        endCursor
      }
    }
  }
  ${POST_FIELDS}
`

export const ME_QUERY = gql`
  query Me {
    me {
      id
      preferredUsername
      displayName
      domain
      avatarUrl
      headerUrl
    }
  }
`

export const POST_BY_ID_QUERY = gql`
  query PostById($id: ID!) {
    postById(id: $id) {
      id
      slug
      title
      summary
      bodyMarkdown
      status
    }
  }
`

export const CREATE_POST_MUTATION = gql`
  mutation CreatePost($input: CreatePostInput!) {
    createPost(input: $input) {
      id
      slug
      status
    }
  }
`

export const UPDATE_POST_MUTATION = gql`
  mutation UpdatePost($id: ID!, $input: UpdatePostInput!) {
    updatePost(id: $id, input: $input) {
      id
      slug
      status
    }
  }
`

export const PUBLISH_POST_MUTATION = gql`
  mutation PublishPost($id: ID!) {
    publishPost(id: $id) {
      id
      slug
      status
      publishedAt
    }
  }
`

export const USERS_QUERY = gql`
  query Users {
    users {
      username
      email
      role
      createdAt
    }
  }
`

export const REQUEST_IMAGE_UPLOAD_MUTATION = gql`
  mutation RequestImageUpload($contentType: String!) {
    requestImageUpload(contentType: $contentType) {
      uploadUrl
      mediaId
      publicUrl
    }
  }
`

export const UPDATE_ACTOR_IMAGE_MUTATION = gql`
  mutation UpdateActorImage($kind: ActorImageKind!, $mediaId: ID!) {
    updateActorImage(kind: $kind, mediaId: $mediaId) {
      id
      avatarUrl
      headerUrl
    }
  }
`
