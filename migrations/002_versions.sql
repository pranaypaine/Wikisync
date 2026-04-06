-- Page version history
CREATE TABLE IF NOT EXISTS page_versions (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    page_id     TEXT NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    saved_by    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title       TEXT NOT NULL,
    content     TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_page_versions_page ON page_versions(page_id, created_at)
