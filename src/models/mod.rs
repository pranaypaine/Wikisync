use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: String,
    pub username: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    /// Can be username or email
    pub identifier: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: UserPublic,
}

#[derive(Debug, Serialize, Clone)]
pub struct UserPublic {
    pub id: String,
    pub username: String,
    pub email: String,
}

impl From<User> for UserPublic {
    fn from(u: User) -> Self {
        UserPublic {
            id: u.id,
            username: u.username,
            email: u.email,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Page {
    pub id: String,
    pub owner_id: String,
    pub parent_id: Option<String>,
    pub title: String,
    pub slug: String,
    pub content: String,
    pub content_html: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreatePageRequest {
    pub title: String,
    pub content: Option<String>,
    pub parent_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePageRequest {
    pub title: Option<String>,
    pub content: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PageResponse {
    pub id: String,
    pub owner_id: String,
    pub parent_id: Option<String>,
    pub title: String,
    pub slug: String,
    pub content: String,
    pub content_html: String,
    pub created_at: String,
    pub updated_at: String,
    pub children: Vec<PageResponse>,
    pub collaborators: Vec<UserPublic>,
    pub active_users: usize,
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct SearchResult {
    pub id: String,
    pub title: String,
    pub slug: String,
    pub snippet: String,
}

#[derive(Debug, Deserialize)]
pub struct ShareRequest {
    pub username: String,
}

/// JWT claims
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String, // user id
    pub username: String,
    pub exp: usize,
}

/// Username availability check response
#[derive(Debug, Serialize)]
pub struct AvailabilityResponse {
    pub available: bool,
}

/// Active users on a page
#[derive(Debug, Serialize)]
pub struct ActiveUsersResponse {
    pub count: usize,
}

/// Pages shared with a user, grouped by the page owner
#[derive(Debug, Serialize)]
pub struct SharedByOwner {
    pub owner: UserPublic,
    pub pages: Vec<PageResponse>,
}

/// A snapshot of a page at a point in time
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PageVersion {
    pub id: String,
    pub page_id: String,
    pub saved_by: String,
    pub title: String,
    pub content: String,
    pub created_at: String,
}
