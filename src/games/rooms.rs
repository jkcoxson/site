// Jackson Coxson
// Shared multiplayer engine: room registry, passcodes, seats with rejoin
// tokens, reconnects, lobby seat pruning and idle-room reaping. Each game
// plugs in through `RoomGame` and only deals with its own rules.

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use serde::{de::DeserializeOwned, Serialize};
use tokio::sync::{mpsc, Mutex, RwLock};

/// Disconnected lobby seats are released after this long so others can join.
const LOBBY_GRACE: Duration = Duration::from_secs(30);
const MAX_NICKNAME: usize = 20;

// ── Seats ─────────────────────────────────────────────────────────────────────

pub struct Seat {
    pub nickname: String,
    /// Rejoin token, generated when the seat is first claimed.
    pub token: String,
    sender: Option<mpsc::UnboundedSender<String>>,
    pub connected: bool,
}

/// The players in a room. A player's id is their index, so ids shift when a
/// lobby seat is pruned.
#[derive(Default)]
pub struct Seats {
    seats: Vec<Seat>,
}

impl Seats {
    pub fn len(&self) -> usize {
        self.seats.len()
    }

    pub fn is_empty(&self) -> bool {
        self.seats.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Seat> {
        self.seats.iter()
    }

    pub fn get(&self, player: usize) -> Option<&Seat> {
        self.seats.get(player)
    }

    pub fn index_of(&self, token: &str) -> Option<usize> {
        self.seats.iter().position(|s| s.token == token)
    }

    pub fn any_connected(&self) -> bool {
        self.seats.iter().any(|s| s.connected)
    }

    pub fn connected_count(&self) -> usize {
        self.seats.iter().filter(|s| s.connected).count()
    }

    pub fn send<T: Serialize>(&self, player: usize, msg: &T) {
        if let Some(Some(tx)) = self.seats.get(player).map(|s| &s.sender) {
            let _ = tx.send(serde_json::to_string(msg).unwrap_or_default());
        }
    }

    pub fn send_others<T: Serialize>(&self, player: usize, msg: &T) {
        for p in 0..self.seats.len() {
            if p != player {
                self.send(p, msg);
            }
        }
    }

    pub fn broadcast<T: Serialize>(&self, msg: &T) {
        let json = serde_json::to_string(msg).unwrap_or_default();
        for s in &self.seats {
            if let Some(tx) = &s.sender {
                let _ = tx.send(json.clone());
            }
        }
    }

    /// Trim and cap a requested nickname. Returns false when it was blank.
    pub fn set_nickname(&mut self, player: usize, name: &str) -> bool {
        let name: String = name.trim().chars().take(MAX_NICKNAME).collect();
        if name.is_empty() {
            return false;
        }
        self.seats[player].nickname = name;
        true
    }

    /// Whoever left during a game gives up their seat when the room goes
    /// back to its lobby.
    pub fn drop_disconnected(&mut self) {
        self.seats.retain(|s| s.connected);
    }

    /// Seat a player with no connection, for driving a game in tests.
    #[cfg(test)]
    pub fn add_for_test(&mut self) -> usize {
        let (tx, _rx) = mpsc::unbounded_channel();
        self.add(tx).0
    }

    fn add(&mut self, tx: mpsc::UnboundedSender<String>) -> (usize, String) {
        let token = gen_token();
        let nickname = gen_nickname(self);
        self.seats.push(Seat {
            nickname,
            token: token.clone(),
            sender: Some(tx),
            connected: true,
        });
        (self.seats.len() - 1, token)
    }
}

// ── Games ─────────────────────────────────────────────────────────────────────

/// One game's rules, plugged into the shared room engine. Hooks run with the
/// room locked; send replies through `seats`.
pub trait RoomGame: Send + 'static {
    type ClientMsg: DeserializeOwned + Send;

    fn capacity(&self) -> usize;
    /// While in the lobby, new players may join and abandoned seats are pruned.
    fn in_lobby(&self) -> bool;
    /// A finished game whose players have all left gets its room removed.
    fn is_over(&self) -> bool;
    /// Short description for the room list.
    fn mode(&self) -> String;

    /// A new player took seat `player` and has been sent `Welcome`.
    fn joined(&mut self, seats: &Seats, player: usize);
    /// A player came back with their token and has been sent `Welcome`.
    fn rejoined(&mut self, seats: &Seats, player: usize);
    fn handle(&mut self, seats: &mut Seats, player: usize, msg: Self::ClientMsg);
    /// A player's socket closed; their seat is already marked disconnected.
    fn left(&mut self, seats: &Seats, player: usize);
    /// An abandoned lobby seat was removed.
    fn pruned(&mut self, seats: &Seats);
}

#[derive(Serialize)]
#[serde(tag = "type", content = "data")]
enum EngineMsg<'a> {
    Welcome { player: u8, token: &'a str },
    Error(&'a str),
}

struct Room<G> {
    seats: Seats,
    game: G,
    /// When the room last became empty (no connected players). `None` while at
    /// least one player is connected. Used by the reaper to clear abandoned rooms.
    empty_since: Option<Instant>,
}

impl<G> Room<G> {
    /// Refresh the empty-room timer based on the current connection state:
    /// clears it while anyone is connected, and starts it the moment the room
    /// becomes empty.
    fn touch_empty(&mut self) {
        if self.seats.any_connected() {
            self.empty_since = None;
        } else if self.empty_since.is_none() {
            self.empty_since = Some(Instant::now());
        }
    }
}

struct LobbyRoom<G> {
    name: String,
    passcode: String,
    room: Arc<Mutex<Room<G>>>,
}

pub struct RoomSummary {
    pub id: String,
    pub name: String,
    pub players: u8,
    pub capacity: u8,
    pub in_progress: bool,
    pub mode: String,
}

// ── Registry ──────────────────────────────────────────────────────────────────

pub struct Rooms<G> {
    rooms: RwLock<HashMap<String, LobbyRoom<G>>>,
}

impl<G: RoomGame> Default for Rooms<G> {
    fn default() -> Self {
        Self::new()
    }
}

impl<G: RoomGame> Rooms<G> {
    pub fn new() -> Self {
        Self {
            rooms: RwLock::new(HashMap::new()),
        }
    }

    pub async fn create(&self, id: String, name: String, passcode: String, game: G) {
        let mut w = self.rooms.write().await;
        w.insert(
            id,
            LobbyRoom {
                name,
                passcode,
                room: Arc::new(Mutex::new(Room {
                    seats: Seats::default(),
                    game,
                    empty_since: Some(Instant::now()),
                })),
            },
        );
    }

    pub async fn list(&self) -> Vec<RoomSummary> {
        let r = self.rooms.read().await;
        let mut infos = Vec::new();
        for (id, lobby) in r.iter() {
            if let Ok(room) = lobby.room.try_lock() {
                infos.push(RoomSummary {
                    id: id.clone(),
                    name: lobby.name.clone(),
                    players: room.seats.connected_count() as u8,
                    capacity: room.game.capacity() as u8,
                    in_progress: !room.game.in_lobby(),
                    mode: room.game.mode(),
                });
            }
        }
        infos.sort_by(|a, b| a.name.cmp(&b.name));
        infos
    }

    pub async fn exists(&self, id: &str) -> bool {
        self.rooms.read().await.contains_key(id)
    }

    async fn get(&self, id: &str) -> Option<(Arc<Mutex<Room<G>>>, String)> {
        let r = self.rooms.read().await;
        r.get(id)
            .map(|lobby| (lobby.room.clone(), lobby.passcode.clone()))
    }

    async fn remove_if_over(&self, id: &str) {
        let should_remove = {
            let r = self.rooms.read().await;
            r.get(id)
                .and_then(|lobby| lobby.room.try_lock().ok())
                .map(|room| room.game.is_over() && !room.seats.any_connected())
                .unwrap_or(false)
        };
        if should_remove {
            self.rooms.write().await.remove(id);
        }
    }

    /// Drop a lobby seat whose player never came back, so the seat opens up.
    async fn prune_lobby_seat(&self, id: &str, token: &str) {
        let Some((room, _)) = self.get(id).await else {
            return;
        };
        let mut r = room.lock().await;
        if !r.game.in_lobby() {
            return;
        }
        if let Some(idx) = r.seats.index_of(token) {
            if !r.seats.seats[idx].connected {
                r.seats.seats.remove(idx);
                let Room { seats, game, .. } = &mut *r;
                game.pruned(seats);
            }
        }
    }

    /// Remove rooms that have had no connected players for at least `idle_ttl`.
    /// Covers rooms abandoned before the game finishes (created-but-never-joined,
    /// quit mid-match) which `remove_if_over` never reaps.
    async fn reap_idle_rooms(&self, idle_ttl: Duration) {
        let stale: Vec<String> = {
            let r = self.rooms.read().await;
            r.iter()
                .filter_map(|(id, lobby)| {
                    let room = lobby.room.try_lock().ok()?;
                    match room.empty_since {
                        Some(t) if t.elapsed() >= idle_ttl => Some(id.clone()),
                        _ => None,
                    }
                })
                .collect()
        };
        if stale.is_empty() {
            return;
        }
        let mut w = self.rooms.write().await;
        for id in stale {
            // Re-check under the write lock: a player may have reconnected
            // between the read scan and acquiring the write lock.
            let still_idle = w
                .get(&id)
                .and_then(|lobby| lobby.room.try_lock().ok())
                .map(|room| room.empty_since.is_some_and(|t| t.elapsed() >= idle_ttl))
                .unwrap_or(false);
            if still_idle {
                w.remove(&id);
            }
        }
    }

    /// Spawn a background task that periodically reaps idle rooms. Rooms empty
    /// for more than an hour are removed.
    pub fn spawn_reaper(self: Arc<Self>) {
        const IDLE_TTL: Duration = Duration::from_secs(60 * 60);
        const SCAN_INTERVAL: Duration = Duration::from_secs(60);
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(SCAN_INTERVAL);
            loop {
                ticker.tick().await;
                self.reap_idle_rooms(IDLE_TTL).await;
            }
        });
    }
}

// ── Seat assignment ───────────────────────────────────────────────────────────

enum SeatResult {
    New(usize, String),
    Reconnected(usize, String),
    InProgress,
    Full,
}

fn gen_token() -> String {
    use rand::RngExt;
    let mut rng = rand::rng();
    (0..8)
        .map(|_| format!("{:02x}", rng.random_range(0u8..=255)))
        .collect()
}

const ADJECTIVES: [&str; 16] = [
    "Bright", "Clever", "Swift", "Brave", "Calm", "Lucky", "Mighty", "Gentle", "Bold", "Sunny",
    "Quiet", "Witty", "Eager", "Jolly", "Keen", "Nimble",
];
const ANIMALS: [&str; 16] = [
    "Otter", "Falcon", "Badger", "Lynx", "Heron", "Fox", "Panda", "Wolf", "Raven", "Tiger",
    "Moose", "Hawk", "Bison", "Crane", "Seal", "Owl",
];

/// A friendly default nickname that nobody else in the room is using.
fn gen_nickname(seats: &Seats) -> String {
    use rand::RngExt;
    let mut rng = rand::rng();
    loop {
        let name = format!(
            "{} {}",
            ADJECTIVES[rng.random_range(0..ADJECTIVES.len())],
            ANIMALS[rng.random_range(0..ANIMALS.len())]
        );
        if seats.iter().all(|s| s.nickname != name) {
            return name;
        }
    }
}

// ── WebSocket session ─────────────────────────────────────────────────────────

/// Run one player's websocket for the room `room_id` until it closes.
pub async fn serve<G: RoomGame>(
    socket: WebSocket,
    rooms: Arc<Rooms<G>>,
    room_id: String,
    passcode: String,
    reconnect_token: Option<String>,
) {
    let (mut sink, mut stream) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    let reject = |msg: &str| serde_json::to_string(&EngineMsg::Error(msg)).unwrap();

    let Some((room, expected_passcode)) = rooms.get(&room_id).await else {
        let _ = sink
            .send(Message::Text(reject("Room not found.").into()))
            .await;
        return;
    };

    if passcode != expected_passcode {
        let _ = sink
            .send(Message::Text(reject("Wrong passcode.").into()))
            .await;
        return;
    }

    // ── Assign/reclaim a seat ──────────────────────────────────────────────
    let seat = {
        let mut r = room.lock().await;
        // Try to reclaim a seat by token; an unknown token is just a new player.
        match reconnect_token
            .as_deref()
            .and_then(|tok| r.seats.index_of(tok))
        {
            Some(idx) => {
                let s = &mut r.seats.seats[idx];
                s.sender = Some(tx.clone());
                s.connected = true;
                let token = s.token.clone();
                r.touch_empty();
                SeatResult::Reconnected(idx, token)
            }
            None if !r.game.in_lobby() => SeatResult::InProgress,
            None if r.seats.len() >= r.game.capacity() => SeatResult::Full,
            None => {
                let (idx, token) = r.seats.add(tx.clone());
                r.touch_empty();
                SeatResult::New(idx, token)
            }
        }
    };

    // Player ids shift when a lobby seat is pruned, so the socket remembers
    // its token and looks its index up fresh each time.
    let token = match seat {
        SeatResult::Full => {
            let _ = sink
                .send(Message::Text(reject("Room is full.").into()))
                .await;
            return;
        }
        SeatResult::InProgress => {
            let _ = sink
                .send(Message::Text(reject("Game already in progress.").into()))
                .await;
            return;
        }
        SeatResult::New(idx, token) | SeatResult::Reconnected(idx, token) => {
            let returning = reconnect_token.as_deref() == Some(token.as_str());
            let mut r = room.lock().await;
            let Room { seats, game, .. } = &mut *r;
            seats.send(
                idx,
                &EngineMsg::Welcome {
                    player: idx as u8,
                    token: &token,
                },
            );
            if returning {
                game.rejoined(seats, idx);
            } else {
                game.joined(seats, idx);
            }
            token
        }
    };

    // ── Forward channel messages to WebSocket sink ─────────────────────────
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sink.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    while let Some(Ok(msg)) = stream.next().await {
        let text = match msg {
            Message::Text(t) => t.to_string(),
            Message::Close(_) => break,
            _ => continue,
        };
        if let Ok(client_msg) = serde_json::from_str::<G::ClientMsg>(&text) {
            let mut r = room.lock().await;
            let Room { seats, game, .. } = &mut *r;
            if let Some(idx) = seats.index_of(&token) {
                game.handle(seats, idx, client_msg);
            }
        }
    }

    send_task.abort();

    // Mark the seat as disconnected (keep game state alive)
    let prune = {
        let mut r = room.lock().await;
        match r.seats.index_of(&token) {
            // A newer socket may have reclaimed this seat already.
            Some(idx)
                if r.seats.seats[idx]
                    .sender
                    .as_ref()
                    .is_some_and(|s| s.same_channel(&tx)) =>
            {
                let s = &mut r.seats.seats[idx];
                s.connected = false;
                s.sender = None;
                r.touch_empty();
                let Room { seats, game, .. } = &mut *r;
                game.left(seats, idx);
                game.in_lobby()
            }
            _ => false,
        }
    };
    if prune {
        let rooms = rooms.clone();
        let room_id = room_id.clone();
        tokio::spawn(async move {
            tokio::time::sleep(LOBBY_GRACE).await;
            rooms.prune_lobby_seat(&room_id, &token).await;
        });
    }

    // Only remove the room if the game is over AND everyone disconnected
    rooms.remove_if_over(&room_id).await;
}
