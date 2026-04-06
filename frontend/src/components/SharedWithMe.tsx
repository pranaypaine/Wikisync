import { useState, useEffect } from 'react'
import { useNavigate } from 'react-router-dom'
import api, { type SharedByOwner, type Page } from '../api'

interface PageNodeProps {
  page: Page
  depth?: number
}

function SharedPageNode({ page, depth = 0 }: PageNodeProps) {
  const [open, setOpen] = useState(false)
  const nav = useNavigate()
  const hasChildren = page.children && page.children.length > 0

  return (
    <li className="page-tree-item">
      <div
        className="page-tree-row"
        style={{ paddingLeft: `${depth * 12}px` }}
        onClick={() => nav(`/pages/${page.id}`)}
      >
        <span
          className="tree-page-icon"
          onClick={e => {
            if (hasChildren) { e.stopPropagation(); setOpen(o => !o) }
          }}
          style={{ cursor: hasChildren ? 'pointer' : 'default' }}
        >
          {hasChildren ? (open ? '▾' : '▸') : '·'}
        </span>
        <span className="tree-page-title">{page.title}</span>
      </div>
      {hasChildren && open && (
        <ul className="page-tree">
          {page.children.map(child => (
            <SharedPageNode key={child.id} page={child} depth={depth + 1} />
          ))}
        </ul>
      )}
    </li>
  )
}

export default function SharedWithMe() {
  const [groups, setGroups] = useState<SharedByOwner[]>([])
  const [collapsedOwners, setCollapsedOwners] = useState<Record<string, boolean>>({})

  useEffect(() => {
    api.get<SharedByOwner[]>('/pages/shared-with-me').then(r => setGroups(r.data))
  }, [])

  if (groups.length === 0) return null

  const toggleOwner = (ownerId: string) =>
    setCollapsedOwners(prev => ({ ...prev, [ownerId]: !prev[ownerId] }))

  return (
    <>
      <div className="sidebar-section-label">Shared with me</div>
      {groups.map(group => (
        <div key={group.owner.id}>
          <div
            className="shared-owner-row"
            onClick={() => toggleOwner(group.owner.id)}
          >
            <span className="tree-page-icon">
              {collapsedOwners[group.owner.id] ? '▸' : '▾'}
            </span>
            <span className="shared-owner-name">{group.owner.username}</span>
          </div>
          {!collapsedOwners[group.owner.id] && (
            <ul className="page-tree">
              {group.pages.map(page => (
                <SharedPageNode key={page.id} page={page} />
              ))}
            </ul>
          )}
        </div>
      ))}
    </>
  )
}
