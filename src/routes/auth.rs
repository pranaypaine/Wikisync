use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::Utc;

use crate::models::{
    AuthResponse, AvailabilityResponse, Claims, LoginRequest, RegisterRequest, UserPublic,
};
use crate::middleware::auth::sign_token;
use crate::AppState;

fn token_expiry() -> usize {
    let duration_secs: u64 = std::env::var("JWT_EXPIRY_SECS")
        .unwrap_or_default()
        .parse()
        .unwrap_or(86400 * 7);
    (Utc::now().timestamp() as u64 + duration_secs) as usize
}

pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> impl IntoResponse {
    let username = req.username.trim().to_lowercase();
    let email = req.email.trim().to_lowercase();

    if username.len() < 3 || username.len() > 32 {
        return (StatusCode::BAD_REQUEST, "Username must be 3-32 characters").into_response();
    }
    if !email.contains('@') {
        return (StatusCode::BAD_REQUEST, "Invalid email").into_response();
    }
    if req.password.len() < 8 {
        return (StatusCode::BAD_REQUEST, "Password must be at least 8 characters").into_response();
    }

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = match argon2.hash_password(req.password.as_bytes(), &salt) {
        Ok(h) => h.to_string(),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Hashing failed").into_response(),
    };

    let id = uuid::Uuid::new_v4().to_string();

    let result = sqlx::query(
        "INSERT INTO users (id, username, email, password_hash) VALUES (?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&username)
    .bind(&email)
    .bind(&password_hash)
    .execute(&state.pool)
    .await;

    match result {
        Err(sqlx::Error::Database(e)) if e.message().contains("UNIQUE") => {
            return (StatusCode::CONFLICT, "Username or email already taken").into_response();
        }
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response(),
        Ok(_) => {}
    }

    let claims = Claims {
        sub: id.clone(),
        username: username.clone(),
        exp: token_expiry(),
    };

    let token = match sign_token(&claims) {
        Ok(t) => t,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Token error").into_response(),
    };

    Json(AuthResponse {
        token,
        user: UserPublic { id, username, email },
    })
    .into_response()
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> impl IntoResponse {
    let identifier = req.identifier.trim().to_lowercase();

    let user = sqlx::query_as::<_, crate::models::User>(
        "SELECT * FROM users WHERE email = ? OR username = ? LIMIT 1",
    )
    .bind(&identifier)
    .bind(&identifier)
    .fetch_optional(&state.pool)
    .await;

    let user = match user {
        Ok(Some(u)) => u,
        Ok(None) => return (StatusCode::UNAUTHORIZED, "Invalid credentials").into_response(),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response(),
    };

    let parsed_hash = match PasswordHash::new(&user.password_hash) {
        Ok(h) => h,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Auth error").into_response(),
    };

    if Argon2::default()
        .verify_password(req.password.as_bytes(), &parsed_hash)
        .is_err()
    {
        return (StatusCode::UNAUTHORIZED, "Invalid credentials").into_response();
    }

    let claims = Claims {
        sub: user.id.clone(),
        username: user.username.clone(),
        exp: token_expiry(),
    };

    let token = match sign_token(&claims) {
        Ok(t) => t,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Token error").into_response(),
    };

    Json(AuthResponse {
        token,
        user: UserPublic {
            id: user.id,
            username: user.username,
            email: user.email,
        },
    })
    .into_response()
}

pub async fn check_username(
    State(state): State<AppState>,
    Path(username): Path<String>,
) -> impl IntoResponse {
    let username = username.trim().to_lowercase();
    let row = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM users WHERE username = ?",
    )
    .bind(&username)
    .fetch_one(&state.pool)
    .await;

    match row {
        Ok(count) => Json(AvailabilityResponse { available: count == 0 }).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response(),
    }
}

pub async fn me(claims: Claims, State(state): State<AppState>) -> impl IntoResponse {
    let user = sqlx::query_as::<_, crate::models::User>(
        "SELECT * FROM users WHERE id = ?",
    )
    .bind(&claims.sub)
    .fetch_optional(&state.pool)
    .await;

    match user {
        Ok(Some(u)) => Json(UserPublic::from(u)).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, "User not found").into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response(),
    }
}
