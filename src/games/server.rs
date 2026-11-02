// Jackson Coxson
// Game server: WebSocket room manager with lobby, passcodes, reconnect support.

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Extension, Path, Query,
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex, RwLock};

use crate::games::rays_of_light::logic::{
    boards_match, is_slash, rotate_piece, trace_ray, validate_board, Board, Color, ResyncPhase,
    ResyncShot,
};

// ── Wire messages ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ServerMsg {
    /// Sent immediately on connect. `token` is used to reclaim this slot on reconnect.
    Welcome { player: u8, token: String },
    WaitingForOpponent,
    GameStart,
    YourTurn,
    /// Play has begun but it's the opponent's move first.
    OpponentTurn,
    RayResult { call: u8, exit: u8, color: Color },
    OpponentShot { call: u8, exit: u8, color: Color },
    OpponentLocked,
    GuessIncorrect,
    OpponentCalledGame,
    RotateResult { row: u8, col: u8, accepted: bool },
    /// The opponent attempted to flip one of your cells. `accepted` is true when
    /// they correctly guessed a mirror's location (and it was rotated).
    YourMirrorRotated { row: u8, col: u8, accepted: bool },
    GameOver { you_win: bool, opponent_board: Board },
    /// Full state snapshot sent after a successful reconnect.
    GameResync { player: u8, phase: ResyncPhase },
    /// Sent to the other player when their opponent reconnects.
    OpponentReconnected,
    /// Sent to the other player when their opponent disconnects.
    OpponentDisconnected,
    Error(String),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ClientMsg {
    LockBoard { board: Board },
    ShootRay { port: u8 },
    CallGame { guess: Board },
    RequestRotate { row: u8, col: u8 },
}

// ── Match state machine ───────────────────────────────────────────────────────

enum MatchPhase {
    WaitingForPlayers,
    Setup { locked: [bool; 2], boards: [Option<Board>; 2] },
    Playing { boards: [Board; 2], current: usize, rotated: bool },
    Over { boards: [Board; 2] },
}

struct ShotRecord {
    player: usize,
    call: u8,
    exit: u8,
    color: Color,
}

struct Match {
    phase: MatchPhase,
    senders: [Option<mpsc::UnboundedSender<String>>; 2],
    /// Rejoin tokens; generated when a slot is first claimed.
    slot_tokens: [Option<String>; 2],
    /// Whether each player is currently connected.
    connected: [bool; 2],
    shot_history: Vec<ShotRecord>,
    /// Track the final winner for resync after Over.
    winner: Option<usize>,
    /// When the room last became empty (no connected players). `None` while at
    /// least one player is connected. Used by the reaper to clear abandoned rooms.
    empty_since: Option<Instant>,
}

impl Match {
    fn new() -> Self {
        Self {
            phase: MatchPhase::WaitingForPlayers,
            senders: [None, None],
            slot_tokens: [None, None],
            connected: [false, false],
            shot_history: Vec::new(),
            winner: None,
            empty_since: Some(Instant::now()),
        }
    }

    /// Refresh the empty-room timer based on the current connection state:
    /// clears it while anyone is connected, and starts it the moment the room
    /// becomes empty.
    fn touch_empty(&mut self) {
        if self.connected.iter().any(|&c| c) {
            self.empty_since = None;
        } else if self.empty_since.is_none() {
            self.empty_since = Some(Instant::now());
        }
    }

    fn send(&self, player: usize, msg: &ServerMsg) {
        if let Some(Some(tx)) = self.senders.get(player) {
            let _ = tx.send(serde_json::to_string(msg).unwrap_or_default());
        }
    }

    fn broadcast(&self, msg: &ServerMsg) {
        let json = serde_json::to_string(msg).unwrap_or_default();
        for slot in &self.senders {
            if let Some(tx) = slot {
                let _ = tx.send(json.clone());
            }
        }
    }

    fn resync(&self, player: usize) -> ResyncPhase {
        match &self.phase {
            MatchPhase::WaitingForPlayers => ResyncPhase::WaitingForOpponent,
            MatchPhase::Setup { locked, boards } => ResyncPhase::Setup {
                your_board: boards[player],
                you_locked: locked[player],
                opponent_locked: locked[1 - player],
            },
            MatchPhase::Playing { boards, current, rotated } => ResyncPhase::Playing {
                your_board: boards[player],
                shots: self.shot_history.iter().map(|s| ResyncShot {
                    mine: s.player == player,
                    call: s.call,
                    exit: s.exit,
                    color: s.color,
                }).collect(),
                your_turn: *current == player,
                you_rotated: *current == player && *rotated,
            },
            MatchPhase::Over { boards } => ResyncPhase::Over {
                you_win: self.winner == Some(player),
                your_board: boards[player],
                opponent_board: boards[1 - player],
                shots: self.shot_history.iter().map(|s| ResyncShot {
                    mine: s.player == player,
                    call: s.call,
                    exit: s.exit,
                    color: s.color,
                }).collect(),
            },
        }
    }

    fn handle(&mut self, player: usize, msg: ClientMsg) {
        match msg {
            ClientMsg::LockBoard { board } => self.on_lock_board(player, board),
            ClientMsg::ShootRay { port } => self.on_shoot(player, port),
            ClientMsg::CallGame { guess } => self.on_call_game(player, guess),
            ClientMsg::RequestRotate { row, col } => self.on_request_rotate(player, row, col),
        }
    }

    fn on_lock_board(&mut self, player: usize, board: Board) {
        if !validate_board(&board) {
            self.send(player, &ServerMsg::Error("Invalid board.".into()));
            return;
        }
        enum Outcome { NotInSetup, Locked { other: usize }, BothLocked { other: usize, boards: [Board; 2] } }
        let outcome = match &mut self.phase {
            MatchPhase::Setup { locked, boards } => {
                boards[player] = Some(board);
                locked[player] = true;
                let other = 1 - player;
                if locked[0] && locked[1] {
                    Outcome::BothLocked { other, boards: [boards[0].unwrap(), boards[1].unwrap()] }
                } else {
                    Outcome::Locked { other }
                }
            }
            _ => Outcome::NotInSetup,
        };
        match outcome {
            Outcome::NotInSetup => {}
            Outcome::Locked { other } => self.send(other, &ServerMsg::OpponentLocked),
            Outcome::BothLocked { other, boards } => {
                self.send(other, &ServerMsg::OpponentLocked);
                self.phase = MatchPhase::Playing { boards, current: 0, rotated: false };
                self.send(0, &ServerMsg::YourTurn);
                self.send(1, &ServerMsg::OpponentTurn);
            }
        }
    }

    fn on_shoot(&mut self, player: usize, port: u8) {
        struct ShotData { other: usize, exit: u8, color: Color }
        enum Outcome { NotYourTurn, InvalidPort, NotPlaying, Shot(ShotData) }
        let outcome = match &mut self.phase {
            MatchPhase::Playing { boards, current, rotated } => {
                if *current != player { Outcome::NotYourTurn }
                else if !(1..=16).contains(&port) { Outcome::InvalidPort }
                else {
                    let other = 1 - player;
                    let (exit, color) = trace_ray(&boards[other], port);
                    *current = other;
                    // Fresh turn for the opponent — they get a new flip.
                    *rotated = false;
                    Outcome::Shot(ShotData { other, exit, color })
                }
            }
            _ => Outcome::NotPlaying,
        };
        match outcome {
            Outcome::NotYourTurn => self.send(player, &ServerMsg::Error("Not your turn.".into())),
            Outcome::InvalidPort => self.send(player, &ServerMsg::Error("Invalid port.".into())),
            Outcome::NotPlaying => {}
            Outcome::Shot(d) => {
                self.shot_history.push(ShotRecord { player, call: port, exit: d.exit, color: d.color });
                self.send(player, &ServerMsg::RayResult { call: port, exit: d.exit, color: d.color });
                self.send(d.other, &ServerMsg::OpponentShot { call: port, exit: d.exit, color: d.color });
                self.send(d.other, &ServerMsg::YourTurn);
            }
        }
    }

    fn on_call_game(&mut self, player: usize, guess: Board) {
        enum Outcome { NotPlaying, NotYourTurn, Correct { boards: [Board; 2] }, Wrong }
        let outcome = match &mut self.phase {
            MatchPhase::Playing { boards, current, rotated } => {
                if *current != player {
                    Outcome::NotYourTurn
                } else if boards_match(&guess, &boards[1 - player]) {
                    Outcome::Correct { boards: *boards }
                } else {
                    // A wrong guess ends your turn, just like shooting does.
                    *current = 1 - player;
                    *rotated = false;
                    Outcome::Wrong
                }
            }
            _ => Outcome::NotPlaying,
        };
        let other = 1 - player;
        match outcome {
            Outcome::NotPlaying => {}
            Outcome::NotYourTurn => {
                self.send(player, &ServerMsg::Error("You can only call the game on your turn.".into()))
            }
            Outcome::Correct { boards } => {
                self.winner = Some(player);
                self.phase = MatchPhase::Over { boards };
                // Each player sees the opponent's actual board — the answer.
                self.send(player, &ServerMsg::GameOver { you_win: true, opponent_board: boards[other] });
                self.send(other, &ServerMsg::GameOver { you_win: false, opponent_board: boards[player] });
            }
            Outcome::Wrong => {
                self.send(player, &ServerMsg::GuessIncorrect);
                self.send(other, &ServerMsg::OpponentCalledGame);
                self.send(other, &ServerMsg::YourTurn);
            }
        }
    }

    fn on_request_rotate(&mut self, player: usize, row: u8, col: u8) {
        if row > 3 || col > 3 {
            self.send(player, &ServerMsg::Error("Invalid coordinates.".into()));
            return;
        }
        let other = 1 - player;
        enum Outcome { NotPlaying, NotYourTurn, AlreadyFlipped, Done { accepted: bool } }
        let outcome = match &mut self.phase {
            MatchPhase::Playing { boards, current, rotated } => {
                if *current != player {
                    Outcome::NotYourTurn
                } else if *rotated {
                    Outcome::AlreadyFlipped
                } else {
                    // The attempt is spent whether or not it lands on a mirror.
                    *rotated = true;
                    let accepted = match boards[other][row as usize][col as usize] {
                        Some(p) if is_slash(p) => {
                            boards[other][row as usize][col as usize] = Some(rotate_piece(p));
                            true
                        }
                        _ => false,
                    };
                    // A correct flip keeps your turn (you still shoot); flipping
                    // a cell with no mirror loses your turn.
                    if !accepted {
                        *current = other;
                        *rotated = false;
                    }
                    Outcome::Done { accepted }
                }
            }
            _ => Outcome::NotPlaying,
        };
        match outcome {
            Outcome::NotPlaying => {}
            Outcome::NotYourTurn => {
                self.send(player, &ServerMsg::Error("You can only flip a mirror on your turn.".into()))
            }
            Outcome::AlreadyFlipped => {
                self.send(player, &ServerMsg::Error("You can only flip once per turn.".into()))
            }
            Outcome::Done { accepted } => {
                self.send(player, &ServerMsg::RotateResult { row, col, accepted });
                if accepted {
                    self.send(other, &ServerMsg::YourMirrorRotated { row, col, accepted });
                } else {
                    // Turn passed to the opponent. Send YourTurn first — its
                    // client handler clears the last-mirror notice — then the
                    // flip-attempt notice so the opponent still sees it.
                    self.send(other, &ServerMsg::YourTurn);
                    self.send(other, &ServerMsg::YourMirrorRotated { row, col, accepted });
                }
            }
        }
    }
}

// ── Lobby ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomInfo {
    pub id: String,
    pub name: String,
    pub players: u8,
    pub in_progress: bool,
}

struct LobbyRoom {
    name: String,
    passcode: String,
    game: Arc<Mutex<Match>>,
}

// ── Global state ──────────────────────────────────────────────────────────────

pub struct GameState {
    rooms: RwLock<HashMap<String, LobbyRoom>>,
}

impl Default for GameState {
    fn default() -> Self { Self::new() }
}

impl GameState {
    pub fn new() -> Self {
        Self { rooms: RwLock::new(HashMap::new()) }
    }

    pub async fn create_room(&self, id: String, name: String, passcode: String) {
        let mut w = self.rooms.write().await;
        w.insert(id, LobbyRoom { name, passcode, game: Arc::new(Mutex::new(Match::new())) });
    }

    pub async fn list_rooms(&self) -> Vec<RoomInfo> {
        let r = self.rooms.read().await;
        let mut infos = Vec::new();
        for (id, room) in r.iter() {
            if let Ok(game) = room.game.try_lock() {
                let players = game.connected.iter().filter(|&&c| c).count() as u8;
                let in_progress = matches!(game.phase, MatchPhase::Playing { .. });
                infos.push(RoomInfo { id: id.clone(), name: room.name.clone(), players, in_progress });
            }
        }
        infos.sort_by(|a, b| a.name.cmp(&b.name));
        infos
    }

    pub async fn room_exists(&self, id: &str) -> bool {
        self.rooms.read().await.contains_key(id)
    }

    async fn get_room(&self, id: &str) -> Option<(Arc<Mutex<Match>>, String)> {
        let r = self.rooms.read().await;
        r.get(id).map(|room| (room.game.clone(), room.passcode.clone()))
    }

    async fn remove_if_over(&self, id: &str) {
        let should_remove = {
            let r = self.rooms.read().await;
            r.get(id).and_then(|room| room.game.try_lock().ok())
                .map(|game| matches!(game.phase, MatchPhase::Over { .. }) && game.connected.iter().all(|&c| !c))
                .unwrap_or(false)
        };
        if should_remove {
            self.rooms.write().await.remove(id);
        }
    }

    /// Remove rooms that have had no connected players for at least `idle_ttl`.
    /// Covers rooms abandoned before the game finishes (created-but-never-joined,
    /// quit mid-match) which `remove_if_over` never reaps.
    async fn reap_idle_rooms(&self, idle_ttl: Duration) {
        let stale: Vec<String> = {
            let r = self.rooms.read().await;
            r.iter()
                .filter_map(|(id, room)| {
                    let game = room.game.try_lock().ok()?;
                    match game.empty_since {
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
                .and_then(|room| room.game.try_lock().ok())
                .map(|game| game.empty_since.is_some_and(|t| t.elapsed() >= idle_ttl))
                .unwrap_or(false);
            if still_idle {
                w.remove(&id);
            }
        }
    }

    /// Spawn a background task that periodically reaps idle rooms. Rooms empty
    /// for more than 5 minutes are removed.
    pub fn spawn_reaper(self: Arc<Self>) {
        const IDLE_TTL: Duration = Duration::from_secs(5 * 60);
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

// ── Slot assignment ───────────────────────────────────────────────────────────

enum SlotResult {
    New { player_id: usize, token: String },
    Reconnected { player_id: usize, token: String },
    Full,
}

fn assign_new_slot(r: &mut Match, tx: mpsc::UnboundedSender<String>) -> SlotResult {
    for idx in 0..2 {
        if !r.connected[idx] && r.slot_tokens[idx].is_none() {
            let token = gen_token();
            r.slot_tokens[idx] = Some(token.clone());
            r.senders[idx] = Some(tx);
            r.connected[idx] = true;
            r.touch_empty();
            return SlotResult::New { player_id: idx, token };
        }
    }
    SlotResult::Full
}

// ── Token generation ──────────────────────────────────────────────────────────

fn gen_token() -> String {
    use rand::RngExt;
    let mut rng = rand::rng();
    (0..8).map(|_| format!("{:02x}", rng.random_range(0u8..=255))).collect()
}

// ── WebSocket URL query params ────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct WsQuery {
    token: Option<String>,
}

// ── Axum WebSocket handler ────────────────────────────────────────────────────

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Path((room_id, passcode)): Path<(String, String)>,
    Query(query): Query<WsQuery>,
    Extension(state): Extension<Arc<GameState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, room_id, passcode, query.token, state))
}

async fn handle_socket(
    socket: WebSocket,
    room_id: String,
    passcode: String,
    reconnect_token: Option<String>,
    state: Arc<GameState>,
) {
    let (mut sink, mut stream) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    let Some((game, expected_passcode)) = state.get_room(&room_id).await else {
        let msg = serde_json::to_string(&ServerMsg::Error("Room not found.".into())).unwrap();
        let _ = sink.send(Message::Text(msg.into())).await;
        return;
    };

    if passcode != expected_passcode {
        let msg = serde_json::to_string(&ServerMsg::Error("Wrong passcode.".into())).unwrap();
        let _ = sink.send(Message::Text(msg.into())).await;
        return;
    }

    // ── Assign/reclaim player slot ─────────────────────────────────────────
    let slot_result = {
        let mut r = game.lock().await;

        // Try to reclaim a slot by token
        if let Some(ref tok) = reconnect_token {
            if let Some(idx) = r.slot_tokens.iter().position(|t| t.as_deref() == Some(tok.as_str())) {
                r.senders[idx] = Some(tx.clone());
                r.connected[idx] = true;
                r.touch_empty();
                SlotResult::Reconnected { player_id: idx, token: tok.clone() }
            } else {
                // Token didn't match — try fresh slot
                assign_new_slot(&mut r, tx.clone())
            }
        } else {
            assign_new_slot(&mut r, tx.clone())
        }
    };

    let player_id = match slot_result {
        SlotResult::Full => {
            let msg = serde_json::to_string(&ServerMsg::Error("Room is full.".into())).unwrap();
            let _ = sink.send(Message::Text(msg.into())).await;
            return;
        }
        SlotResult::New { player_id, token } => {
            let welcome = serde_json::to_string(&ServerMsg::Welcome {
                player: player_id as u8,
                token,
            }).unwrap();
            let _ = sink.send(Message::Text(welcome.into())).await;

            if player_id == 1 {
                let mut r = game.lock().await;
                r.phase = MatchPhase::Setup { locked: [false; 2], boards: [None; 2] };
                r.broadcast(&ServerMsg::GameStart);
            } else {
                let msg = serde_json::to_string(&ServerMsg::WaitingForOpponent).unwrap();
                let _ = sink.send(Message::Text(msg.into())).await;
            }
            player_id
        }
        SlotResult::Reconnected { player_id, token } => {
            let (welcome, resync) = {
                let r = game.lock().await;
                let w = serde_json::to_string(&ServerMsg::Welcome {
                    player: player_id as u8,
                    token,
                }).unwrap();
                let rs = serde_json::to_string(&ServerMsg::GameResync {
                    player: player_id as u8,
                    phase: r.resync(player_id),
                }).unwrap();
                (w, rs)
            };
            let _ = sink.send(Message::Text(welcome.into())).await;
            let _ = sink.send(Message::Text(resync.into())).await;
            let r = game.lock().await;
            r.send(1 - player_id, &ServerMsg::OpponentReconnected);
            player_id
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
        if let Ok(client_msg) = serde_json::from_str::<ClientMsg>(&text) {
            let mut r = game.lock().await;
            r.handle(player_id, client_msg);
        }
    }

    send_task.abort();

    // Mark slot as disconnected (keep game state alive)
    {
        let mut r = game.lock().await;
        r.connected[player_id] = false;
        r.touch_empty();
        let other = 1 - player_id;
        r.send(other, &ServerMsg::OpponentDisconnected);
    }

    // Only remove the room if the game is over AND both players disconnected
    state.remove_if_over(&room_id).await;
}

