mod db;
mod middleware;
mod models;
mod routes;
mod static_files;
mod ws;

use axum::{
    routing::{delete, get, patch, post},
    Router,
};
use sqlx::SqlitePool;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use ws::room::ActiveRooms;

/// Shared application state, cloneable via Arc internals.
#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub rooms: ActiveRooms,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "wiki_server=info,tower_http=info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:./wiki.db".to_string());

    let pool = db::create_pool(&database_url).await?;
    let rooms = ActiveRooms::new();

    let state = AppState { pool, rooms };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        // Auth
        .route("/api/auth/register", post(routes::auth::register))
        .route("/api/auth/login", post(routes::auth::login))
        .route("/api/auth/me", get(routes::auth::me))
        .route("/api/auth/check-username/:username", get(routes::auth::check_username))
        // Pages
        .route("/api/pages", get(routes::pages::list_pages))
        .route("/api/pages", post(routes::pages::create_page))
        .route("/api/pages/shared-with-me", get(routes::pages::shared_with_me))
        .route("/api/pages/:id", get(routes::pages::get_page))
        .route("/api/pages/:id", patch(routes::pages::update_page))
        .route("/api/pages/:id", delete(routes::pages::delete_page))
        .route("/api/pages/:id/share", post(routes::pages::share_page))
        .route("/api/pages/:id/active-users", get(routes::pages::active_users))
        .route("/api/pages/:id/versions", get(routes::pages::list_versions))
        .route("/api/pages/:id/versions/:vid", get(routes::pages::get_version))
        .route("/api/pages/:id/versions/:vid/restore", post(routes::pages::restore_version))
        // Search
        .route("/api/search", get(routes::pages::search_pages))
        // WebSocket
        .route("/ws/pages/:id", get(ws::handler::ws_handler))
        // Static files (SPA)
        .fallback(static_files::serve_static)
        .with_state(state)
        .layer(cors);

    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("0.0.0.0:{}", port);
    info!("Wiki server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

