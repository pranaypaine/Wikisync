use std::{
    collections::HashMap,
    sync::Arc,
};
use tokio::sync::{broadcast, RwLock};

/// Message broadcast to all users in a page room.
/// The server acts as a relay: when user A sends an edit, all other users in the room receive it.
/// The client is responsible for applying operational transforms / CRDT merges.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EditMessage {
    pub user_id: String,
    pub username: String,
    /// Full content after edit (simple last-write-wins) or a JSON patch — client chooses format.
    pub content: String,
    pub cursor_pos: Option<u32>,
}

/// Per-page broadcast channel + live user set.
struct Room {
    tx: broadcast::Sender<String>,
    /// user_id -> username
    users: HashMap<String, String>,
}

impl Room {
    fn new() -> Self {
        let (tx, _) = broadcast::channel(256);
        Room {
            tx,
            users: HashMap::new(),
        }
    }
}

/// Shared state holding all active page rooms.
#[derive(Clone)]
pub struct ActiveRooms(Arc<RwLock<HashMap<String, Room>>>);

impl ActiveRooms {
    pub fn new() -> Self {
        ActiveRooms(Arc::new(RwLock::new(HashMap::new())))
    }

    /// Subscribe to a page room. Returns a receiver and the broadcast sender.
    pub async fn join(
        &self,
        page_id: &str,
        user_id: String,
        username: String,
    ) -> broadcast::Receiver<String> {
        let mut rooms = self.0.write().await;
        let room = rooms.entry(page_id.to_string()).or_insert_with(Room::new);
        room.users.insert(user_id, username);
        room.tx.subscribe()
    }

    /// Remove user from room. Clean up room if empty.
    pub async fn leave(&self, page_id: &str, user_id: &str) {
        let mut rooms = self.0.write().await;
        if let Some(room) = rooms.get_mut(page_id) {
            room.users.remove(user_id);
            if room.users.is_empty() {
                rooms.remove(page_id);
            }
        }
    }

    /// Broadcast a message to all users in a room.
    pub async fn broadcast(&self, page_id: &str, msg: &str) {
        let rooms = self.0.read().await;
        if let Some(room) = rooms.get(page_id) {
            let _ = room.tx.send(msg.to_string());
        }
    }

    /// Return the broadcast sender for a page, so the WS handler can send directly.
    pub async fn sender(&self, page_id: &str) -> Option<broadcast::Sender<String>> {
        let rooms = self.0.read().await;
        rooms.get(page_id).map(|r| r.tx.clone())
    }

    pub async fn user_count(&self, page_id: &str) -> usize {
        let rooms = self.0.read().await;
        rooms.get(page_id).map(|r| r.users.len()).unwrap_or(0)
    }

    /// Return a snapshot of all (user_id, username) pairs in the room.
    pub async fn users(&self, page_id: &str) -> Vec<(String, String)> {
        let rooms = self.0.read().await;
        rooms
            .get(page_id)
            .map(|r| r.users.iter().map(|(id, name)| (id.clone(), name.clone())).collect())
            .unwrap_or_default()
    }
}
