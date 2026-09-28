import { Link, Outlet, useNavigate } from 'react-router'
import { isLoggedIn, logout } from '../lib/auth'

function Layout() {
  const navigate = useNavigate()
  const loggedIn = isLoggedIn()

  function handleLogout() {
    logout()
    navigate('/home')
  }

  return (
    <>
      <header>
        <nav>
          <Link to="/home">Brillion</Link>
          {loggedIn && <Link to="/posts/new">New post</Link>}
          {loggedIn && <Link to="/settings">Settings</Link>}
          {loggedIn && <Link to="/admin/users">Admin</Link>}
          {loggedIn ? (
            <button type="button" onClick={handleLogout}>
              Log out
            </button>
          ) : (
            <Link to="/login">Log in</Link>
          )}
        </nav>
      </header>
      <Outlet />
    </>
  )
}

export default Layout
