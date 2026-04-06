import { useState } from 'react'
import api, {type Page} from '../api'

interface Props {
  parentId: string | null
  onCreated: (page: Page) => void
  onClose: () => void
}

export default function CreatePageModal({ parentId, onCreated, onClose }: Props) {
  const [title, setTitle] = useState('')
  const [loading, setLoading] = useState(false)

  const submit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!title.trim()) return
    setLoading(true)
    try {
      const res = await api.post('/pages', { title: title.trim(), parent_id: parentId })
      onCreated(res.data)
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal" onClick={e => e.stopPropagation()}>
        <h2>{parentId ? 'New sub-page' : 'New page'}</h2>
        <form onSubmit={submit}>
          <input
            autoFocus
            placeholder="Page title"
            value={title}
            onChange={e => setTitle(e.target.value)}
            required
          />
          <div className="modal-actions">
            <button type="button" className="btn-ghost" onClick={onClose}>Cancel</button>
            <button type="submit" disabled={loading}>{loading ? 'Creating…' : 'Create'}</button>
          </div>
        </form>
      </div>
    </div>
  )
}
