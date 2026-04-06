import { useEffect, useRef, useState } from 'react'

interface TocItem {
  level: number
  text: string
  id: string
}

interface Props {
  contentHtml: string
}

function slugId(text: string, idx: number) {
  return (
    text
      .toLowerCase()
      .replace(/[^\w\s-]/g, '')
      .replace(/\s+/g, '-')
      .slice(0, 60) + `-${idx}`
  )
}

function parseToc(html: string): TocItem[] {
  const div = document.createElement('div')
  div.innerHTML = html
  const items: TocItem[] = []
  let idx = 0
  div.querySelectorAll('h1,h2,h3,h4').forEach(el => {
    const level = parseInt(el.tagName[1])
    const text = el.textContent ?? ''
    const id = slugId(text, idx++)
    items.push({ level, text, id })
  })
  return items
}

/** Inject IDs into heading elements in the DOM so scroll-to works */
export function injectHeadingIds(container: HTMLElement, html: string) {
  const temp = document.createElement('div')
  temp.innerHTML = html
  let idx = 0
  const headings = container.querySelectorAll('h1,h2,h3,h4')
  headings.forEach(el => {
    const text = el.textContent ?? ''
    el.id = slugId(text, idx++)
  })
}

export default function TableOfContents({ contentHtml }: Props) {
  const [items, setItems] = useState<TocItem[]>([])
  const [activeId, setActiveId] = useState('')
  const observerRef = useRef<IntersectionObserver | null>(null)

  useEffect(() => {
    setItems(parseToc(contentHtml))
  }, [contentHtml])

  // Observe headings for active highlighting
  useEffect(() => {
    if (observerRef.current) observerRef.current.disconnect()
    const targets = document.querySelectorAll(
      '.wiki-content-view h1, .wiki-content-view h2, .wiki-content-view h3, .wiki-content-view h4'
    )
    if (!targets.length) return

    observerRef.current = new IntersectionObserver(
      entries => {
        const visible = entries.filter(e => e.isIntersecting)
        if (visible.length) setActiveId(visible[0].target.id)
      },
      { rootMargin: '0px 0px -60% 0px', threshold: 0 }
    )
    targets.forEach(t => observerRef.current!.observe(t))
    return () => observerRef.current?.disconnect()
  }, [items])

  if (items.length < 2) return null

  const scrollTo = (id: string) => {
    const el = document.getElementById(id)
    el?.scrollIntoView({ behavior: 'smooth', block: 'start' })
  }

  return (
    <aside className="wiki-toc">
      <div className="wiki-toc-label">On this page</div>
      <ul className="wiki-toc-list">
        {items.map(item => (
          <li
            key={item.id}
            className={`wiki-toc-item level-${item.level}${activeId === item.id ? ' active' : ''}`}
            style={{ paddingLeft: `${(item.level - 1) * 12}px` }}
            onClick={() => scrollTo(item.id)}
          >
            {item.text}
          </li>
        ))}
      </ul>
    </aside>
  )
}
