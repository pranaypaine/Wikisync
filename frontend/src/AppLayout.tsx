import { useState, useEffect, useRef } from 'react'
import { useNavigate, useLocation, Outlet } from 'react-router-dom'
import api, { type Page, type SearchResult } from './api'
import { useAuth } from './store'
import PageTree from './components/PageTree'
import SharedWithMe from './components/SharedWithMe'
import CreatePageModal from './components/CreatePageModal'

function useDarkMode() {
  const [dark, setDark] = useState<boolean>(() => {
    const stored = localStorage.getItem('theme')
    if (stored) return stored === 'dark'
    return window.matchMedia('(prefers-color-scheme: dark)').matches
  })

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', dark ? 'dark' : 'light')
    localStorage.setItem('theme', dark ? 'dark' : 'light')
  }, [dark])

  // Respect system changes when no manual override
  useEffect(() => {
    const mq = window.matchMedia('(prefers-color-scheme: dark)')
    const handler = (e: MediaQueryListEvent) => {
      if (!localStorage.getItem('theme')) setDark(e.matches)
    }
    mq.addEventListener('change', handler)
    return () => mq.removeEventListener('change', handler)
  }, [])

  return [dark, setDark] as const
}

export default function AppLayout() {
  const { user, logout } = useAuth()
  const [pages, setPages] = useState<Page[]>([])
  const [search, setSearch] = useState('')
  const [results, setResults] = useState<SearchResult[]>([])
  const [showCreate, setShowCreate] = useState(false)
  const [parentId, setParentId] = useState<string | null>(null)
  const [dark, setDark] = useDarkMode()
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const searchContainerRef = useRef<HTMLDivElement>(null)
  const nav = useNavigate()
  const location = useLocation()

  const refreshPages = () => api.get('/pages').then(r => setPages(r.data))

  useEffect(() => { refreshPages() }, [location.pathname])

  useEffect(() => {
    if (debounceRef.current) clearTimeout(debounceRef.current)
    if (!search.trim()) { setResults([]); return }
    debounceRef.current = setTimeout(async () => {
      const res = await api.get(`/search?q=${encodeURIComponent(search)}`)
      setResults(res.data)
    }, 300)
  }, [search])

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (searchContainerRef.current && !searchContainerRef.current.contains(e.target as Node)) {
        setSearch('')
        setResults([])
      }
    }
    document.addEventListener('mousedown', handler)
    return () => document.removeEventListener('mousedown', handler)
  }, [])

  const openCreate = (pid: string | null = null) => {
    setParentId(pid)
    setShowCreate(true)
  }

  const onCreated = (page: Page) => {
    setShowCreate(false)
    refreshPages()
    nav(`/pages/${page.id}`)
  }

  return (
    <div className="app-layout">
      <aside className="sidebar">
        <div className="sidebar-top">
          <span className="sidebar-logo">Wiki</span>
        </div>

        <div className="sidebar-search" ref={searchContainerRef}>
          <input
            placeholder="Search..."
            value={search}
            onChange={e => setSearch(e.target.value)}
          />
          {results.length > 0 && (
            <div className="search-results-dropdown">
              {results.map(r => (
                <div
                  key={r.id}
                  className="search-result"
                  onClick={() => { setSearch(''); setResults([]); nav(`/pages/${r.id}`) }}
                >
                  <strong>{r.title}</strong>
                  <span dangerouslySetInnerHTML={{ __html: r.snippet }} />
                </div>
              ))}
            </div>
          )}
        </div>

        <button className="sidebar-new-page" onClick={() => openCreate(null)}>
          + New page
        </button>

        <div className="sidebar-pages">
          <div className="sidebar-section-label">Pages</div>
          <PageTree pages={pages} onOpenCreate={openCreate} />
          <SharedWithMe />
        </div>

        <div className="sidebar-footer">
          <span className="sidebar-user">{user?.username}</span>
          <button
            className="sidebar-theme-toggle"
            onClick={() => setDark(d => !d)}
            title={dark ? 'Switch to light mode' : 'Switch to dark mode'}
          >
            {dark ? '☀' : '☾'}
          </button>
          <button className="sidebar-logout" onClick={logout}>Log out</button>
        </div>
      </aside>

      <div className="main-content">
        <Outlet />
      </div>

      {showCreate && (
        <CreatePageModal
          parentId={parentId}
          onCreated={onCreated}
          onClose={() => setShowCreate(false)}
        />
      )}
    </div>
  )
}
