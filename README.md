# Wiki

A self-hosted collaborative wiki with real-time editing, live markdown preview, full-text search, and nested pages.

## Features

### Core
- **JWT authentication** — Register, login (by username or email), per-user page ownership
- **Nested pages** — Unlimited depth hierarchy with unique per-user URL slugs
- **Full-text search** — SQLite FTS5 with highlighted snippets, searches owned and shared pages
- **Page sharing** — Share any page with other users by username; collaborators get read/write access
- **Owner-only controls** — Share, delete, sub-page creation, and title editing are restricted to the page owner
- **Version history** — Every save creates a snapshot; browse and restore any previous version from a slide-in panel
- **Single binary** — React SPA embedded in the Rust binary via `rust-embed`; zero external dependencies at runtime

### Real-time Collaboration
- **Live two-pane editor** — Write raw markdown on the left, see the rendered output update on the right as you type
- **Real-time content sync** — WebSocket broadcasts every keystroke to all users on the same page; changes appear without refresh
- **Cursor presence** — Each collaborator's cursor position is tracked and rendered as a coloured caret with a name label directly in the editor
- **Active user avatars** — Coloured initials appear in the topbar for every user currently viewing the page; avatars update instantly on join/leave
- **Idle-safe sync** — Remote content updates pause while you are actively typing (< 1.5 s idle) to prevent cursor disruption

### Markdown
- **Full GitHub-flavored markdown** — Headings (`#`), bold, italic, lists, code blocks, tables, blockquotes, horizontal rules, images, links
- **Single-newline line breaks** — Pressing Enter once creates a visible line break in the preview (`remark-breaks`)
- **Syntax highlighting** — Fenced code blocks render with a dark theme

### Sidebar
- **Page tree** — Collapsible nested tree of all pages you own; add sub-pages inline
- **Shared with me** — Separate sidebar section showing pages others have shared with you, grouped by owner with collapsible trees per group

## Quick Start

### One command (builds + runs everything)

```bash
# Optional: create a .env file with your settings
cp .env.example .env   # or create manually — see Configuration below

./run.sh
```

`run.sh` will:

1. Check if all the requiremnts are met, else will ask to install them.
2. Load environment variables from `.env` (if present)
3. Build the React frontend (`npm ci && npm run build`)
4. Build the Rust backend (`cargo build --release`)
5. Start the server

Open http://localhost:3000

### Manual build

```bash
# 1. Build frontend
cd frontend && npm install && npm run build && cd ..

# 2. Build Rust binary (requires Rust 1.70+)
cargo build --release

# 3. Run
JWT_SECRET=your-secret-here ./target/release/wiki-server
```

### Development

```bash
# Terminal 1 — Rust backend with hot-reload
JWT_SECRET=dev-secret DATABASE_URL=sqlite:./wiki.db cargo watch -x run

# Terminal 2 — Vite dev server with HMR
cd frontend && npm run dev
```

Frontend dev server runs on http://localhost:5173 and proxies `/api` and `/ws` to the Rust backend on port 3000.

## Deployment

### Linux VPS (systemd)

```bash
# 1. Build on your local machine (or on the server)
./run.sh   # or build.sh to build only, without starting

# 2. Copy the binary and database directory to your server
scp target/release/wiki-server user@your-server:/opt/wiki/
ssh user@your-server "mkdir -p /opt/wiki/data"

# 3. Create /etc/systemd/system/wiki.service
[Unit]
Description=Wiki Server
After=network.target

[Service]
ExecStart=/opt/wiki/wiki-server
WorkingDirectory=/opt/wiki
EnvironmentFile=/opt/wiki/.env
Restart=on-failure
User=wiki

[Install]
WantedBy=multi-user.target

# 4. Enable and start
sudo systemctl daemon-reload
sudo systemctl enable --now wiki
```

Create `/opt/wiki/.env`:
```
DATABASE_URL=sqlite:./data/wiki.db
JWT_SECRET=<generate with: openssl rand -hex 32>
PORT=3000
```

> **SQLite data lives in `WorkingDirectory`** — back up the `.db` file regularly.

---

### Docker

```dockerfile
# Dockerfile
FROM rust:1.77 AS build
WORKDIR /app
COPY . .
RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash - \
 && apt-get install -y nodejs \
 && cd frontend && npm ci && npm run build && cd .. \
 && cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /app/target/release/wiki-server .
VOLUME ["/app/data"]
ENV DATABASE_URL=sqlite:./data/wiki.db
EXPOSE 3000
CMD ["./wiki-server"]
```

```bash
docker build -t wiki .

docker run -d \
  --name wiki \
  -p 3000:3000 \
  -v wiki_data:/app/data \
  -e JWT_SECRET=$(openssl rand -hex 32) \
  wiki
```

---

### Fly.io

```bash
# Install flyctl and sign in
curl -L https://fly.io/install.sh | sh
fly auth login

# Create the app
fly launch --name my-wiki --no-deploy

# Attach a persistent volume for SQLite
fly volumes create wiki_data --size 1   # GB

# Set secrets
fly secrets set JWT_SECRET=$(openssl rand -hex 32)
fly secrets set DATABASE_URL=sqlite:./data/wiki.db
```

Add to `fly.toml`:
```toml
[mounts]
  source = "wiki_data"
  destination = "/app/data"

[[services]]
  internal_port = 3000
  protocol = "tcp"

  [[services.ports]]
    port = 443
    handlers = ["tls", "http"]
  [[services.ports]]
    port = 80
    handlers = ["http"]
```

```bash
fly deploy
```

---

### Reverse proxy (nginx)

Place this behind nginx to add TLS:

```nginx
server {
    listen 443 ssl;
    server_name wiki.example.com;

    ssl_certificate     /etc/letsencrypt/live/wiki.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/wiki.example.com/privkey.pem;

    location / {
        proxy_pass http://127.0.0.1:3000;
        proxy_http_version 1.1;
        # Required for WebSocket upgrade
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

Obtain a certificate with `certbot --nginx -d wiki.example.com`.

## Stress Testing

A load-test script is included at `scripts/stress_test.sh`. It auto-creates a test user, runs HTTP benchmarks against every major endpoint, and then hammers the WebSocket layer with concurrent clients — no external account or API key needed.

### Dependencies

| Tool | Required | Install |
|---|---|---|
| `wrk` | Yes | `sudo apt install wrk` / `brew install wrk` |
| `jq` | Yes | `sudo apt install jq` / `brew install jq` |
| `node` | Yes (WebSocket test) | already needed to build the frontend |
| `wrk2` | No — only for `--rps` cap | `brew install wrk2` |

### Running

```bash
# Make sure the server is running first
./run.sh &   # or in a separate terminal

# Default: 500 connections, 30 s per endpoint, 200 WebSocket clients
./scripts/stress_test.sh

# Quick smoke test
./scripts/stress_test.sh --duration 10s --conns 100

# High concurrency
./scripts/stress_test.sh --conns 2000 --threads 16 --duration 60s

# Cap request rate at 50 000 req/s (requires wrk2)
./scripts/stress_test.sh --rps 50000 --duration 60s

# Skip WebSocket benchmark
./scripts/stress_test.sh --skip-ws

# Test a remote server
./scripts/stress_test.sh --url https://wiki.example.com
```

### Options

| Flag | Default | Description |
|---|---|---|
| `--url` | `http://localhost:3000` | Base URL of the server under test |
| `--duration` | `30s` | How long to hammer each HTTP endpoint |
| `--conns` | `500` | Concurrent HTTP connections |
| `--threads` | `nproc` | wrk worker threads |
| `--rps` | unlimited | Target req/s (wrk2 only) |
| `--ws-clients` | `200` | Concurrent WebSocket clients |
| `--skip-ws` | — | Skip the WebSocket benchmark |

### What it measures

| Benchmark | Endpoint | Measures |
|---|---|---|
| 1 | `GET /api/auth/me` | JWT validation overhead per request |
| 2 | `GET /api/pages` | DB read + page-tree build |
| 3 | `GET /api/pages/:id` | Single row fetch |
| 4 | `GET /api/search?q=stress` | SQLite FTS5 query latency |
| 5 | `GET /api/pages/shared-with-me` | JOIN query |
| 6 | WebSocket (`/ws/pages/:id`) | Concurrent connections + broadcast throughput |

Each HTTP benchmark reports: **avg, stdev, p50, p90, p95, p99, p99.9, max** latency, HTTP errors, and timeouts.

The WebSocket benchmark reports: total messages sent, elapsed time, **msgs/s throughput**, and connection lifecycle latency at avg/p50/p90/p95/p99/max.

### Results

Reports are saved to `stress-results/report_<timestamp>.txt` after every run, so you can diff runs across deploys:

```bash
ls -lh stress-results/
```

All configuration via environment variables or a `.env` file in the project root:

| Variable | Default | Description |
|---|---|---|
| `DATABASE_URL` | `sqlite:./wiki.db` | SQLite file path |
| `JWT_SECRET` | `change-me-in-production-please` | JWT signing secret — **always set this in production** |
| `PORT` | `3000` | HTTP listen port |
| `RUST_LOG` | `wiki_server=info,tower_http=info` | Log verbosity |

Example `.env`:
```
DATABASE_URL=sqlite:./wiki.db
JWT_SECRET=super-secret-value
PORT=3000
```

## API Reference

### Auth
| Method | Path | Auth | Description |
|---|---|---|---|
| POST | `/api/auth/register` | ✓ | Register `{username, email, password}` |
| POST | `/api/auth/login` | ✓ | Login `{identifier, password}` (identifier = username or email) |
| GET | `/api/auth/me` | ✓ | Current user info |
| GET | `/api/auth/check-username/:u` | ✓ | `{"available": bool}` |

### Pages
| Method | Path | Auth | Description |
|---|---|---|---|
| GET | `/api/pages` | ✓ | List owned root pages as a tree |
| POST | `/api/pages` | ✓ | Create page `{title, content?, parent_id?}` |
| GET | `/api/pages/shared-with-me` | ✓ | Pages shared with the current user, grouped by owner |
| GET | `/api/pages/:id` | ✓ | Get page with children and collaborators |
| PATCH | `/api/pages/:id` | ✓ | Update `{title?, content?}` (owner or collaborator) |
| DELETE | `/api/pages/:id` | ✓ | Delete page and all sub-pages (owner only) |
| POST | `/api/pages/:id/share` | ✓ | Share `{username}` (owner only) |
| GET | `/api/pages/:id/active-users` | ✓ | `{"count": N}` users currently on the page |
| GET | `/api/pages/:id/versions` | ✓ | List up to 50 version snapshots |
| GET | `/api/pages/:id/versions/:vid` | ✓ | Get a specific version |
| POST | `/api/pages/:id/versions/:vid/restore` | ✓ | Restore page to a previous version |

### Search
| Method | Path | Auth | Description |
|---|---|---|---|
| GET | `/api/search?q=...` | ✓ | FTS5 full-text search across owned and shared pages |

### WebSocket
```
ws://host/ws/pages/:id?token=<jwt>
```

**Incoming message types (server → client):**

| `type` | Fields | Description |
|---|---|---|
| `presence` | `active_users[]` | Sent on user join/leave; full list of `{user_id, username}` |
| `edit` | `user_id`, `username`, `content`, `cursor_pos`, `active_users[]` | Content change from another user |
| `cursor` | `user_id`, `username`, `cursor_pos`, `active_users[]` | Cursor-only move (no content change) |

**Outgoing message (client → server):**
```json
{ "content": "...", "cursor_pos": 42 }   // edit
{ "cursor_pos": 42 }                      // cursor-only move
```

## Architecture

```
wiki-server (single binary)
├── Axum 0.7             — HTTP + WebSocket router
├── SQLite (sqlx 0.7)    — WAL mode, FTS5, foreign keys
├── argon2id             — Password hashing
├── jsonwebtoken 9       — HS256 JWT
├── pulldown-cmark       — Markdown → HTML (server-side, for version snapshots)
├── ammonia              — HTML sanitization (XSS prevention)
├── rust-embed 8         — Embeds frontend/dist/ at compile time
└── React + TypeScript (Vite)
    ├── zustand                        — Auth state
    ├── @uiw/react-markdown-preview    — Live rendered markdown preview
    ├── remark-breaks                  — Single-newline line breaks
    └── react-router-dom v7
```

The SQLite database is created automatically on first run with WAL mode, `cache_size=-64000`, and `busy_timeout=5000` for performance and concurrency.

---

## Using PostgreSQL Instead of SQLite

The default database is SQLite (single file, zero setup). Switching to PostgreSQL requires the following code changes.

### 1. Update `Cargo.toml`

Replace the `sqlx` dependency:

```toml
# Before
sqlx = { version = "0.7", features = ["runtime-tokio-rustls", "sqlite", "migrate", "chrono", "uuid"] }

# After
sqlx = { version = "0.7", features = ["runtime-tokio-rustls", "postgres", "migrate", "chrono", "uuid"] }
```

### 2. Rewrite `src/db/mod.rs`

```rust
use sqlx::{postgres::PgConnectOptions, PgPool};
use anyhow::Result;
use std::str::FromStr;

pub async fn create_pool(database_url: &str) -> Result<PgPool> {
    let opts = PgConnectOptions::from_str(database_url)?;
    let pool = PgPool::connect_with(opts).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}
```

Remove all `PRAGMA` lines — they are SQLite-only. PostgreSQL connection pooling and durability are configured at the server level.

### 3. Update all route files — query placeholders

SQLite uses `?` for bind parameters; PostgreSQL uses `$1`, `$2`, … Replace every occurrence:

```rust
// Before (SQLite)
sqlx::query_as("SELECT * FROM users WHERE id = ?").bind(id)

// After (PostgreSQL)
sqlx::query_as("SELECT * FROM users WHERE id = $1").bind(id)
```

Also update the `SqlitePool` / `SqliteRow` type references throughout `src/routes/` and `src/models/` to `PgPool` / `PgRow`.

### 4. Rewrite migrations

The current migrations use SQLite-specific syntax. Create PostgreSQL equivalents in `migrations/`:

**`migrations/001_init.sql`** — key differences:

| SQLite | PostgreSQL equivalent |
|---|---|
| `lower(hex(randomblob(16)))` | `gen_random_uuid()` (built-in since Postgres 13) |
| `TEXT PRIMARY KEY DEFAULT (lower(hex(...)))` | `UUID PRIMARY KEY DEFAULT gen_random_uuid()` |
| `strftime('%Y-%m-%dT%H:%M:%SZ', 'now')` | `NOW()` |
| `CREATE VIRTUAL TABLE … USING fts5(…)` | `tsvector` column + GIN index (see below) |

Example PostgreSQL `001_init.sql`:

```sql
CREATE TABLE IF NOT EXISTS users (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username      TEXT NOT NULL UNIQUE,
    email         TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS pages (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    parent_id    UUID REFERENCES pages(id) ON DELETE CASCADE,
    title        TEXT NOT NULL,
    slug         TEXT NOT NULL,
    content      TEXT NOT NULL DEFAULT '',
    content_html TEXT NOT NULL DEFAULT '',
    search_vec   TSVECTOR GENERATED ALWAYS AS (
                     to_tsvector('english', title || ' ' || content)
                 ) STORED,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(owner_id, slug)
);

CREATE TABLE IF NOT EXISTS page_collaborators (
    page_id UUID NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    PRIMARY KEY (page_id, user_id)
);

CREATE INDEX IF NOT EXISTS idx_pages_owner  ON pages(owner_id);
CREATE INDEX IF NOT EXISTS idx_pages_parent ON pages(parent_id);
CREATE INDEX IF NOT EXISTS idx_pages_slug   ON pages(owner_id, slug);
CREATE INDEX IF NOT EXISTS idx_pages_fts    ON pages USING GIN(search_vec);
```

Update the search query in `src/routes/pages.rs` to use `search_vec @@ plainto_tsquery('english', $1)` instead of the FTS5 `MATCH` syntax.

### 5. Update `DATABASE_URL`

```
# .env
DATABASE_URL=postgres://user:password@localhost:5432/wiki
```

### 6. Run database migrations

With `sqlx::migrate!()` in place the migrations run automatically on startup. To run them manually:

```bash
sqlx migrate run --database-url postgres://user:password@localhost:5432/wiki
```
