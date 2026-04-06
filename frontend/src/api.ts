import axios from 'axios'

const api = axios.create({
  baseURL: '/api',
})

api.interceptors.request.use((config) => {
  const token = localStorage.getItem('token')
  if (token) config.headers.Authorization = `Bearer ${token}`
  return config
})

export default api

// ─── Types ───────────────────────────────────────────────────────────────────

export interface User {
  id: string
  username: string
  email: string
}

export interface Page {
  id: string
  owner_id: string
  parent_id: string | null
  title: string
  slug: string
  content: string
  content_html: string
  created_at: string
  updated_at: string
  children: Page[]
  collaborators: User[]
  active_users: number
}

export interface SearchResult {
  id: string
  title: string
  slug: string
  snippet: string
}

export interface PageVersion {
  id: string
  page_id: string
  saved_by: string
  title: string
  content: string
  created_at: string
}

export interface SharedByOwner {
  owner: User
  pages: Page[]
}
