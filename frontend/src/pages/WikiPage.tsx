import { useEffect, useLayoutEffect, useRef, useState, useCallback } from 'react'
import { useParams, useNavigate } from 'react-router-dom'
import MarkdownPreview from '@uiw/react-markdown-preview'
import remarkBreaks from 'remark-breaks'
import api, { type Page } from '../api'
import { useAuth } from '../store'
import CreatePageModal from '../components/CreatePageModal'
import VersionHistory from '../components/VersionHistory'

// ─── Cursor colours ───────────────────────────────────────────────────────────

const CURSOR_COLORS = [
  '#e03131', '#2f9e44', '#1971c2', '#e67700',
  '#9c36b5', '#0c8599', '#c2255c', '#5c7a00',
]

function userColor(userId: string): string {
  let h = 0
  for (let i = 0; i < userId.length; i++) h = (h * 31 + userId.charCodeAt(i)) | 0
  return CURSOR_COLORS[Math.abs(h) % CURSOR_COLORS.length]
}

// ─── Types ────────────────────────────────────────────────────────────────────

interface Peer {
  username: string
  color: string
  cursor_pos: number | null
}

// ─── Caret coordinate helper (mirror-div technique) ───────────────────────────

function getCaretCoords(
  el: HTMLTextAreaElement,
  pos: number,
): { top: number; left: number; height: number } {
  const cs = window.getComputedStyle(el)
  const mirror = document.createElement('div')
  Object.assign(mirror.style, {
    position: 'absolute',
    visibility: 'hidden',
    left: '-9999px',
    top: '0',
    whiteSpace: 'pre-wrap',
    wordBreak: 'break-word',
    overflowWrap: 'break-word',
    width: el.offsetWidth + 'px',
    paddingTop: cs.paddingTop,
    paddingRight: cs.paddingRight,
    paddingBottom: cs.paddingBottom,
    paddingLeft: cs.paddingLeft,
    fontSize: cs.fontSize,
    fontFamily: cs.fontFamily,
    fontWeight: cs.fontWeight,
    lineHeight: cs.lineHeight,
    letterSpacing: cs.letterSpacing,
    wordSpacing: cs.wordSpacing,
    borderTop: `${cs.borderTopWidth} solid transparent`,
    borderRight: `${cs.borderRightWidth} solid transparent`,
    borderBottom: `${cs.borderBottomWidth} solid transparent`,
    borderLeft: `${cs.borderLeftWidth} solid transparent`,
    boxSizing: cs.boxSizing,
  })
  const safePos = Math.min(pos, el.value.length)
  mirror.appendChild(document.createTextNode(el.value.substring(0, safePos)))
  const marker = document.createElement('span')
  marker.textContent = '\u200b'
  mirror.appendChild(marker)
  document.body.appendChild(mirror)
  const top = marker.offsetTop
  const left = marker.offsetLeft
  const height = parseFloat(cs.lineHeight) || 24
  document.body.removeChild(mirror)
  return { top, left, height }
}

// ─── Remote cursor ────────────────────────────────────────────────────────────

function RemoteCursor({
  peer,
  textarea,
  content,
}: {
  peer: Peer & { userId: string }
  textarea: HTMLTextAreaElement | null
  content: string
}) {
  const [c, setC] = useState<{ top: number; left: number; height: number } | null>(null)

  useLayoutEffect(() => {
    if (!textarea || peer.cursor_pos == null) { setC(null); return }
    setC(getCaretCoords(textarea, peer.cursor_pos))
  }, [peer.cursor_pos, textarea, content])

  if (!c) return null
  return (
    <div className="remote-cursor" style={{ top: c.top, left: c.left }}>
      <div className="remote-cursor-caret" style={{ height: c.height, background: peer.color }} />
      <div className="remote-cursor-label" style={{ background: peer.color }}>{peer.username}</div>
    </div>
  )
}

// ─── Page component ───────────────────────────────────────────────────────────

export default function WikiPage() {
  const { id } = useParams<{ id: string }>()
  const { user, token } = useAuth()
  const nav = useNavigate()

  const [page, setPage] = useState<Page | null>(null)
  const [content, setContent] = useState('')
  const [saving, setSaving] = useState(false)
  const [peers, setPeers] = useState<Map<string, Peer>>(new Map())

  const [editingTitle, setEditingTitle] = useState(false)
  const [titleVal, setTitleVal] = useState('')
  const [showShare, setShowShare] = useState(false)
  const [shareUsername, setShareUsername] = useState('')
  const [showCreate, setShowCreate] = useState(false)
  const [showHistory, setShowHistory] = useState(false)

  const lastLocalEditRef = useRef<number>(0)
  const textareaRef = useRef<HTMLTextAreaElement>(null)
  const wsRef = useRef<WebSocket | null>(null)
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const cursorTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  // ── Auto-resize textarea ──────────────────────────────────────────────────
  const adjustHeight = useCallback(() => {
    const ta = textareaRef.current
    if (!ta) return
    ta.style.height = 'auto'
    ta.style.height = ta.scrollHeight + 'px'
  }, [])

  useEffect(() => { adjustHeight() }, [content, adjustHeight])

  // ── Load page ─────────────────────────────────────────────────────────────
  const loadPage = useCallback(() =>
    api.get(`/pages/${id}`).then(r => {
      setPage(r.data)
      setContent(r.data.content)
    }), [id])

  useEffect(() => { if (id) loadPage() }, [id, loadPage])

  // ── WebSocket ─────────────────────────────────────────────────────────────
  useEffect(() => {
    if (!id || !token) return

    const protocol = location.protocol === 'https:' ? 'wss' : 'ws'
    const wsUrl = `${protocol}://${location.host}/ws/pages/${id}?token=${encodeURIComponent(token)}`
    const ws = new WebSocket(wsUrl)
    wsRef.current = ws

    ws.onmessage = (e) => {
      try {
        const msg = JSON.parse(e.data) as {
          type?: string
          user_id?: string
          username?: string
          content?: string
          cursor_pos?: number
          active_users?: { user_id: string; username: string }[]
        }

        // Sync presence from every message
        if (Array.isArray(msg.active_users)) {
          setPeers(prev => {
            const next = new Map<string, Peer>()
            for (const u of msg.active_users!) {
              if (u.user_id === user?.id) continue
              next.set(u.user_id, {
                username: u.username,
                color: userColor(u.user_id),
                cursor_pos: prev.get(u.user_id)?.cursor_pos ?? null,
              })
            }
            return next
          })
        }

        if (msg.user_id === user?.id) return

        if (msg.type === 'edit' && typeof msg.content === 'string') {
          // Apply remote content only when the local user has been idle for >1.5 s
          if (Date.now() - lastLocalEditRef.current > 1500) {
            setContent(msg.content)
          }
        }

        if (msg.cursor_pos != null && msg.user_id) {
          setPeers(prev => {
            const existing = prev.get(msg.user_id!) ?? {
              username: msg.username ?? msg.user_id!,
              color: userColor(msg.user_id!),
              cursor_pos: null,
            }
            return new Map(prev).set(msg.user_id!, { ...existing, cursor_pos: msg.cursor_pos! })
          })
        }
      } catch { /* ignore malformed frames */ }
    }

    ws.onclose = () => { wsRef.current = null }
    return () => { ws.close() }
  }, [id, token, user?.id])

  // ── Broadcast cursor position (debounced) ─────────────────────────────────
  const broadcastCursor = useCallback((pos: number) => {
    if (cursorTimerRef.current) clearTimeout(cursorTimerRef.current)
    cursorTimerRef.current = setTimeout(() => {
      if (wsRef.current?.readyState === WebSocket.OPEN)
        wsRef.current.send(JSON.stringify({ cursor_pos: pos }))
    }, 50)
  }, [])

  // ── Content change ────────────────────────────────────────────────────────
  const handleChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const val = e.target.value
    setContent(val)
    adjustHeight()
    lastLocalEditRef.current = Date.now()
    const pos = e.target.selectionStart ?? 0

    if (wsRef.current?.readyState === WebSocket.OPEN)
      wsRef.current.send(JSON.stringify({ content: val, cursor_pos: pos }))

    if (saveTimerRef.current) clearTimeout(saveTimerRef.current)
    saveTimerRef.current = setTimeout(async () => {
      setSaving(true)
      try {
        const res = await api.patch(`/pages/${id}`, { content: val })
        setPage(res.data)
      } finally { setSaving(false) }
    }, 3000)
  }

  const handleCursorEvent = (
    e: React.KeyboardEvent<HTMLTextAreaElement> | React.MouseEvent<HTMLTextAreaElement> | React.FocusEvent<HTMLTextAreaElement>,
  ) => broadcastCursor((e.currentTarget as HTMLTextAreaElement).selectionStart ?? 0)

  // ── Title ─────────────────────────────────────────────────────────────────
  const saveTitle = async (newTitle: string) => {
    if (!id || !newTitle.trim()) return
    const res = await api.patch(`/pages/${id}`, { title: newTitle.trim() })
    setPage(res.data)
  }

  // ── Page actions ──────────────────────────────────────────────────────────
  const deletePage = async () => {
    if (!id || !window.confirm('Delete this page and all sub-pages?')) return
    await api.delete(`/pages/${id}`)
    nav('/')
  }

  const sharePage = async () => {
    if (!id || !shareUsername.trim()) return
    await api.post(`/pages/${id}/share`, { username: shareUsername.trim() })
    setShareUsername('')
    setShowShare(false)
    loadPage()
  }

  if (!page) return <div className="loading">Loading…</div>

  const peersArray = Array.from(peers.entries()).map(([userId, peer]) => ({ userId, ...peer }))
  const isOwner = page.owner_id === user?.id

  return (
    <div className="wiki-main">

      {/* ── Top bar ── */}
      <div className="wiki-topbar">
        <button className="btn" onClick={() => nav('/')}>← Back</button>

        <div className="presence-bar">
          {peersArray.map(p => (
            <div
              key={p.userId}
              className="presence-avatar"
              style={{ background: p.color }}
              title={p.username}
            >
              {p.username[0].toUpperCase()}
            </div>
          ))}
          {saving && <span className="saving-indicator">Saving…</span>}
        </div>

        <div className="wiki-topbar-actions">
          <button className="btn" onClick={() => setShowHistory(true)}>History</button>
          <button className="btn" onClick={() => setShowShare(s => !s)} disabled={!isOwner}>Share</button>
          <button className="btn" onClick={() => setShowCreate(true)} disabled={!isOwner}>+ Sub-page</button>
          <button className="btn btn-danger" onClick={deletePage} disabled={!isOwner}>Delete</button>
        </div>
      </div>

      {/* ── Writing area ── */}
      <div className="wiki-writer-scroll">
        <div className="wiki-writer-body">

          {/* Header: title, meta, share */}
          <div className="wiki-writer-header">
            {editingTitle ? (
              <input
                className="wiki-title-input"
                value={titleVal}
                autoFocus
                onChange={e => setTitleVal(e.target.value)}
                onBlur={() => { saveTitle(titleVal); setEditingTitle(false) }}
                onKeyDown={e => {
                  if (e.key === 'Enter') { saveTitle(titleVal); setEditingTitle(false) }
                  if (e.key === 'Escape') setEditingTitle(false)
                }}
              />
            ) : (
              <h1
                className="wiki-title-display"
                onClick={() => { if (isOwner) { setTitleVal(page.title); setEditingTitle(true) } }}
                style={{ cursor: isOwner ? 'text' : 'default' }}
              >
                {page.title}
              </h1>
            )}

            {page.collaborators.length > 0 && (
              <p className="wiki-collab-hint">
                Shared with {page.collaborators.map(c => c.username).join(', ')}
              </p>
            )}

            {showShare && (
              <div className="share-panel">
                <input
                  placeholder="Username to share with"
                  value={shareUsername}
                  autoFocus
                  onChange={e => setShareUsername(e.target.value)}
                  onKeyDown={e => { if (e.key === 'Enter') sharePage() }}
                />
                <button className="btn" onClick={sharePage}>Share</button>
                <button className="btn-ghost" onClick={() => setShowShare(false)}>Cancel</button>
              </div>
            )}
          </div>

          {/* Live two-pane: raw markdown editor | rendered preview */}
          <div className="wiki-live-layout">
            <div className="wiki-editor-pane">
              <div className="wiki-pane-label">Markdown</div>
              <div className="wiki-editor-wrapper">
                <textarea
                  ref={textareaRef}
                  className="wiki-textarea"
                  value={content}
                  onChange={handleChange}
                  onKeyUp={handleCursorEvent}
                  onMouseUp={handleCursorEvent}
                  onFocus={handleCursorEvent}
                  placeholder="Start writing…"
                  spellCheck
                />
                <div className="wiki-cursor-overlay" aria-hidden="true">
                  {peersArray.map(p => (
                    <RemoteCursor
                      key={p.userId}
                      peer={p}
                      textarea={textareaRef.current}
                      content={content}
                    />
                  ))}
                </div>
              </div>
            </div>

            <div className="wiki-preview-pane">
              <div className="wiki-pane-label">Preview</div>
              <div className="wiki-md-preview">
                {content
                  ? <MarkdownPreview source={content} remarkPlugins={[remarkBreaks]} />
                  : <p className="wiki-empty-hint">Start writing on the left…</p>}
              </div>
            </div>
          </div>

          {page.children.length > 0 && (
            <div className="wiki-writer-header">
              <div className="sub-pages">
                <div className="sub-pages-header">Sub-pages</div>
                <div className="sub-pages-grid">
                  {page.children.map(child => (
                    <div
                      key={child.id}
                      className="sub-page-card"
                      onClick={() => nav(`/pages/${child.id}`)}
                    >
                      {child.title}
                    </div>
                  ))}
                </div>
              </div>
            </div>
          )}

        </div>
      </div>

      {showHistory && (
        <VersionHistory
          pageId={page.id}
          currentContent={content}
          onRestore={() => loadPage()}
          onClose={() => setShowHistory(false)}
        />
      )}

      {showCreate && (
        <CreatePageModal
          parentId={page.id}
          onCreated={p => { setShowCreate(false); nav(`/pages/${p.id}`) }}
          onClose={() => setShowCreate(false)}
        />
      )}
    </div>
  )
}
