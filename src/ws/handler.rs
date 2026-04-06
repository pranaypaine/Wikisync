use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Utc;
use futures::{SinkExt, StreamExt};
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use serde::Deserialize;

use crate::{
    middleware::auth::jwt_secret,
    models::Claims,
    routes::markdown::render_markdown,
    AppState,
};

#[derive(Deserialize)]
pub struct WsQuery {
    pub token: String,
}

/// WebSocket upgrade handler: GET /ws/pages/:id?token=<jwt>
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(page_id): Path<String>,
    Query(q): Query<WsQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let secret = jwt_secret();
    let claims = decode::<Claims>(
        &q.token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    );

    let claims = match claims {
        Ok(c) => c.claims,
        Err(_) => return (StatusCode::UNAUTHORIZED, "Invalid token").into_response(),
    };

    let access: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pages WHERE id = ? AND (
             owner_id = ?
             OR EXISTS (SELECT 1 FROM page_collaborators WHERE page_id = ? AND user_id = ?)
         )",
    )
    .bind(&page_id)
    .bind(&claims.sub)
    .bind(&page_id)
    .bind(&claims.sub)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    if access == 0 {
        return (StatusCode::FORBIDDEN, "Access denied").into_response();
    }

    let pool = state.pool.clone();
    let active_rooms = state.rooms.clone();

    ws.on_upgrade(move |socket| handle_socket(socket, page_id, claims, pool, active_rooms))
        .into_response()
}

async fn handle_socket(
    socket: WebSocket,
    page_id: String,
    claims: Claims,
    pool: sqlx::SqlitePool,
    active_rooms: crate::ws::room::ActiveRooms,
) {
    let (mut sender, mut receiver) = socket.split();

    let mut rx = active_rooms
        .join(&page_id, claims.sub.clone(), claims.username.clone())
        .await;

    // Immediately broadcast presence so every client learns about the new user
    {
        let users = active_rooms.users(&page_id).await;
        let active_users: Vec<serde_json::Value> = users
            .iter()
            .map(|(id, name)| serde_json::json!({ "user_id": id, "username": name }))
            .collect();
        let presence = serde_json::json!({
            "type": "presence",
            "active_users": active_users,
        });
        active_rooms.broadcast(&page_id, &presence.to_string()).await;
    }

    // Forward broadcasts to this client
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    let page_id_clone = page_id.clone();
    let user_id = claims.sub.clone();
    let username = claims.username.clone();
    let active_rooms_clone = active_rooms.clone();
    let pool_clone = pool.clone();

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            let text = match msg {
                Message::Text(t) => t.to_string(),
                Message::Close(_) => break,
                _ => continue,
            };

            #[derive(serde::Deserialize)]
            struct IncomingMsg {
                content: Option<String>,
                cursor_pos: Option<u32>,
            }

            if let Ok(incoming) = serde_json::from_str::<IncomingMsg>(&text) {
                let users = active_rooms_clone.users(&page_id_clone).await;
                let active_users: Vec<serde_json::Value> = users
                    .iter()
                    .map(|(id, name)| serde_json::json!({ "user_id": id, "username": name }))
                    .collect();

                if let Some(content) = incoming.content {
                    // Content edit — persist and broadcast
                    let content_html = render_markdown(&content);
                    let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

                    let _ = sqlx::query(
                        "UPDATE pages SET content=?, content_html=?, updated_at=? WHERE id=?",
                    )
                    .bind(&content)
                    .bind(&content_html)
                    .bind(&now)
                    .bind(&page_id_clone)
                    .execute(&pool_clone)
                    .await;

                    let broadcast_msg = serde_json::json!({
                        "type": "edit",
                        "user_id": user_id,
                        "username": username,
                        "content": content,
                        "cursor_pos": incoming.cursor_pos,
                        "updated_at": now,
                        "active_users": active_users,
                    });

                    active_rooms_clone
                        .broadcast(&page_id_clone, &broadcast_msg.to_string())
                        .await;
                } else if incoming.cursor_pos.is_some() {
                    // Cursor-only move — broadcast without DB write
                    let broadcast_msg = serde_json::json!({
                        "type": "cursor",
                        "user_id": user_id,
                        "username": username,
                        "cursor_pos": incoming.cursor_pos,
                        "active_users": active_users,
                    });

                    active_rooms_clone
                        .broadcast(&page_id_clone, &broadcast_msg.to_string())
                        .await;
                }
            }
        }
    });

    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }

    active_rooms.leave(&page_id, &claims.sub).await;

    // Notify remaining users that someone left
    let users = active_rooms.users(&page_id).await;
    if !users.is_empty() {
        let active_users: Vec<serde_json::Value> = users
            .iter()
            .map(|(id, name)| serde_json::json!({ "user_id": id, "username": name }))
            .collect();
        let presence = serde_json::json!({
            "type": "presence",
            "active_users": active_users,
        });
        active_rooms.broadcast(&page_id, &presence.to_string()).await;
    }
}
