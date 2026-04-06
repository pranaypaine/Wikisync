-- Users
CREATE TABLE IF NOT EXISTS users (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    username    TEXT NOT NULL UNIQUE,
    email       TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- Wiki pages
CREATE TABLE IF NOT EXISTS pages (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    owner_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    parent_id   TEXT REFERENCES pages(id) ON DELETE CASCADE,
    title       TEXT NOT NULL,
    slug        TEXT NOT NULL,
    content     TEXT NOT NULL DEFAULT '',
    content_html TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    UNIQUE(owner_id, slug)
);

-- Page collaborators
CREATE TABLE IF NOT EXISTS page_collaborators (
    page_id     TEXT NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    PRIMARY KEY (page_id, user_id)
);

-- Full-text search virtual table (FTS5)
CREATE VIRTUAL TABLE IF NOT EXISTS pages_fts USING fts5(
    title,
    content,
    content='pages',
    content_rowid='rowid'
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_pages_owner ON pages(owner_id);
CREATE INDEX IF NOT EXISTS idx_pages_parent ON pages(parent_id);
CREATE INDEX IF NOT EXISTS idx_pages_slug ON pages(owner_id, slug);
