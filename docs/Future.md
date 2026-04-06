# Planned Future Features

---

## AI Integration

### In-editor AI assistant
- **Inline suggestions** — press a shortcut to ask an AI to continue, summarise, or rewrite the selected paragraph
- **Slash commands** — `/ai summarise`, `/ai expand`, `/ai translate to Spanish` inside the editor
- **Page Q&A** — ask a question about the current page and get a cited answer (RAG over page content)

### Knowledge-base search
- **Semantic search** — embed page content via an embedding model (e.g. `nomic-embed-text` via Ollama or OpenAI); store vectors in SQLite with `sqlite-vec` extension; return semantically relevant results alongside FTS5 keyword results
- **Cross-page Q&A** — "What does our onboarding doc say about X?" answered from all pages the user has access to

### AI-powered page creation
- **Generate from prompt** — create a full structured wiki page from a one-line description
- **Auto-title and auto-slug** — suggest a title and URL slug from the content
- **Table-of-contents generation** — auto-insert a `[TOC]` based on headings

### Local-first AI (privacy)
- Support **Ollama** as the AI backend so all inference runs on the user's own machine with no data leaving the network
- Configurable provider: `AI_PROVIDER=ollama|openai|anthropic` in `.env`

---

## Cloud Deployment & Multi-tenancy

### Organisation accounts
- **Organisations** — a workspace shared by a team; each org has its own page tree, members, and roles
- **Roles** — `owner`, `admin`, `editor`, `viewer` per organisation
- **Invitations** — invite members by email; accept via a one-time link

### Managed cloud offering
- **Subdomain routing** — `acme.wiki.example.com` maps to the `acme` organisation
- **Per-org SQLite isolation** — each organisation writes to its own `.db` file (easy backup, easy delete)
- **Usage quotas** — page count, storage size, and active-user limits per plan tier

### Infrastructure improvements
- **S3-compatible image uploads** — attach images to pages; store in MinIO / AWS S3 / Cloudflare R2; serve via signed URLs
- **CDN-friendly static assets** — move the embedded frontend to a CDN edge on managed deployments
- **Read replicas** — Litestream continuous replication of SQLite to S3 for zero-RPO disaster recovery + optional read replica via `rqlite`

### SSO / Identity
- **OAuth 2.0 / OIDC** — login with Google, GitHub, or any corporate identity provider
- **SAML 2.0** — enterprise SSO (Okta, Azure AD, JumpCloud)
- **SCIM provisioning** — auto-create and deprovision users when employees join or leave the org

---

## Editor Improvements

### Rich content blocks
- **Notion-style slash menu** — `/image`, `/table`, `/code`, `/callout`, `/embed` block types
- **Drag-and-drop reordering** of blocks
- **Embeds** — YouTube, Figma, GitHub Gist, Google Maps iframes via a whitelist

### Collaborative editing
- **CRDT-based conflict resolution** — replace last-write-wins with `yjs` or `automerge` for true offline-first, conflict-free merging
- **Offline mode** — edit without an internet connection; sync automatically when back online
- **Presence cursors for mobile** — touch-friendly cursor display on iOS/Android

### Markdown extensions
- **Diagrams** — fenced code blocks with `mermaid` language tag render as flowcharts / sequence diagrams
- **Math** — `$...$` and `$$...$$` rendered via KaTeX
- **Footnotes** and **definition lists**
- **Custom callout blocks** — `> [!NOTE]`, `> [!WARNING]`, `> [!TIP]`

---

## Sharing & Permissions

- **Public pages** — make a page publicly readable via a shareable link (no login required)
- **Password-protected pages** — share with a link + passphrase
- **Expiring share links** — links that auto-expire after N days
- **Granular permissions** — per-page override of org-level role (e.g. make one page view-only for editors)
- **Comment threads** — inline comments on paragraphs; resolve/reopen; mention users with `@username`

---

## Notifications & Activity

- **In-app notification centre** — "Alice edited Onboarding Guide", "Bob shared a page with you"
- **Email digests** — daily or weekly summary of changes to pages you follow
- **Page watch** — subscribe to a page and get notified of every edit
- **Activity feed** — per-page audit log visible to collaborators

---

## Developer & Integration Features

- **REST webhooks** — POST to a URL on page create/update/delete; useful for Slack/Teams bots or CI triggers
- **Zapier / Make integration** — no-code automation triggers
- **API tokens** — long-lived personal access tokens for scripting and CI
- **CLI tool** — `wiki push page.md` / `wiki pull onboarding` for syncing pages from the filesystem
- **VS Code extension** — edit wiki pages in your editor, push on save

---

## Performance & Scalability

- **Connection pooling proxy** — `pgbouncer`-style pooling when running with PostgreSQL
- **Horizontal scaling** — stateless HTTP workers behind a load balancer; WebSocket room state moved to Redis Pub/Sub
- **Edge caching** — cache public pages at the CDN level with `Cache-Control` + `ETag` headers
- **Binary protocol** — evaluate replacing JSON WebSocket messages with MessagePack for lower bandwidth on large pages
