import { create } from 'zustand'
import api, {type User} from './api'

interface AuthState {
  user: User | null
  token: string | null
  login: (identifier: string, password: string) => Promise<void>
  register: (username: string, email: string, password: string) => Promise<void>
  logout: () => void
  loadMe: () => Promise<void>
}

export const useAuth = create<AuthState>((set) => ({
  user: null,
  token: localStorage.getItem('token'),

  login: async (identifier, password) => {
    const res = await api.post('/auth/login', { identifier, password })
    localStorage.setItem('token', res.data.token)
    set({ token: res.data.token, user: res.data.user })
  },

  register: async (username, email, password) => {
    const res = await api.post('/auth/register', { username, email, password })
    localStorage.setItem('token', res.data.token)
    set({ token: res.data.token, user: res.data.user })
  },

  logout: () => {
    localStorage.removeItem('token')
    set({ token: null, user: null })
  },

  loadMe: async () => {
    try {
      const res = await api.get('/auth/me')
      set({ user: res.data })
    } catch {
      localStorage.removeItem('token')
      set({ token: null, user: null })
    }
  },
}))
