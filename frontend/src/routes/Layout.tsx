import { Link, Outlet } from 'react-router'

function Layout() {
  return (
    <>
      <header>
        <nav>
          <Link to="/">Brillion</Link>
          <Link to="/settings">Settings</Link>
          <Link to="/admin/users">Admin</Link>
          <Link to="/login">Log in</Link>
        </nav>
      </header>
      <Outlet />
    </>
  )
}

export default Layout
