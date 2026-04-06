use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use slug::slugify;
use sqlx::SqlitePool;

use crate::{
    models::{
        ActiveUsersResponse, Claims, CreatePageRequest, Page, PageResponse, PageVersion,
        SearchQuery, SearchResult, ShareRequest, SharedByOwner, UpdatePageRequest, UserPublic,
    },
    routes::markdown::render_markdown,
    ws::room::ActiveRooms,
    AppState,
};

// ─── helpers ────────────────────────────────────────────────────────────────

async fn build_page_response(
    pool: &SqlitePool,
    page: Page,
    active_rooms: &ActiveRooms,
) -> PageResponse {
    let children = fetch_children(pool, &page.id, active_rooms).await;
    let collaborators = fetch_collaborators(pool, &page.id).await;
    let active_users = active_rooms.user_count(&page.id).await;

    PageResponse {
        id: page.id,
        owner_id: page.owner_id,
        parent_id: page.parent_id,
        title: page.title,
        slug: page.slug,
        content: page.content,
        content_html: page.content_html,
        created_at: page.created_at,
        updated_at: page.updated_at,
        children,
        collaborators,
        active_users,
    }
}

fn to_response_shallow(page: Page, active_users: usize) -> PageResponse {
    PageResponse {
        id: page.id,
        owner_id: page.owner_id,
        parent_id: page.parent_id,
        title: page.title,
        slug: page.slug,
        content: page.content,
        content_html: page.content_html,
        created_at: page.created_at,
        updated_at: page.updated_at,
        children: vec![],
        collaborators: vec![],
        active_users,
    }
}

async fn fetch_children(
    pool: &SqlitePool,
    parent_id: &str,
    active_rooms: &ActiveRooms,
) -> Vec<PageResponse> {
    let pages = sqlx::query_as::<_, Page>(
        "SELECT * FROM pages WHERE parent_id = ? ORDER BY created_at ASC",
    )
    .bind(parent_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut result = vec![];
    for p in pages {
        let active_users = active_rooms.user_count(&p.id).await;
        result.push(to_response_shallow(p, active_users));
    }
    result
}

async fn fetch_collaborators(pool: &SqlitePool, page_id: &str) -> Vec<UserPublic> {
    sqlx::query_as::<_, (String, String, String)>(
        "SELECT u.id, u.username, u.email FROM users u
         JOIN page_collaborators pc ON u.id = pc.user_id
         WHERE pc.page_id = ?",
    )
    .bind(page_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|(id, username, email)| UserPublic { id, username, email })
    .collect()
}

/// Check if a user is owner or collaborator of a page
async fn has_access(pool: &SqlitePool, page_id: &str, user_id: &str) -> bool {
    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pages WHERE id = ? AND (
             owner_id = ?
             OR EXISTS (SELECT 1 FROM page_collaborators WHERE page_id = ? AND user_id = ?)
         )",
    )
    .bind(page_id)
    .bind(user_id)
    .bind(page_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    n > 0
}

/// Generate a unique slug for a user, appending counter if needed
async fn unique_slug(pool: &SqlitePool, owner_id: &str, title: &str) -> String {
    let base = slugify(title);
    let mut candidate = base.clone();
    let mut i = 1u32;
    loop {
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pages WHERE owner_id = ? AND slug = ?",
        )
        .bind(owner_id)
        .bind(&candidate)
        .fetch_one(pool)
        .await
        .unwrap_or(0);

        if exists == 0 {
            return candidate;
        }
        candidate = format!("{}-{}", base, i);
        i += 1;
    }
}

// ─── Route handlers ──────────────────────────────────────────────────────────

/// GET /api/pages  — list root pages for the authenticated user
pub async fn list_pages(
    claims: Claims,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let pages = sqlx::query_as::<_, Page>(
        "SELECT * FROM pages WHERE owner_id = ? AND parent_id IS NULL ORDER BY created_at ASC",
    )
    .bind(&claims.sub)
    .fetch_all(&state.pool)
    .await;

    match pages {
        Ok(pages) => {
            let mut result = vec![];
            for p in pages {
                result.push(build_page_response(&state.pool, p, &state.rooms).await);
            }
            Json(result).into_response()
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response(),
    }
}

/// POST /api/pages
pub async fn create_page(
    claims: Claims,
    State(state): State<AppState>,
    Json(req): Json<CreatePageRequest>,
) -> impl IntoResponse {
    let title = req.title.trim().to_string();
    if title.is_empty() {
        return (StatusCode::BAD_REQUEST, "Title is required").into_response();
    }

    // Validate parent_id belongs to this user (if given)
    if let Some(ref pid) = req.parent_id {
        let ok: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pages WHERE id = ? AND owner_id = ?",
        )
        .bind(pid)
        .bind(&claims.sub)
        .fetch_one(&state.pool)
        .await
        .unwrap_or(0);
        if ok == 0 {
            return (StatusCode::NOT_FOUND, "Parent page not found").into_response();
        }
    }

    let content = req.content.unwrap_or_default();
    let content_html = render_markdown(&content);
    let slug = unique_slug(&state.pool, &claims.sub, &title).await;
    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    let result = sqlx::query(
        "INSERT INTO pages (id, owner_id, parent_id, title, slug, content, content_html, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&claims.sub)
    .bind(&req.parent_id)
    .bind(&title)
    .bind(&slug)
    .bind(&content)
    .bind(&content_html)
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await;

    match result {
        Ok(r) => {
            // Sync FTS index
            let rowid = r.last_insert_rowid();
            let _ = sqlx::query(
                "INSERT INTO pages_fts(rowid, title, content) VALUES (?, ?, ?)",
            )
            .bind(rowid)
            .bind(&title)
            .bind(&content)
            .execute(&state.pool)
            .await;

            let page = Page {
                id,
                owner_id: claims.sub,
                parent_id: req.parent_id,
                title,
                slug,
                content,
                content_html,
                created_at: now.clone(),
                updated_at: now,
            };
            Json(to_response_shallow(page, 0)).into_response()
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response(),
    }
}

/// GET /api/pages/:id
pub async fn get_page(
    claims: Claims,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if !has_access(&state.pool, &id, &claims.sub).await {
        return (StatusCode::NOT_FOUND, "Page not found").into_response();
    }

    let page = sqlx::query_as::<_, Page>("SELECT * FROM pages WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.pool)
        .await;

    match page {
        Ok(Some(p)) => Json(build_page_response(&state.pool, p, &state.rooms).await).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, "Page not found").into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response(),
    }
}

/// PATCH /api/pages/:id
pub async fn update_page(
    claims: Claims,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdatePageRequest>,
) -> impl IntoResponse {
    if !has_access(&state.pool, &id, &claims.sub).await {
        return (StatusCode::NOT_FOUND, "Page not found").into_response();
    }

    let page = sqlx::query_as::<_, Page>("SELECT * FROM pages WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.pool)
        .await;

    let mut page = match page {
        Ok(Some(p)) => p,
        _ => return (StatusCode::NOT_FOUND, "Page not found").into_response(),
    };

    if let Some(title) = req.title {
        let title = title.trim().to_string();
        if !title.is_empty() {
            page.slug = unique_slug(&state.pool, &claims.sub, &title).await;
            page.title = title;
        }
    }
    if let Some(content) = req.content {
        page.content_html = render_markdown(&content);
        page.content = content;
    }
    page.updated_at = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    // Save a version snapshot before updating
    let version_id = uuid::Uuid::new_v4().to_string();
    let _ = sqlx::query(
        "INSERT INTO page_versions (id, page_id, saved_by, title, content) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&version_id)
    .bind(&page.id)
    .bind(&claims.sub)
    .bind(&page.title)
    .bind(&page.content)
    .execute(&state.pool)
    .await;

    let result = sqlx::query(
        "UPDATE pages SET title=?, slug=?, content=?, content_html=?, updated_at=? WHERE id=?",
    )
    .bind(&page.title)
    .bind(&page.slug)
    .bind(&page.content)
    .bind(&page.content_html)
    .bind(&page.updated_at)
    .bind(&page.id)
    .execute(&state.pool)
    .await;

    match result {
        Ok(_) => {
            // Sync FTS index: delete old entry then reinsert
            let _ = sqlx::query(
                "INSERT INTO pages_fts(pages_fts, rowid, title, content) \
                 SELECT 'delete', rowid, title, content FROM pages WHERE id = ?",
            )
            .bind(&page.id)
            .execute(&state.pool)
            .await;
            let _ = sqlx::query(
                "INSERT INTO pages_fts(rowid, title, content) SELECT rowid, title, content FROM pages WHERE id = ?",
            )
            .bind(&page.id)
            .execute(&state.pool)
            .await;

            let active_users = state.rooms.user_count(&page.id).await;
            Json(to_response_shallow(page, active_users)).into_response()
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response(),
    }
}

/// DELETE /api/pages/:id
pub async fn delete_page(
    claims: Claims,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Only owner can delete
    let owner: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pages WHERE id = ? AND owner_id = ?",
    )
    .bind(&id)
    .bind(&claims.sub)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    if owner == 0 {
        return (StatusCode::NOT_FOUND, "Page not found or not owner").into_response();
    }

    // Sync FTS index before deleting
    let _ = sqlx::query(
        "INSERT INTO pages_fts(pages_fts, rowid, title, content) \
         SELECT 'delete', rowid, title, content FROM pages WHERE id = ?",
    )
    .bind(&id)
    .execute(&state.pool)
    .await;

    let _ = sqlx::query("DELETE FROM pages WHERE id = ?")
        .bind(&id)
        .execute(&state.pool)
        .await;

    StatusCode::NO_CONTENT.into_response()
}

/// GET /api/search?q=...
pub async fn search_pages(
    claims: Claims,
    State(state): State<AppState>,
    Query(q): Query<SearchQuery>,
) -> impl IntoResponse {
    if q.q.trim().is_empty() {
        return Json(Vec::<SearchResult>::new()).into_response();
    }

    // FTS5 using MATCH; snippet() function for excerpt; restrict to owned/shared pages
    let results = sqlx::query_as::<_, SearchResult>(
        "SELECT p.id, p.title, p.slug,
                snippet(pages_fts, 1, '<mark>', '</mark>', '...', 20) AS snippet
         FROM pages_fts
         JOIN pages p ON p.rowid = pages_fts.rowid
         WHERE pages_fts MATCH ?
           AND (
               p.owner_id = ?
               OR EXISTS (SELECT 1 FROM page_collaborators pc WHERE pc.page_id = p.id AND pc.user_id = ?)
           )
         ORDER BY rank
         LIMIT 20",
    )
    .bind(format!("\"{}\"", q.q.replace('"', ""))) // sanitise query
    .bind(&claims.sub)
    .bind(&claims.sub)
    .fetch_all(&state.pool)
    .await;

    match results {
        Ok(r) => Json(r).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Search error").into_response(),
    }
}

/// POST /api/pages/:id/share  — share page with another user by username
pub async fn share_page(
    claims: Claims,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<ShareRequest>,
) -> impl IntoResponse {
    // Only owner can share
    let owner: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pages WHERE id = ? AND owner_id = ?",
    )
    .bind(&id)
    .bind(&claims.sub)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    if owner == 0 {
        return (StatusCode::NOT_FOUND, "Page not found or not owner").into_response();
    }

    let username = req.username.trim().to_lowercase();
    let target = sqlx::query_as::<_, crate::models::User>(
        "SELECT * FROM users WHERE username = ?",
    )
    .bind(&username)
    .fetch_optional(&state.pool)
    .await;

    let target = match target {
        Ok(Some(u)) => u,
        Ok(None) => return (StatusCode::NOT_FOUND, "User not found").into_response(),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response(),
    };

    let _ = sqlx::query(
        "INSERT OR IGNORE INTO page_collaborators (page_id, user_id) VALUES (?, ?)",
    )
    .bind(&id)
    .bind(&target.id)
    .execute(&state.pool)
    .await;

    StatusCode::OK.into_response()
}

/// GET /api/pages/:id/active-users
pub async fn active_users(
    claims: Claims,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if !has_access(&state.pool, &id, &claims.sub).await {
        return (StatusCode::NOT_FOUND, "Page not found").into_response();
    }

    let count = state.rooms.user_count(&id).await;
    Json(ActiveUsersResponse { count }).into_response()
}

/// GET /api/pages/:id/versions
pub async fn list_versions(
    claims: Claims,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if !has_access(&state.pool, &id, &claims.sub).await {
        return (StatusCode::NOT_FOUND, "Page not found").into_response();
    }

    let versions = sqlx::query_as::<_, PageVersion>(
        "SELECT id, page_id, saved_by, title, content, created_at \
         FROM page_versions WHERE page_id = ? ORDER BY created_at DESC LIMIT 50",
    )
    .bind(&id)
    .fetch_all(&state.pool)
    .await;

    match versions {
        Ok(v) => Json(v).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response(),
    }
}

/// GET /api/pages/:id/versions/:vid
pub async fn get_version(
    claims: Claims,
    State(state): State<AppState>,
    Path((id, vid)): Path<(String, String)>,
) -> impl IntoResponse {
    if !has_access(&state.pool, &id, &claims.sub).await {
        return (StatusCode::NOT_FOUND, "Page not found").into_response();
    }

    let version = sqlx::query_as::<_, PageVersion>(
        "SELECT id, page_id, saved_by, title, content, created_at \
         FROM page_versions WHERE id = ? AND page_id = ?",
    )
    .bind(&vid)
    .bind(&id)
    .fetch_optional(&state.pool)
    .await;

    match version {
        Ok(Some(v)) => Json(v).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, "Version not found").into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response(),
    }
}

/// POST /api/pages/:id/versions/:vid/restore
pub async fn restore_version(
    claims: Claims,
    State(state): State<AppState>,
    Path((id, vid)): Path<(String, String)>,
) -> impl IntoResponse {
    // Only owner or collaborator can restore
    if !has_access(&state.pool, &id, &claims.sub).await {
        return (StatusCode::NOT_FOUND, "Page not found").into_response();
    }

    let version = sqlx::query_as::<_, PageVersion>(
        "SELECT id, page_id, saved_by, title, content, created_at \
         FROM page_versions WHERE id = ? AND page_id = ?",
    )
    .bind(&vid)
    .bind(&id)
    .fetch_optional(&state.pool)
    .await;

    let version = match version {
        Ok(Some(v)) => v,
        Ok(None) => return (StatusCode::NOT_FOUND, "Version not found").into_response(),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response(),
    };

    let content_html = render_markdown(&version.content);
    let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    // Save current state as a new version before overwriting
    let snap_id = uuid::Uuid::new_v4().to_string();
    let _ = sqlx::query(
        "INSERT INTO page_versions (id, page_id, saved_by, title, content) \
         SELECT ?, id, owner_id, title, content FROM pages WHERE id = ?",
    )
    .bind(&snap_id)
    .bind(&id)
    .execute(&state.pool)
    .await;

    let result = sqlx::query(
        "UPDATE pages SET title=?, content=?, content_html=?, updated_at=? WHERE id=?",
    )
    .bind(&version.title)
    .bind(&version.content)
    .bind(&content_html)
    .bind(&now)
    .bind(&id)
    .execute(&state.pool)
    .await;

    match result {
        Ok(_) => {
            // Sync FTS
            let _ = sqlx::query(
                "INSERT INTO pages_fts(pages_fts, rowid, title, content) \
                 SELECT 'delete', rowid, title, content FROM pages WHERE id = ?",
            )
            .bind(&id)
            .execute(&state.pool)
            .await;
            let _ = sqlx::query(
                "INSERT INTO pages_fts(rowid, title, content) \
                 SELECT rowid, title, content FROM pages WHERE id = ?",
            )
            .bind(&id)
            .execute(&state.pool)
            .await;

            let page = sqlx::query_as::<_, Page>("SELECT * FROM pages WHERE id = ?")
                .bind(&id)
                .fetch_one(&state.pool)
                .await;

            match page {
                Ok(p) => Json(build_page_response(&state.pool, p, &state.rooms).await).into_response(),
                Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response(),
            }
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response(),
    }
}

/// GET /api/pages/shared-with-me — list pages shared with the authenticated user, grouped by owner
pub async fn shared_with_me(
    claims: Claims,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // All pages where current user is a collaborator but not the owner
    let shared_pages = sqlx::query_as::<_, Page>(
        "SELECT p.* FROM pages p
         JOIN page_collaborators pc ON p.id = pc.page_id
         WHERE pc.user_id = ? AND p.owner_id != ?
         ORDER BY p.created_at ASC",
    )
    .bind(&claims.sub)
    .bind(&claims.sub)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    if shared_pages.is_empty() {
        return Json(Vec::<SharedByOwner>::new()).into_response();
    }

    // IDs of all pages in the shared set (used to identify tree roots and valid children)
    let shared_ids: std::collections::HashSet<String> =
        shared_pages.iter().map(|p| p.id.clone()).collect();

    // Unique owner IDs, preserving first-seen order
    let mut owner_ids_ordered: Vec<String> = vec![];
    {
        let mut seen = std::collections::HashSet::new();
        for p in &shared_pages {
            if seen.insert(p.owner_id.clone()) {
                owner_ids_ordered.push(p.owner_id.clone());
            }
        }
    }

    // Fetch owner user info
    let mut owner_map: std::collections::HashMap<String, UserPublic> =
        std::collections::HashMap::new();
    for oid in &owner_ids_ordered {
        if let Ok(Some(u)) =
            sqlx::query_as::<_, crate::models::User>("SELECT * FROM users WHERE id = ?")
                .bind(oid)
                .fetch_optional(&state.pool)
                .await
        {
            owner_map.insert(oid.clone(), u.into());
        }
    }

    // Recursively build a PageResponse tree from the shared pages set
    fn build_shared_tree(
        page: &Page,
        all: &[Page],
        shared_ids: &std::collections::HashSet<String>,
    ) -> PageResponse {
        let children: Vec<PageResponse> = all
            .iter()
            .filter(|p| {
                p.parent_id.as_deref() == Some(page.id.as_str()) && shared_ids.contains(&p.id)
            })
            .map(|p| build_shared_tree(p, all, shared_ids))
            .collect();

        PageResponse {
            id: page.id.clone(),
            owner_id: page.owner_id.clone(),
            parent_id: page.parent_id.clone(),
            title: page.title.clone(),
            slug: page.slug.clone(),
            content: String::new(),
            content_html: String::new(),
            created_at: page.created_at.clone(),
            updated_at: page.updated_at.clone(),
            children,
            collaborators: vec![],
            active_users: 0,
        }
    }

    // Root pages are those whose parent is not also in the shared set
    let result: Vec<SharedByOwner> = owner_ids_ordered
        .iter()
        .filter_map(|oid| {
            let owner = owner_map.get(oid)?.clone();
            let pages: Vec<PageResponse> = shared_pages
                .iter()
                .filter(|p| {
                    &p.owner_id == oid
                        && p.parent_id
                            .as_ref()
                            .map_or(true, |pid| !shared_ids.contains(pid))
                })
                .map(|p| build_shared_tree(p, &shared_pages, &shared_ids))
                .collect();
            if pages.is_empty() {
                None
            } else {
                Some(SharedByOwner { owner, pages })
            }
        })
        .collect();

    Json(result).into_response()
}

