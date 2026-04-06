import {type Page} from '../api'
import { useNavigate } from 'react-router-dom'

interface Props {
  pages: Page[]
  onOpenCreate: (parentId: string | null) => void
  depth?: number
}

export default function PageTree({ pages, onOpenCreate, depth = 0 }: Props) {
  const nav = useNavigate()

  if (pages.length === 0 && depth === 0) {
    return (
      <div className="empty-state">
        No pages yet.<br />Click + New page to get started.
      </div>
    )
  }

  return (
    <ul className="page-tree" style={{ paddingLeft: depth * 12 }}>
      {pages.map(page => (
        <li key={page.id} className="page-tree-item">
          <div className="page-tree-row">
            <span className="tree-page-icon">▸</span>
            <span className="tree-page-title" onClick={() => nav(`/pages/${page.id}`)}>
              {page.title}
            </span>
            <button
              className="tree-add-btn"
              title="Add sub-page"
              onClick={e => { e.stopPropagation(); onOpenCreate(page.id) }}
            >
              +
            </button>
          </div>
          {page.children?.length > 0 && (
            <PageTree pages={page.children} onOpenCreate={onOpenCreate} depth={depth + 1} />
          )}
        </li>
      ))}
    </ul>
  )
}
