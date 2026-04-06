import { useState } from 'react'
import { useNavigate, Link } from 'react-router-dom'
import { useAuth } from '../store'
import api from '../api'

export default function Register() {
  const [username, setUsername] = useState('')
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [usernameStatus, setUsernameStatus] = useState<'idle' | 'checking' | 'available' | 'taken'>('idle')
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(false)
  const { register } = useAuth()
  const nav = useNavigate()

  const checkUsername = async (val: string) => {
    if (val.length < 3) return
    setUsernameStatus('checking')
    try {
      const res = await api.get(`/auth/check-username/${val}`)
      setUsernameStatus(res.data.available ? 'available' : 'taken')
    } catch {
      setUsernameStatus('idle')
    }
  }

  const submit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (usernameStatus === 'taken') return
    setError('')
    setLoading(true)
    try {
      await register(username, email, password)
      nav('/')
    } catch (err: any) {
      setError(err.response?.data || 'Registration failed')
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="auth-container">
      <h1>Wiki</h1>
      <form onSubmit={submit} className="auth-form">
        <h2>Create account</h2>
        {error && <div className="error">{error}</div>}
        <label>
          Username
          <input
            value={username}
            onChange={e => {
              setUsername(e.target.value)
              checkUsername(e.target.value)
            }}
            required
            minLength={3}
            maxLength={32}
            autoFocus
          />
          {usernameStatus === 'available' && <span className="hint available">✓ Available</span>}
          {usernameStatus === 'taken' && <span className="hint taken">✗ Already taken</span>}
          {usernameStatus === 'checking' && <span className="hint">Checking…</span>}
        </label>
        <label>
          Email
          <input
            type="email"
            value={email}
            onChange={e => setEmail(e.target.value)}
            required
          />
        </label>
        <label>
          Password
          <input
            type="password"
            value={password}
            onChange={e => setPassword(e.target.value)}
            required
            minLength={8}
          />
        </label>
        <button type="submit" disabled={loading || usernameStatus === 'taken'}>
          {loading ? 'Creating account…' : 'Create account'}
        </button>
        <p>
          Have an account? <Link to="/login">Sign in</Link>
        </p>
      </form>
    </div>
  )
}
