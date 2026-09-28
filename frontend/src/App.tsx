import { BrowserRouter, Navigate, Route, Routes } from 'react-router'
import './App.css'
import AdminUsersPage from './routes/AdminUsersPage'
import HomePage from './routes/HomePage'
import Layout from './routes/Layout'
import LoginPage from './routes/LoginPage'
import NotFoundPage from './routes/NotFoundPage'
import OAuthCallbackPage from './routes/OAuthCallbackPage'
import PostEditorPage from './routes/PostEditorPage'
import SettingsPage from './routes/SettingsPage'

// This SPA is the authenticated app only (SPEC.md §11/§12 #2) — public
// pages (homepage, actor profile, post permalink) are server-rendered
// by the backend, so there are deliberately no routes here for
// `/users/:username` etc.
function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route element={<Layout />}>
          <Route index element={<Navigate to="/home" replace />} />
          <Route path="home" element={<HomePage />} />
          <Route path="login" element={<LoginPage />} />
          <Route path="oauth/callback" element={<OAuthCallbackPage />} />
          <Route path="settings" element={<SettingsPage />} />
          <Route path="admin/users" element={<AdminUsersPage />} />
          <Route path="posts/new" element={<PostEditorPage />} />
          <Route path="posts/:postId/edit" element={<PostEditorPage />} />
          <Route path="*" element={<NotFoundPage />} />
        </Route>
      </Routes>
    </BrowserRouter>
  )
}

export default App
