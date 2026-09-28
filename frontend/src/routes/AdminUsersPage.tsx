import { useQuery } from '@apollo/client/react'
import { USERS_QUERY } from '../lib/graphql'

interface UsersData {
  users: {
    username: string
    email: string
    role: string
    createdAt: string
  }[]
}

// Admin: read-only user list (creation stays CLI-only — SPEC.md §11).
function AdminUsersPage() {
  const { data, loading, error } = useQuery<UsersData>(USERS_QUERY)

  if (loading) return <main>Loading…</main>
  if (error) return <main role="alert">{error.message}</main>

  return (
    <main>
      <h1>Users</h1>
      <table>
        <thead>
          <tr>
            <th>Username</th>
            <th>Email</th>
            <th>Role</th>
            <th>Created</th>
          </tr>
        </thead>
        <tbody>
          {data?.users.map((user) => (
            <tr key={user.username}>
              <td>{user.username}</td>
              <td>{user.email}</td>
              <td>{user.role}</td>
              <td>{new Date(user.createdAt).toLocaleDateString()}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </main>
  )
}

export default AdminUsersPage
