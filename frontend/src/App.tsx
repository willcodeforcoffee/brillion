import { BrowserRouter, Route, Routes } from 'react-router'
import './App.css'
import AdminUsersPage from './routes/AdminUsersPage'
import ActorProfilePage from './routes/ActorProfilePage'
import HomePage from './routes/HomePage'
import Layout from './routes/Layout'
import LoginPage from './routes/LoginPage'
import NotFoundPage from './routes/NotFoundPage'
import OAuthCallbackPage from './routes/OAuthCallbackPage'
import PostEditorPage from './routes/PostEditorPage'
import PostPage from './routes/PostPage'
import SettingsPage from './routes/SettingsPage'

function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route element={<Layout />}>
          <Route index element={<HomePage />} />
          <Route path="login" element={<LoginPage />} />
          <Route path="oauth/callback" element={<OAuthCallbackPage />} />
          <Route path="settings" element={<SettingsPage />} />
          <Route path="admin/users" element={<AdminUsersPage />} />
          <Route path="posts/new" element={<PostEditorPage />} />
          <Route path="posts/:postId/edit" element={<PostEditorPage />} />
          <Route path="users/:username" element={<ActorProfilePage />} />
          <Route path="users/:username/:slug" element={<PostPage />} />
          <Route path="*" element={<NotFoundPage />} />
        </Route>
      </Routes>
    </BrowserRouter>
  )
}

export default App
