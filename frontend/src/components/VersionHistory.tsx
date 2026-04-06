import { useEffect, useState } from 'react'
import { diffLines } from 'diff'
import api, { type PageVersion } from '../api'

interface Props {
  pageId: string
  currentContent: string
  onRestore: () => void
  onClose: () => void
}

export default function VersionHistory({ pageId, currentContent, onRestore, onClose }: Props) {
  const [versions, setVersions] = useState<PageVersion[]>([])
  const [selected, setSelected] = useState<PageVersion | null>(null)
  const [showDiff, setShowDiff] = useState(false)
  const [restoring, setRestoring] = useState(false)

  useEffect(() => {
    api.get(`/pages/${pageId}/versions`).then(r => setVersions(r.data))
  }, [pageId])

  const handleRestore = async () => {
    if (!selected) return
    setRestoring(true)
    try {
      await api.post(`/pages/${pageId}/versions/${selected.id}/restore`)
      onRestore()
      onClose()
    } finally {
      setRestoring(false)
    }
  }

  const formatDate = (iso: string) => {
    const d = new Date(iso)
    return d.toLocaleString(undefined, {
      month: 'short', day: 'numeric',
      hour: '2-digit', minute: '2-digit',
    })
  }

  const diffContent = selected
    ? diffLines(selected.content, currentContent)
    : []

  return (
    <div className="vh-overlay" onClick={onClose}>
      <div className="vh-panel" onClick={e => e.stopPropagation()}>
        {/* Header */}
        <div className="vh-header">
          <span className="vh-title">Version history</span>
          <button className="vh-close btn" onClick={onClose}>✕</button>
        </div>

        <div className="vh-body">
          {/* Version list */}
          <div className="vh-list">
            {versions.length === 0 && (
              <div className="vh-empty">No saved versions yet.<br />Versions are created automatically on each save.</div>
            )}
            {versions.map((v, i) => (
              <div
                key={v.id}
                className={`vh-item${selected?.id === v.id ? ' active' : ''}`}
                onClick={() => { setSelected(v); setShowDiff(false) }}
              >
                <span className="vh-item-label">
                  {i === 0 ? 'Latest save' : `Version ${versions.length - i}`}
                </span>
                <span className="vh-item-date">{formatDate(v.created_at)}</span>
              </div>
            ))}
          </div>

          {/* Preview / diff panel */}
          {selected && (
            <div className="vh-preview">
              <div className="vh-preview-header">
                <span className="vh-preview-title">{selected.title}</span>
                <div className="vh-preview-actions">
                  <button
                    className={`btn${showDiff ? ' active' : ''}`}
                    onClick={() => setShowDiff(d => !d)}
                  >
                    {showDiff ? 'Preview' : 'Show diff'}
                  </button>
                  <button
                    className="btn vh-restore-btn"
                    onClick={handleRestore}
                    disabled={restoring}
                  >
                    {restoring ? 'Restoring…' : 'Restore'}
                  </button>
                </div>
              </div>

              {showDiff ? (
                <div className="vh-diff">
                  {diffContent.map((part, idx) => (
                    <div
                      key={idx}
                      className={`vh-diff-part${part.added ? ' added' : part.removed ? ' removed' : ''}`}
                    >
                      <span className="vh-diff-sign">
                        {part.added ? '+' : part.removed ? '−' : ' '}
                      </span>
                      <pre>{part.value}</pre>
                    </div>
                  ))}
                </div>
              ) : (
                <pre className="vh-content-preview">{selected.content || '(empty)'}</pre>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
