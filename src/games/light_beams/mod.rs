// Jackson Coxson

pub mod beam_tracing;
pub mod logic;

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use beam_tracing::trace_beam;
use logic::{
    board_counts, rotate_piece, same_piece_type, ArrowDir, Board, BoardSource, Color, LobbyPlayer,
    Piece, PieceCounts, PieceType, PlayerResult, Resolution, ResyncPhase, Rules, SlashDir,
    MAX_SIZE, MIN_SIZE,
};

#[derive(Clone, Debug, PartialEq, Default)]
pub enum Phase {
    #[default]
    Connecting,
    /// The waiting room, before the host starts the game.
    Lobby,
    Setup,
    Playing {
        your_turn: bool,
    },
    /// The independent engine (async / computer-board) has begun, can shoot freely.
    Independent {
        you_done: bool,
    },
    Reconnecting,
    GameOver {
        you_win: bool,
    },
    /// The connection was rejected and won't be retried (e.g. the room doesn't
    /// exist, the passcode is wrong, or the room is full). The error banner
    /// explains why.
    Closed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ServerMsg {
    Welcome {
        player: u8,
        token: String,
    },
    Lobby {
        players: Vec<LobbyPlayer>,
        host: u8,
        you: u8,
        rules: Rules,
    },
    GameStart {
        rules: Rules,
    },
    YourTurn,
    OpponentTurn,
    BeginPlay,
    BeamResult {
        call: u8,
        exit: u8,
        color: Color,
    },
    OpponentShot {
        call: u8,
        exit: u8,
        color: Color,
    },
    OpponentLocked,
    GuessIncorrect,
    OpponentCalledGame,
    RotateResult {
        row: u8,
        col: u8,
        accepted: bool,
    },
    YourMirrorRotated {
        row: u8,
        col: u8,
        accepted: bool,
    },
    YouFinished {
        score: u32,
    },
    PlayerFinished {
        player: u8,
    },
    GameOver {
        you_win: bool,
        your_board: Board,
        answer_board: Board,
        results: Vec<PlayerResult>,
        winner: Option<u8>,
    },
    /// Full state snapshot after reconnect.
    GameResync {
        player: u8,
        rules: Rules,
        players: Vec<LobbyPlayer>,
        host: u8,
        phase: ResyncPhase,
    },
    PlayerConnected {
        player: u8,
    },
    PlayerDisconnected {
        player: u8,
    },
    Error(String),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ClientMsg {
    SetNickname { name: String },
    StartGame,
    LockBoard { board: Board },
    ShootBeam { port: u8 },
    CallGame { guess: Board },
    RequestRotate { row: u8, col: u8 },
    PlayAgain,
}

#[derive(Clone, Debug)]
pub struct Shot {
    pub call: u8,
    pub exit: u8,
    pub color: Color,
    pub mine: bool,
}

/// A single entry in the single-player history log. Every entry counts as one
/// beam toward the score, so wrong guesses and wasted flips are recorded here
/// alongside real shots to explain where the tally comes from.
#[derive(Clone, Debug)]
enum SoloEvent {
    Beam(Shot),
    WrongGuess,
    WastedFlip { row: u8, col: u8 },
}

#[derive(Clone, Debug, Default)]
pub struct GameUiState {
    pub phase: Phase,
    pub player: Option<u8>,
    pub room_id: String,
    pub rules: Rules,
    pub players: Vec<LobbyPlayer>,
    pub host: u8,
    pub your_board: Board,
    pub guess_board: Board,
    /// The board you were guessing, revealed when the game ends.
    pub opponent_board: Option<Board>,
    pub shot_history: Vec<Shot>,
    pub opponent_locked: bool,
    pub error: Option<String>,
    pub last_guess_wrong: bool,
    /// Result of your own flip attempt this turn: (row, col, hit a mirror).
    pub last_rotate_result: Option<(u8, u8, bool)>,
    /// Opponent's flip attempt against your board: (row, col, hit a mirror).
    pub last_mirror_attempt: Option<(u8, u8, bool)>,
    /// Whether you've already used your one flip this turn.
    pub flip_used_this_turn: bool,
    /// Rejoin token for this slot, persisted to localStorage.
    pub token: Option<String>,
    pub opponent_disconnected: bool,
    /// Finished flags for every player in an independent game.
    pub done: Vec<bool>,
    pub you_done: bool,
    pub results: Vec<PlayerResult>,
    pub winner: Option<u8>,
}

/// Errors the server sends immediately before closing the socket. Reconnecting
/// after one of these is pointless, so the client treats them as terminal.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn is_fatal_error(msg: &str) -> bool {
    matches!(msg, "Room not found." | "Wrong passcode." | "Room is full.")
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn to_shots(shots: Vec<logic::ResyncShot>) -> Vec<Shot> {
    shots
        .into_iter()
        .map(|s| Shot {
            call: s.call,
            exit: s.exit,
            color: s.color,
            mine: s.mine,
        })
        .collect()
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
impl GameUiState {
    fn new(room_id: String) -> Self {
        Self {
            room_id,
            ..Default::default()
        }
    }

    fn me(&self) -> usize {
        self.player.unwrap_or(0) as usize
    }

    /// Fresh boards and history for a new game with the current rules.
    fn reset_boards(&mut self) {
        self.your_board = Board::new(self.rules.size);
        self.guess_board = Board::new(self.rules.size);
        self.opponent_board = None;
        self.shot_history.clear();
        self.opponent_locked = false;
        self.flip_used_this_turn = false;
        self.done = vec![false; self.players.len()];
        self.you_done = false;
        self.results.clear();
        self.winner = None;
    }

    fn refresh_disconnected(&mut self) {
        let me = self.player;
        self.opponent_disconnected = self
            .players
            .iter()
            .any(|p| Some(p.id) != me && !p.connected);
    }

    fn set_connected(&mut self, player: u8, connected: bool) {
        if let Some(p) = self.players.iter_mut().find(|p| p.id == player) {
            p.connected = connected;
        }
        self.refresh_disconnected();
    }

    fn apply(&mut self, msg: ServerMsg) {
        self.error = None;
        self.last_guess_wrong = false;
        self.last_rotate_result = None;
        self.last_mirror_attempt = None;
        match msg {
            ServerMsg::Welcome { player, token } => {
                self.player = Some(player);
                self.token = Some(token);
            }
            ServerMsg::Lobby {
                players,
                host,
                you,
                rules,
            } => {
                self.player = Some(you);
                self.players = players;
                self.host = host;
                self.rules = rules;
                self.phase = Phase::Lobby;
                self.refresh_disconnected();
            }
            ServerMsg::GameStart { rules } => {
                self.rules = rules;
                self.reset_boards();
                // A computer board skips setup and goes straight to shooting.
                self.phase = match rules.source {
                    BoardSource::Versus => Phase::Setup,
                    BoardSource::Computer => Phase::Independent { you_done: false },
                };
            }
            ServerMsg::YourTurn => {
                self.phase = Phase::Playing { your_turn: true };
                // A new turn begins, refresh the flip allowance.
                self.flip_used_this_turn = false;
            }
            ServerMsg::OpponentTurn => self.phase = Phase::Playing { your_turn: false },
            ServerMsg::BeginPlay => {
                self.flip_used_this_turn = false;
                self.phase = Phase::Independent { you_done: false };
            }
            ServerMsg::BeamResult { call, exit, color } => {
                self.shot_history.push(Shot {
                    call,
                    exit,
                    color,
                    mine: true,
                });
                if matches!(self.phase, Phase::Independent { .. }) {
                    // Shooting ends the "turn" and refreshes the flip.
                    self.flip_used_this_turn = false;
                } else {
                    self.phase = Phase::Playing { your_turn: false };
                }
            }
            ServerMsg::OpponentShot { call, exit, color } => {
                self.shot_history.push(Shot {
                    call,
                    exit,
                    color,
                    mine: false,
                });
            }
            ServerMsg::OpponentLocked => self.opponent_locked = true,
            ServerMsg::GuessIncorrect => {
                self.last_guess_wrong = true;
                // A wrong guess ends your turn.
                if !matches!(self.phase, Phase::Independent { .. }) {
                    self.phase = Phase::Playing { your_turn: false };
                }
            }
            ServerMsg::OpponentCalledGame => {}
            ServerMsg::RotateResult { row, col, accepted } => {
                self.last_rotate_result = Some((row, col, accepted));
                // A correct flip turns the real mirror, so flip the guess too.
                if accepted {
                    let (r, c) = (row as usize, col as usize);
                    if let Some(p) = self.guess_board.get(r, c).filter(|p| logic::is_slash(*p)) {
                        self.guess_board.set(r, c, Some(rotate_piece(p)));
                    }
                }
                // Your one flip for this turn is now spent.
                self.flip_used_this_turn = true;
                // Flipping a cell with no mirror loses your turn.
                if !accepted && !matches!(self.phase, Phase::Independent { .. }) {
                    self.phase = Phase::Playing { your_turn: false };
                }
            }
            ServerMsg::YourMirrorRotated { row, col, accepted } => {
                if accepted {
                    let (r, c) = (row as usize, col as usize);
                    if let Some(p) = self.your_board.get(r, c) {
                        self.your_board.set(r, c, Some(rotate_piece(p)));
                    }
                }
                self.last_mirror_attempt = Some((row, col, accepted));
            }
            ServerMsg::YouFinished { .. } => {
                self.you_done = true;
                self.phase = Phase::Independent { you_done: true };
                let me = self.me();
                if let Some(d) = self.done.get_mut(me) {
                    *d = true;
                }
            }
            ServerMsg::PlayerFinished { player } => {
                if let Some(d) = self.done.get_mut(player as usize) {
                    *d = true;
                }
            }
            ServerMsg::GameOver {
                you_win,
                your_board,
                answer_board,
                results,
                winner,
            } => {
                self.your_board = your_board;
                self.opponent_board = Some(answer_board);
                self.results = results;
                self.winner = winner;
                self.phase = Phase::GameOver { you_win };
            }
            ServerMsg::GameResync {
                player,
                rules,
                players,
                host,
                phase,
            } => {
                self.player = Some(player);
                self.rules = rules;
                self.players = players;
                self.host = host;
                self.reset_boards();
                self.phase = match &phase {
                    ResyncPhase::Lobby => Phase::Lobby,
                    ResyncPhase::Setup { .. } => Phase::Setup,
                    ResyncPhase::Playing { your_turn, .. } => Phase::Playing {
                        your_turn: *your_turn,
                    },
                    ResyncPhase::Independent { you_done, .. } => Phase::Independent {
                        you_done: *you_done,
                    },
                    ResyncPhase::Over { you_win, .. } => Phase::GameOver { you_win: *you_win },
                };
                match phase {
                    ResyncPhase::Lobby => {}
                    ResyncPhase::Setup {
                        your_board,
                        you_locked: _,
                        opponent_locked,
                    } => {
                        if let Some(b) = your_board {
                            self.your_board = b;
                        }
                        self.opponent_locked = opponent_locked;
                    }
                    ResyncPhase::Playing {
                        your_board,
                        shots,
                        you_rotated,
                        ..
                    } => {
                        self.your_board = your_board;
                        self.flip_used_this_turn = you_rotated;
                        self.shot_history = to_shots(shots);
                    }
                    ResyncPhase::Independent {
                        your_board,
                        shots,
                        you_done,
                        done,
                        ..
                    } => {
                        self.your_board = your_board;
                        self.shot_history = to_shots(shots);
                        self.you_done = you_done;
                        self.done = done;
                    }
                    ResyncPhase::Over {
                        your_board,
                        opponent_board,
                        shots,
                        results,
                        winner,
                        ..
                    } => {
                        self.your_board = your_board;
                        self.opponent_board = Some(opponent_board);
                        self.shot_history = to_shots(shots);
                        self.results = results;
                        self.winner = winner;
                    }
                }
                self.refresh_disconnected();
            }
            ServerMsg::PlayerConnected { player } => self.set_connected(player, true),
            ServerMsg::PlayerDisconnected { player } => self.set_connected(player, false),
            ServerMsg::Error(e) => {
                // Connection-rejection errors are terminal, the server closes
                // the socket right after sending them, so there's nothing to
                // reconnect to. Show the message instead of looping forever.
                if is_fatal_error(&e) {
                    self.phase = Phase::Closed;
                }
                self.error = Some(e);
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;

#[cfg(target_arch = "wasm32")]
thread_local! {
    static GAME_WS: RefCell<Option<web_sys::WebSocket>> = const { RefCell::new(None) };
}

#[cfg(target_arch = "wasm32")]
fn send_msg(msg: &ClientMsg) {
    let json = match serde_json::to_string(msg) {
        Ok(j) => j,
        Err(_) => return,
    };
    GAME_WS.with(|ws| {
        if let Some(ws) = ws.borrow().as_ref() {
            let _ = ws.send_with_str(&json);
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn send_msg(_msg: &ClientMsg) {}

#[cfg(target_arch = "wasm32")]
fn token_storage_key(room_id: &str) -> String {
    format!("rol_token_{room_id}")
}

/// Convert internal `(row, col)` board indices into the port numbers a player
/// actually reads off the edges of the board: the number printed above the
/// column and the number printed beside the row. The top edge is labelled
/// 1..n left-to-right and the left edge 4n..3n+1 top-to-bottom, so the
/// top-left cell of a 4×4 board reads as (1, 16). Returns `(top, left)`.
fn cell_label(row: u8, col: u8, size: u8) -> (u32, u32) {
    (col as u32 + 1, 4 * size as u32 - row as u32)
}

#[cfg(target_arch = "wasm32")]
fn save_token(room_id: &str, token: &str) {
    if let Some(s) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = s.set_item(&token_storage_key(room_id), token);
    }
}

#[cfg(target_arch = "wasm32")]
fn load_token(room_id: &str) -> Option<String> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())?
        .get_item(&token_storage_key(room_id))
        .ok()
        .flatten()
}

#[cfg(target_arch = "wasm32")]
fn clear_token(room_id: &str) {
    if let Some(s) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = s.remove_item(&token_storage_key(room_id));
    }
}

#[cfg(target_arch = "wasm32")]
fn schedule_reconnect(
    room_id: String,
    passcode: String,
    state: RwSignal<GameUiState>,
    delay_ms: i32,
) {
    use wasm_bindgen_futures::spawn_local;
    spawn_local(async move {
        let promise = js_sys::Promise::new(&mut |resolve, _| {
            if let Some(w) = web_sys::window() {
                let _ = w.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, delay_ms);
            }
        });
        let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
        if state.with_untracked(|s| s.phase == Phase::Reconnecting) {
            connect_ws(room_id, passcode, state, (delay_ms * 2).min(30_000));
        }
    });
}

#[cfg(target_arch = "wasm32")]
fn connect_ws(room_id: String, passcode: String, state: RwSignal<GameUiState>, next_delay_ms: i32) {
    use wasm_bindgen::{closure::Closure, JsCast};
    use web_sys::{MessageEvent, WebSocket};

    let window = match web_sys::window() {
        Some(w) => w,
        None => return,
    };
    let location = window.location();
    let proto = location.protocol().unwrap_or_default();
    let ws_proto = if proto == "https:" { "wss" } else { "ws" };
    let host = location.host().unwrap_or_default();

    let token_suffix = load_token(&room_id)
        .map(|t| format!("?token={t}"))
        .unwrap_or_default();
    let url = format!("{ws_proto}://{host}/games/ws/{room_id}/{passcode}{token_suffix}");

    let ws = match WebSocket::new(&url) {
        Ok(w) => w,
        Err(_) => {
            state.update(|s| s.error = Some("Failed to connect to game server.".into()));
            return;
        }
    };

    let rid1 = room_id.clone();
    let onmessage = Closure::<dyn FnMut(MessageEvent)>::new(move |e: MessageEvent| {
        if let Ok(js_str) = e.data().dyn_into::<js_sys::JsString>() {
            let text = String::from(js_str);
            if let Ok(msg) = serde_json::from_str::<ServerMsg>(&text) {
                if let ServerMsg::Welcome { ref token, .. } = msg {
                    save_token(&rid1, token);
                }
                state.update(|s| s.apply(msg));
            }
        }
    });
    ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();

    let onerror = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
        state.update(|s| s.error = Some("Connection error, reconnecting...".into()));
    });
    ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
    onerror.forget();

    let rc2 = room_id.clone();
    let rp2 = passcode.clone();
    let onclose = Closure::<dyn FnMut(web_sys::CloseEvent)>::new(move |_| {
        // Don't reconnect once the connection was rejected (room missing,
        // wrong passcode, full), there's nothing to rejoin.
        let terminal = state.with_untracked(|s| matches!(s.phase, Phase::Closed));
        if terminal {
            clear_token(&rc2);
        } else {
            state.update(|s| {
                s.phase = Phase::Reconnecting;
                s.opponent_disconnected = false;
            });
            schedule_reconnect(rc2.clone(), rp2.clone(), state, next_delay_ms);
        }
    });
    ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
    onclose.forget();

    GAME_WS.with(|cell| *cell.borrow_mut() = Some(ws));
}

#[component]
pub fn GameRoomInner(room_id: String, room_passcode: String) -> impl IntoView {
    let state = RwSignal::new(GameUiState::new(room_id.clone()));
    // Only swap views when the phase actually changes, so local state like the
    // selected piece survives other updates.
    let phase = Memo::new(move |_| state.with(|s| s.phase.clone()));

    #[cfg(target_arch = "wasm32")]
    {
        let rc = room_id.clone();
        let rp = room_passcode.clone();
        Effect::new(move |_| connect_ws(rc.clone(), rp.clone(), state, 1_000));
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = room_passcode;
    }
    #[cfg(target_arch = "wasm32")]
    {
        on_cleanup(move || {
            GAME_WS.with(|cell| {
                if let Some(ws) = cell.borrow().as_ref() {
                    let _ = ws.close();
                }
                *cell.borrow_mut() = None;
            });
        });
    }

    view! {
        <div class="max-w-4xl mx-auto">
            <div class="flex items-center justify-between mb-4">
                <div class="flex items-center gap-2">
                    <h1 class="text-2xl font-bold dark:text-white">"Light Beams"</h1>
                    <RulesButton />
                </div>
                <span class="font-mono text-sm bg-stone-100 dark:bg-stone-700 dark:text-white px-3 py-1 rounded">
                    {move || state.with(|s| s.room_id.clone())}
                </span>
            </div>

            {move || {
                if let Some(err) = state.with(|s| s.error.clone()) {
                    view! {
                        <div class="mb-4 rounded bg-red-100 border border-red-300 text-red-800 px-4 py-2 text-sm">
                            {err}
                        </div>
                    }
                        .into_any()
                } else {
                    view! { <span /> }.into_any()
                }
            }}

            {move || match phase.get() {
                Phase::Connecting => {
                    view! {
                        <div class="flex flex-col items-center justify-center gap-3 py-16 text-stone-500 dark:text-stone-400">
                            <div class="h-8 w-8 animate-spin rounded-full border-4 border-stone-600 border-t-blue-500"></div>
                            <p>"Loading game…"</p>
                            <p class="text-xs text-stone-600">
                                "Setting things up... This can take a moment on the first load."
                            </p>
                        </div>
                    }
                        .into_any()
                }
                Phase::Lobby => view! { <LobbyView state /> }.into_any(),
                Phase::Reconnecting => {
                    view! {
                        <div class="text-center py-16 space-y-3">
                            <p class="text-lg dark:text-white">"Reconnecting..."</p>
                            <p class="text-sm text-stone-500 dark:text-stone-400">
                                "Connection lost. Your game is still saved."
                            </p>
                        </div>
                    }
                        .into_any()
                }
                Phase::Setup => view! { <SetupView state /> }.into_any(),
                Phase::Playing { your_turn } => view! { <PlayView state your_turn /> }.into_any(),
                Phase::Independent { .. } => view! { <IndependentPlayView state /> }.into_any(),
                Phase::GameOver { you_win } => view! { <GameOverView state you_win /> }.into_any(),
                Phase::Closed => {
                    view! {
                        <div class="text-center py-16 space-y-3">
                            <p class="text-lg dark:text-white">"Couldn't join this game."</p>
                            <a
                                href="/games/light-beams"
                                class="inline-block text-sm text-blue-600 dark:text-blue-400 hover:underline"
                            >
                                "Back to the lobby"
                            </a>
                        </div>
                    }
                        .into_any()
                }
            }}
        </div>
    }
}

// ── Waiting room ──────────────────────────────────────────────────────────────

#[component]
fn LobbyView(state: RwSignal<GameUiState>) -> impl IntoView {
    let is_host = move || state.with(|s| s.player == Some(s.host));
    let count = move || state.with(|s| s.players.iter().filter(|p| p.connected).count());
    let my_name = move || {
        state.with(|s| {
            s.players
                .iter()
                .find(|p| Some(p.id) == s.player)
                .map(|p| p.nickname.clone())
                .unwrap_or_default()
        })
    };
    let capacity = move || state.with(|s| s.rules.max_players());

    view! {
        <div class="max-w-md mx-auto space-y-6 py-6">
            <div class="text-center space-y-1">
                <h2 class="text-xl font-bold dark:text-white">"Waiting room"</h2>
                <p class="text-sm text-stone-500 dark:text-stone-400">
                    "Share the room code above to invite players."
                </p>
            </div>

            <div>
                <label class="block text-sm font-semibold mb-1 dark:text-stone-200">
                    "Your nickname"
                </label>
                <input
                    type="text"
                    maxlength="20"
                    placeholder="Pick a nickname"
                    class="w-full rounded border border-stone-300 dark:border-stone-600 bg-white dark:bg-stone-700 px-3 py-2 text-sm dark:text-white"
                    prop:value=my_name
                    on:change=move |ev| {
                        let name = event_target_value(&ev);
                        if !name.trim().is_empty() {
                            send_msg(&ClientMsg::SetNickname { name });
                        }
                    }
                />
            </div>

            <div class="rounded-lg border border-stone-200 dark:border-stone-700 p-4">
                <div class="flex items-center justify-between mb-2">
                    <h3 class="font-semibold dark:text-stone-100">"Players"</h3>
                    <span class="text-xs font-mono text-stone-400">
                        {move || format!("{} / {}", count(), capacity())}
                    </span>
                </div>
                <ul class="space-y-1">
                    {move || {
                        let (players, host, me) = state
                            .with(|s| (s.players.clone(), s.host, s.player));
                        players
                            .into_iter()
                            .map(|p| {
                                let dot = if p.connected { "bg-green-500" } else { "bg-stone-500" };
                                view! {
                                    <li class="flex items-center gap-2 text-sm dark:text-stone-200">
                                        <span class=format!(
                                            "inline-block h-2 w-2 rounded-full {dot}",
                                        )></span>
                                        <span class="truncate">{p.nickname.clone()}</span>
                                        {(p.id == host)
                                            .then(|| {
                                                view! {
                                                    <span class="rounded bg-yellow-500/20 text-yellow-500 text-[10px] px-1.5 py-0.5">
                                                        "host"
                                                    </span>
                                                }
                                            })}
                                        {(Some(p.id) == me)
                                            .then(|| {
                                                view! {
                                                    <span class="rounded bg-blue-500/20 text-blue-400 text-[10px] px-1.5 py-0.5">
                                                        "you"
                                                    </span>
                                                }
                                            })}
                                    </li>
                                }
                            })
                            .collect_view()
                    }}
                </ul>
            </div>

            {move || {
                if is_host() {
                    let ready = count() >= 2;
                    view! {
                        <button
                            type="button"
                            class="w-full rounded bg-blue-500 py-2 font-semibold text-white hover:bg-blue-600 disabled:opacity-50 disabled:cursor-not-allowed touch-manipulation"
                            disabled=!ready
                            on:click=move |_| send_msg(&ClientMsg::StartGame)
                        >
                            {if ready { "Go" } else { "Waiting for more players…" }}
                        </button>
                    }
                        .into_any()
                } else {
                    view! {
                        <p class="text-center text-sm text-stone-500 dark:text-stone-400">
                            "Waiting for the host to start…"
                        </p>
                    }
                        .into_any()
                }
            }}
        </div>
    }
}

// ── Piece visual icon (SVG) ───────────────────────────────────────────────────

/// One arrow pointing up from the centre, rotated by `deg` (0 = up, 90 = right).
fn arrow_head(deg: i32) -> impl IntoView {
    view! {
        <g transform=format!("rotate({deg} 16 16)")>
            <line
                x1="16"
                y1="16"
                x2="16"
                y2="6"
                stroke="#94a3b8"
                stroke-width="3"
                stroke-linecap="round"
            />
            <polyline
                points="12,10 16,5 20,10"
                fill="none"
                stroke="#94a3b8"
                stroke-width="3"
                stroke-linecap="round"
                stroke-linejoin="round"
            />
        </g>
    }
}

fn piece_icon(piece: Piece) -> AnyView {
    match piece {
        Piece::Red => view! {
            <svg viewBox="0 0 32 32" class="w-full h-full" xmlns="http://www.w3.org/2000/svg">
                <circle cx="16" cy="17" r="11" fill="rgba(0,0,0,0.25)" />
                <circle cx="16" cy="16" r="11" fill="#ef4444" />
                <circle cx="12" cy="11" r="4" fill="rgba(255,255,255,0.35)" />
            </svg>
        }
        .into_any(),
        Piece::Yellow => view! {
            <svg viewBox="0 0 32 32" class="w-full h-full" xmlns="http://www.w3.org/2000/svg">
                <circle cx="16" cy="17" r="11" fill="rgba(0,0,0,0.25)" />
                <circle cx="16" cy="16" r="11" fill="#eab308" />
                <circle cx="12" cy="11" r="4" fill="rgba(255,255,255,0.35)" />
            </svg>
        }
        .into_any(),
        Piece::Blue => view! {
            <svg viewBox="0 0 32 32" class="w-full h-full" xmlns="http://www.w3.org/2000/svg">
                <circle cx="16" cy="17" r="11" fill="rgba(0,0,0,0.25)" />
                <circle cx="16" cy="16" r="11" fill="#3b82f6" />
                <circle cx="12" cy="11" r="4" fill="rgba(255,255,255,0.35)" />
            </svg>
        }
        .into_any(),
        Piece::Black => view! {
            <svg viewBox="0 0 32 32" class="w-full h-full" xmlns="http://www.w3.org/2000/svg">
                <circle cx="16" cy="17" r="11" fill="rgba(0,0,0,0.5)" />
                <circle cx="16" cy="16" r="11" fill="#1c1917" />
                <circle cx="12" cy="11" r="3" fill="rgba(255,255,255,0.18)" />
            </svg>
        }
        .into_any(),
        Piece::Slash(SlashDir::Forward) => view! {
            <svg viewBox="0 0 32 32" class="w-full h-full" xmlns="http://www.w3.org/2000/svg">
                <line
                    x1="8"
                    y1="26"
                    x2="24"
                    y2="6"
                    stroke="rgba(0,0,0,0.3)"
                    stroke-width="5"
                    stroke-linecap="round"
                />
                <line
                    x1="8"
                    y1="25"
                    x2="24"
                    y2="5"
                    stroke="#94a3b8"
                    stroke-width="5"
                    stroke-linecap="round"
                />
                <line
                    x1="10"
                    y1="22"
                    x2="22"
                    y2="8"
                    stroke="rgba(255,255,255,0.5)"
                    stroke-width="2"
                    stroke-linecap="round"
                />
            </svg>
        }
        .into_any(),
        Piece::Slash(SlashDir::Backward) => view! {
            <svg viewBox="0 0 32 32" class="w-full h-full" xmlns="http://www.w3.org/2000/svg">
                <line
                    x1="8"
                    y1="6"
                    x2="24"
                    y2="26"
                    stroke="rgba(0,0,0,0.3)"
                    stroke-width="5"
                    stroke-linecap="round"
                />
                <line
                    x1="8"
                    y1="5"
                    x2="24"
                    y2="25"
                    stroke="#94a3b8"
                    stroke-width="5"
                    stroke-linecap="round"
                />
                <line
                    x1="10"
                    y1="8"
                    x2="22"
                    y2="22"
                    stroke="rgba(255,255,255,0.5)"
                    stroke-width="2"
                    stroke-linecap="round"
                />
            </svg>
        }
        .into_any(),
        Piece::Arrow(dir) => {
            // A corner of two arrows, each pointing outward from the centre.
            let (d1, d2): (i32, i32) = match dir {
                ArrowDir::UpRight => (0, 90),
                ArrowDir::DownRight => (180, 90),
                ArrowDir::DownLeft => (180, 270),
                ArrowDir::UpLeft => (0, 270),
            };
            view! {
                <svg viewBox="0 0 32 32" class="w-full h-full" xmlns="http://www.w3.org/2000/svg">
                    {arrow_head(d1)}
                    {arrow_head(d2)}
                </svg>
            }
            .into_any()
        }
        Piece::FourArrow => view! {
            <svg viewBox="0 0 32 32" class="w-full h-full" xmlns="http://www.w3.org/2000/svg">
                {[0, 90, 180, 270].into_iter().map(arrow_head).collect_view()}
            </svg>
        }
        .into_any(),
    }
}

// ── Piece palette ─────────────────────────────────────────────────────────────

/// The piece types a rule set uses, in palette order. Arrows and slashes are
/// one entry each; their orientation is picked separately.
fn palette_pieces(rules: &Rules) -> Vec<Piece> {
    PieceType::ALL
        .into_iter()
        .filter(|ty| rules.counts.get(*ty) > 0)
        .map(PieceType::sample)
        .collect()
}

#[component]
fn PiecePalette(
    selected: RwSignal<Option<Piece>>,
    #[prop(into)] rules: Signal<Rules>,
    /// The big setup palette, rather than the compact guess palette.
    #[prop(optional)]
    large: bool,
) -> impl IntoView {
    let (size, on, off) = if large {
        (
            "w-12 h-12 rounded-xl p-1.5 shadow-sm",
            "border-blue-400 scale-110 ring-2 ring-blue-300 bg-stone-700",
            "border-stone-500 bg-stone-700 hover:bg-stone-600 hover:border-stone-400",
        )
    } else {
        (
            "w-10 h-10 rounded-lg p-1",
            "border-blue-400 scale-110 bg-stone-700",
            "border-stone-600 bg-stone-800 hover:bg-stone-700",
        )
    };
    view! {
        <div class="flex flex-wrap gap-2">
            {move || {
                rules
                    .with(palette_pieces)
                    .into_iter()
                    .map(|p| {
                        let is_sel = move || {
                            selected.with(|s| s.map(|sel| same_piece_type(sel, p)).unwrap_or(false))
                        };
                        view! {
                            <button
                                title=p.label()
                                class=move || {
                                    format!(
                                        "border-2 touch-manipulation transition-all {size} {}",
                                        if is_sel() { on } else { off },
                                    )
                                }
                                on:click=move |_| {
                                    selected
                                        .update(|s| {
                                            *s = if s
                                                .map(|sel| same_piece_type(sel, p))
                                                .unwrap_or(false)
                                            {
                                                None
                                            } else {
                                                Some(p)
                                            };
                                        });
                                }
                            >
                                {piece_icon(p)}
                            </button>
                        }
                    })
                    .collect_view()
            }}
        </div>
    }
}

/// Lets the user pick a mirror's orientation when a mirror tool is selected.
/// Selecting an orientation stores that exact piece, so board clicks place it
/// directly (no click-to-cycle).
#[component]
fn OrientationPicker(selected: RwSignal<Option<Piece>>) -> impl IntoView {
    let button = move |piece: Piece, active: bool| {
        view! {
            <button
                title=piece.label()
                class=format!(
                    "w-10 h-10 rounded-lg p-1 border-2 touch-manipulation transition-all {}",
                    if active {
                        "border-blue-400 scale-110 bg-stone-700"
                    } else {
                        "border-stone-600 bg-stone-800 hover:bg-stone-700"
                    },
                )
                on:click=move |_| selected.set(Some(piece))
            >
                {piece_icon(piece)}
            </button>
        }
    };
    move || {
        let options: Vec<Piece> = match selected.get() {
            Some(Piece::Slash(_)) => {
                vec![
                    Piece::Slash(SlashDir::Forward),
                    Piece::Slash(SlashDir::Backward),
                ]
            }
            Some(Piece::Arrow(_)) => vec![
                Piece::Arrow(ArrowDir::UpRight),
                Piece::Arrow(ArrowDir::DownRight),
                Piece::Arrow(ArrowDir::DownLeft),
                Piece::Arrow(ArrowDir::UpLeft),
            ],
            _ => return view! { <span /> }.into_any(),
        };
        let current = selected.get();
        view! {
            <div class="flex gap-2 items-center">
                <span class="text-xs text-stone-500 mr-1">"Orientation:"</span>
                {options.into_iter().map(|p| button(p, current == Some(p))).collect_view()}
            </div>
        }
        .into_any()
    }
}

/// "Place exactly: 1× Red, 1× Yellow, ..." for the pieces a rule set needs.
fn piece_requirement_text(counts: &PieceCounts) -> String {
    let parts: Vec<String> = PieceType::ALL
        .into_iter()
        .filter(|ty| counts.get(*ty) > 0)
        .map(|ty| format!("{}× {}", counts.get(ty), ty.sample().label()))
        .collect();
    format!("Place exactly: {}.", parts.join(", "))
}

/// One chip per required piece type showing how many are on the board.
#[component]
fn PieceChecklist(
    #[prop(into)] board: Signal<Board>,
    #[prop(into)] rules: Signal<Rules>,
) -> impl IntoView {
    view! {
        <div class="space-y-2">
            <p class="text-xs font-semibold uppercase tracking-wide text-stone-500">
                "Place exactly:"
            </p>
            <div class="flex flex-wrap gap-2">
                {move || {
                    let need = rules.with(|r| r.counts);
                    let have = board.with(board_counts);
                    PieceType::ALL
                        .into_iter()
                        .filter(|ty| need.get(*ty) > 0)
                        .map(|ty| {
                            let (n, want) = (have.get(ty), need.get(ty));
                            let tone = if n == want {
                                "border-green-600 bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-300"
                            } else if n > want {
                                "border-red-500 bg-red-100 text-red-800 dark:border-red-600 dark:bg-red-900/30 dark:text-red-300"
                            } else {
                                "border-stone-300 bg-stone-100 text-stone-700 dark:border-stone-600 dark:bg-stone-800 dark:text-stone-300"
                            };
                            view! {
                                <div class=format!(
                                    "flex items-center gap-1.5 rounded-lg border px-2 py-1 {tone}",
                                )>
                                    <div class="h-6 w-6 shrink-0 rounded bg-stone-700 p-0.5">
                                        {piece_icon(ty.sample())}
                                    </div>
                                    <span class="font-mono text-xs">{format!("{n}/{want}")}</span>
                                </div>
                            }
                        })
                        .collect_view()
                }}
            </div>
        </div>
    }
}

// ── Setup view ────────────────────────────────────────────────────────────────

#[component]
fn SetupView(state: RwSignal<GameUiState>) -> impl IntoView {
    let selected = RwSignal::new(Option::<Piece>::None);
    let locked = RwSignal::new(false);
    let rules = Memo::new(move |_| state.with(|s| s.rules));
    let board = Signal::derive(move || state.with(|s| s.your_board.clone()));

    let on_lock = move |_| {
        let (board, rules) = state.with_untracked(|s| (s.your_board.clone(), s.rules));
        if !logic::validate_board(&board, &rules) {
            state.update(|s| s.error = Some(piece_requirement_text(&rules.counts)));
            return;
        }
        locked.set(true);
        send_msg(&ClientMsg::LockBoard { board });
    };

    view! {
        <div class="space-y-5">
            <p class="text-stone-400 text-sm">
                "Pick a piece, then place it on the board. For a mirror, choose its orientation first. Click a placed piece to remove it."
            </p>

            <PieceChecklist board rules />
            <PiecePalette selected rules large=true />
            <OrientationPicker selected />

            <div class="overflow-x-auto">
                <BoardGrid
                    board
                    on_cell=move |r, c| {
                        if locked.get_untracked() {
                            return;
                        }
                        let sel = selected.get_untracked();
                        state.update(|s| s.your_board.toggle(r, c, sel));
                    }
                    interactive=true
                />
            </div>

            <div class="flex items-center gap-4">
                <button
                    class="min-h-[48px] rounded-xl bg-blue-500 px-6 py-2 text-base font-semibold text-white hover:bg-blue-600 disabled:opacity-40 touch-manipulation shadow-md"
                    on:click=on_lock
                    disabled=move || locked.get()
                >
                    {move || if locked.get() { "Board Locked ✓" } else { "Lock Board" }}
                </button>
                {move || {
                    if state.with(|s| s.opponent_locked) {
                        view! { <span class="text-sm text-green-400">"Opponent is ready!"</span> }
                            .into_any()
                    } else {
                        view! {
                            <span class="text-sm text-stone-500">"Waiting for opponent..."</span>
                        }
                            .into_any()
                    }
                }}
            </div>
        </div>
    }
}

// ── Shared play controls ──────────────────────────────────────────────────────

fn call_button(
    can_call: impl Fn() -> bool + Copy + Send + Sync + 'static,
    on_call: impl Fn() + Copy + Send + Sync + 'static,
) -> impl IntoView {
    view! {
        <button
            class=move || {
                format!(
                    "min-h-[44px] rounded-xl px-5 py-2 text-base font-semibold touch-manipulation shadow-md {}",
                    if can_call() {
                        "bg-yellow-500 text-black hover:bg-yellow-400"
                    } else {
                        "bg-stone-700 text-stone-500 cursor-not-allowed"
                    },
                )
            }
            on:click=move |_| {
                if can_call() {
                    on_call();
                }
            }
            disabled=move || !can_call()
        >
            "Call Game!"
        </button>
    }
}

fn flip_button(
    can_flip: impl Fn() -> bool + Copy + Send + Sync + 'static,
    rotate_mode: RwSignal<bool>,
    label: impl Fn() -> &'static str + Send + Sync + 'static,
) -> impl IntoView {
    view! {
        <button
            class=move || {
                format!(
                    "min-h-[44px] rounded-xl px-3 py-2 text-sm touch-manipulation shadow {}",
                    if !can_flip() {
                        "bg-stone-800 text-stone-600 cursor-not-allowed"
                    } else if rotate_mode.get() {
                        "bg-orange-500 text-white"
                    } else {
                        "bg-stone-700 text-stone-300 hover:bg-stone-600"
                    },
                )
            }
            disabled=move || !can_flip()
            on:click=move |_| {
                if can_flip() {
                    rotate_mode.update(|m| *m = !*m)
                }
            }
        >
            {label}
        </button>
    }
}

/// The guess board with its ports, plus the palette for building a guess.
#[component]
fn GuessPanel(
    #[prop(into)] board: Signal<Board>,
    #[prop(into)] rules: Signal<Rules>,
    selected: RwSignal<Option<Piece>>,
    on_port: impl Fn(u8) + Copy + Send + Sync + 'static,
    #[prop(into)] port_active: Signal<bool>,
    on_cell: impl Fn(usize, usize) + Copy + Send + Sync + 'static,
    #[prop(into)] highlight: Signal<Option<(u8, u8, Color)>>,
) -> impl IntoView {
    view! {
        <BoardWithPorts board on_port port_active on_cell cell_interactive=true highlight />
        <div class="mt-4 space-y-3">
            <PiecePalette selected rules />
            <OrientationPicker selected />
        </div>
    }
}

#[component]
fn OpponentDisconnectedBanner(state: RwSignal<GameUiState>) -> impl IntoView {
    move || {
        if state.with(|s| s.opponent_disconnected) {
            view! {
                <div class="rounded-xl border border-yellow-700 bg-yellow-900/30 px-4 py-2 text-sm text-yellow-300">
                    "Opponent disconnected! Waiting for them to reconnect..."
                </div>
            }
                .into_any()
        } else {
            view! { <span /> }.into_any()
        }
    }
}

/// The last beam fired by you (`mine`) or by the other side.
fn last_shot(state: RwSignal<GameUiState>, mine: bool) -> Signal<Option<(u8, u8, Color)>> {
    Signal::derive(move || {
        state.with(|s| {
            s.shot_history
                .iter()
                .rev()
                .find(|sh| sh.mine == mine)
                .map(|sh| (sh.call, sh.exit, sh.color))
        })
    })
}

/// Place or remove the selected piece on the guess board, or attempt a flip
/// when flip mode is armed.
fn guess_cell_handler(
    state: RwSignal<GameUiState>,
    selected: RwSignal<Option<Piece>>,
    rotate_mode: RwSignal<bool>,
    can_flip: impl Fn() -> bool + Copy + Send + Sync + 'static,
) -> impl Fn(usize, usize) + Copy + Send + Sync + 'static {
    move |r: usize, c: usize| {
        if rotate_mode.get_untracked() {
            if can_flip() {
                send_msg(&ClientMsg::RequestRotate {
                    row: r as u8,
                    col: c as u8,
                });
            }
            rotate_mode.set(false);
        } else {
            let sel = selected.get_untracked();
            state.update(|s| s.guess_board.toggle(r, c, sel));
        }
    }
}

// ── Play view ─────────────────────────────────────────────────────────────────

#[component]
fn PlayView(state: RwSignal<GameUiState>, your_turn: bool) -> impl IntoView {
    let guess_selected = RwSignal::new(Option::<Piece>::None);
    let rotate_mode = RwSignal::new(false);
    let rules = Memo::new(move |_| state.with(|s| s.rules));
    let size = move || state.with(|s| s.rules.size);

    // You may flip exactly one mirror, only on your own turn.
    let can_flip = move || your_turn && !state.with(|s| s.flip_used_this_turn);
    // You may only call the game on your own turn.
    let can_call =
        move || your_turn && state.with(|s| logic::validate_board(&s.guess_board, &s.rules));
    let on_call = move || {
        let guess = state.with_untracked(|s| s.guess_board.clone());
        send_msg(&ClientMsg::CallGame { guess });
    };

    view! {
        <div class="space-y-6">
            <OpponentDisconnectedBanner state />

            <div class="grid grid-cols-1 lg:grid-cols-2 gap-10">
                <div>
                    <h3 class="font-semibold mb-2 text-stone-300">"Your Board"</h3>
                    <BoardWithPorts
                        board=Signal::derive(move || state.with(|s| s.your_board.clone()))
                        on_port=|_| {}
                        port_active=false
                        on_cell=|_, _| {}
                        cell_interactive=false
                        highlight=last_shot(state, false)
                    />
                    <div class=if your_turn {
                        "mt-3 text-base font-semibold text-green-400"
                    } else {
                        "mt-3 text-base font-semibold text-stone-500"
                    }>
                        {if your_turn { "Your turn. Shoot a beam!" } else { "Opponent's turn..." }}
                    </div>
                    {move || {
                        if let Some((row, col, accepted)) = state.with(|s| s.last_mirror_attempt) {
                            let (top, left) = cell_label(row, col, size());
                            let msg = if accepted {
                                format!(
                                    "Opponent found your mirror at ({top}, {left}) and flipped it!",
                                )
                            } else {
                                format!("Opponent tried to flip ({top}, {left}). No mirror there.")
                            };
                            let cls = if accepted {
                                "mt-2 text-sm text-orange-400"
                            } else {
                                "mt-2 text-sm text-stone-500"
                            };
                            view! { <p class=cls>{msg}</p> }.into_any()
                        } else {
                            view! { <span /> }.into_any()
                        }
                    }}
                </div>

                <div>
                    <h3 class="font-semibold mb-2 text-stone-300">"Guess Board"</h3>
                    <p class="text-xs text-stone-500 mb-2">
                        {if your_turn {
                            "Click a port to shoot a beam."
                        } else {
                            "Build your guess while opponent shoots."
                        }}
                    </p>
                    <GuessPanel
                        board=Signal::derive(move || state.with(|s| s.guess_board.clone()))
                        rules
                        selected=guess_selected
                        on_port=move |port| send_msg(&ClientMsg::ShootBeam { port })
                        port_active=your_turn
                        on_cell=guess_cell_handler(state, guess_selected, rotate_mode, can_flip)
                        highlight=last_shot(state, true)
                    />
                    <div class="mt-4 flex flex-wrap gap-2 items-center">
                        {call_button(can_call, on_call)}
                        {flip_button(
                            can_flip,
                            rotate_mode,
                            move || {
                                if state.with(|s| s.flip_used_this_turn) {
                                    "Flip used this turn"
                                } else if !your_turn {
                                    "Flip mirror (your turn only)"
                                } else if rotate_mode.get() {
                                    "Tap a cell to flip it"
                                } else {
                                    "Flip a mirror"
                                }
                            },
                        )}
                    </div>
                    {move || {
                        if state.with(|s| s.last_guess_wrong) {
                            view! {
                                <p class="mt-3 text-sm text-red-400">
                                    "Wrong guess, game continues."
                                </p>
                            }
                                .into_any()
                        } else {
                            view! { <span /> }.into_any()
                        }
                    }}
                    {move || {
                        if let Some((row, col, accepted)) = state.with(|s| s.last_rotate_result) {
                            let (top, left) = cell_label(row, col, size());
                            let msg = if accepted {
                                format!(
                                    "Correct! You found a mirror at ({top}, {left}) and flipped it.",
                                )
                            } else {
                                format!("No mirror at ({top}, {left}). You lose your turn.")
                            };
                            let cls = if accepted {
                                "mt-2 text-sm text-green-400"
                            } else {
                                "mt-2 text-sm text-stone-500"
                            };
                            view! { <p class=cls>{msg}</p> }.into_any()
                        } else {
                            view! { <span /> }.into_any()
                        }
                    }}
                </div>
            </div>

            <ShotHistory state />
        </div>
    }
}

// ── Independent play (async / computer board) ─────────────────────────────────

/// Who has solved their board so far.
#[component]
fn SolveProgress(state: RwSignal<GameUiState>) -> impl IntoView {
    view! {
        <div class="flex flex-wrap items-center justify-center gap-2">
            {move || {
                let (players, done, me) = state
                    .with(|s| (s.players.clone(), s.done.clone(), s.player));
                players
                    .into_iter()
                    .map(|p| {
                        let finished = done.get(p.id as usize).copied().unwrap_or(false);
                        let you = if Some(p.id) == me { " (you)" } else { "" };
                        let check = if finished { " ✓" } else { "" };
                        let cls = if finished {
                            "rounded-full px-2.5 py-0.5 text-xs bg-green-600/20 text-green-400"
                        } else {
                            "rounded-full px-2.5 py-0.5 text-xs bg-stone-700 text-stone-300"
                        };
                        view! { <span class=cls>{format!("{}{you}{check}", p.nickname)}</span> }
                    })
                    .collect_view()
            }}
        </div>
    }
}

fn beams_text(n: usize) -> String {
    if n == 1 {
        "1 beam".to_string()
    } else {
        format!("{n} beams")
    }
}

#[component]
fn IndependentPlayView(state: RwSignal<GameUiState>) -> impl IntoView {
    let guess_selected = RwSignal::new(Option::<Piece>::None);
    let rotate_mode = RwSignal::new(false);
    let rules = Memo::new(move |_| state.with(|s| s.rules));
    let size = move || state.with(|s| s.rules.size);
    let done = move || state.with(|s| s.you_done);

    let can_flip = move || !done() && !state.with(|s| s.flip_used_this_turn);
    let can_call =
        move || !done() && state.with(|s| logic::validate_board(&s.guess_board, &s.rules));
    let on_call = move || {
        let guess = state.with_untracked(|s| s.guess_board.clone());
        send_msg(&ClientMsg::CallGame { guess });
    };
    let beams = move || state.with(|s| s.shot_history.iter().filter(|sh| sh.mine).count());

    view! {
        <div class="space-y-6">
            <OpponentDisconnectedBanner state />
            <SolveProgress state />
            {move || {
                let (you_done, all_done) = state
                    .with(|s| (s.you_done, !s.done.is_empty() && s.done.iter().all(|d| *d)));
                if you_done {
                    view! {
                        <div class="rounded-xl border border-green-300 bg-green-100 text-green-800 px-4 py-3 font-semibold dark:border-green-700 dark:bg-green-900/30 dark:text-green-300">
                            {if all_done {
                                "Everyone finished — comparing scores..."
                            } else {
                                "Solved! Waiting for the others to finish..."
                            }}
                        </div>
                    }
                        .into_any()
                } else {
                    view! { <span /> }.into_any()
                }
            }}

            <div class="max-w-md mx-auto">
                <div class="flex items-center justify-between mb-2">
                    <h3 class="font-semibold text-stone-300">"Guess Board"</h3>
                    <span class="text-xs font-mono text-stone-400">
                        {move || format!("{} beams", beams())}
                    </span>
                </div>
                <p class="text-xs text-stone-500 mb-2">
                    "Shoot beams through the ports, deduce the hidden board, then call it."
                </p>
                <GuessPanel
                    board=Signal::derive(move || state.with(|s| s.guess_board.clone()))
                    rules
                    selected=guess_selected
                    on_port=move |port| {
                        if !done() {
                            rotate_mode.set(false);
                            send_msg(&ClientMsg::ShootBeam { port });
                        }
                    }
                    port_active=Signal::derive(move || !done())
                    on_cell=guess_cell_handler(state, guess_selected, rotate_mode, can_flip)
                    highlight=last_shot(state, true)
                />
                <div class="mt-4 flex flex-wrap gap-2 items-center">
                    {call_button(can_call, on_call)}
                    {flip_button(
                        can_flip,
                        rotate_mode,
                        move || {
                            if state.with(|s| s.flip_used_this_turn) {
                                "Flip used (shoot to refresh)"
                            } else if rotate_mode.get() {
                                "Tap a cell to flip it"
                            } else {
                                "Flip a mirror"
                            }
                        },
                    )}
                </div>
                {move || {
                    if state.with(|s| s.last_guess_wrong) {
                        view! {
                            <p class="mt-3 text-sm text-red-400">
                                "Wrong guess — that costs a beam. Keep shooting."
                            </p>
                        }
                            .into_any()
                    } else {
                        view! { <span /> }.into_any()
                    }
                }}
                {move || {
                    if let Some((row, col, accepted)) = state.with(|s| s.last_rotate_result) {
                        let (top, left) = cell_label(row, col, size());
                        let (msg, cls) = if accepted {
                            (
                                format!(
                                    "Correct! You found a mirror at ({top}, {left}) and flipped it.",
                                ),
                                "mt-2 text-sm text-green-400",
                            )
                        } else {
                            (
                                format!("No mirror at ({top}, {left}). That costs a beam."),
                                "mt-2 text-sm text-stone-500",
                            )
                        };
                        view! { <p class=cls>{msg}</p> }.into_any()
                    } else {
                        view! { <span /> }.into_any()
                    }
                }}
            </div>

            <ShotHistory state />
        </div>
    }
}

// ── Game over view ────────────────────────────────────────────────────────────

fn score_text(score: Option<u32>, timed: bool) -> String {
    match score {
        None => "—".to_string(),
        Some(s) if timed => format!("{s}s"),
        Some(s) => format!("{s} beams"),
    }
}

/// Final standings for an independent game.
#[component]
fn Scoreboard(state: RwSignal<GameUiState>) -> impl IntoView {
    let (results, me, timed) =
        state.with_untracked(|s| (s.results.clone(), s.player, s.rules.timed));
    let mut results = results;
    results.sort_by_key(|r| r.place);
    view! {
        <div class="max-w-xs mx-auto rounded-lg border border-stone-200 dark:border-stone-700 overflow-hidden">
            {results
                .into_iter()
                .map(|r| {
                    let mine = Some(r.id) == me;
                    view! {
                        <div class=if mine {
                            "flex items-center justify-between px-3 py-2 text-sm bg-blue-500/10"
                        } else {
                            "flex items-center justify-between px-3 py-2 text-sm"
                        }>
                            <div class="flex items-center gap-2 min-w-0">
                                <span class="font-mono text-xs text-stone-400 w-5">
                                    {format!("#{}", r.place)}
                                </span>
                                <span class="truncate dark:text-stone-100">
                                    {r.nickname.clone()}
                                </span>
                                {mine
                                    .then(|| {
                                        view! {
                                            <span class="rounded bg-blue-500/20 text-blue-400 text-[10px] px-1.5 py-0.5">
                                                "you"
                                            </span>
                                        }
                                    })}
                            </div>
                            <span class="font-mono text-xs text-stone-400">
                                {score_text(r.score, timed)}
                            </span>
                        </div>
                    }
                })
                .collect_view()}
        </div>
    }
}

#[component]
fn GameOverView(state: RwSignal<GameUiState>, you_win: bool) -> impl IntoView {
    let your_board = Signal::derive(move || state.with(|s| s.your_board.clone()));
    let guess_board = Signal::derive(move || state.with(|s| s.guess_board.clone()));
    let answer_board = Signal::derive(move || {
        state.with(|s| {
            s.opponent_board
                .clone()
                .unwrap_or_else(|| Board::new(s.rules.size))
        })
    });
    let independent = state.with_untracked(|s| !s.rules.turn_based());
    // A computer-board race names whoever called it first.
    let race_winner = state.with_untracked(|s| {
        let race = matches!(s.rules.source, BoardSource::Computer)
            && matches!(s.rules.resolution, Resolution::Live);
        if !race {
            return None;
        }
        let w = s.winner?;
        s.players
            .iter()
            .find(|p| p.id == w)
            .map(|p| p.nickname.clone())
    });
    let alone = move || {
        state.with(|s| {
            s.players.len() > 1
                && s.players
                    .iter()
                    .all(|p| Some(p.id) == s.player || !p.connected)
        })
    };

    view! {
        <div class="space-y-8">
            <div class="text-center space-y-3">
                <div class=if you_win {
                    "text-4xl font-bold text-green-500"
                } else {
                    "text-4xl font-bold text-red-500"
                }>{if you_win { "You Win!" } else { "You Lose." }}</div>
                {match race_winner {
                    Some(name) => {
                        view! {
                            <p class="text-lg text-yellow-400 font-semibold">
                                {format!("{name} wins the race!")}
                            </p>
                        }
                            .into_any()
                    }
                    None => view! { <span /> }.into_any(),
                }}
                {if independent {
                    view! { <Scoreboard state /> }.into_any()
                } else {
                    view! { <span /> }.into_any()
                }}
                {move || {
                    if alone() {
                        view! {
                            <div class="space-y-2">
                                <p class="text-sm text-stone-400">"Everyone else left the room."</p>
                                <a
                                    href="/games/light-beams"
                                    class="inline-block rounded bg-blue-500 px-6 py-2 text-white hover:bg-blue-600"
                                >
                                    "Back to Lobby"
                                </a>
                            </div>
                        }
                            .into_any()
                    } else {
                        view! {
                            <div class="flex items-center justify-center gap-4">
                                <button
                                    type="button"
                                    class="inline-block rounded bg-blue-500 px-6 py-2 text-white hover:bg-blue-600"
                                    on:click=move |_| send_msg(&ClientMsg::PlayAgain)
                                >
                                    "Play Again"
                                </button>
                                <a
                                    href="/games/light-beams"
                                    class="text-sm text-blue-400 hover:underline"
                                >
                                    "Back to Lobby"
                                </a>
                            </div>
                        }
                            .into_any()
                    }
                }}
            </div>

            <div class="grid grid-cols-1 lg:grid-cols-3 gap-8 justify-items-center">
                <div>
                    <h3 class="font-semibold mb-2 text-stone-300">"Your Guess"</h3>
                    <BoardGrid board=guess_board on_cell=|_, _| {} interactive=false />
                </div>
                <div>
                    <h3 class="font-semibold mb-2 text-green-400">"Correct Answer"</h3>
                    <BoardGrid board=answer_board on_cell=|_, _| {} interactive=false />
                    <p class="text-xs text-stone-500 mt-2">"The opponent's actual board."</p>
                </div>
                <div>
                    <h3 class="font-semibold mb-2 text-stone-300">"Your Board"</h3>
                    <BoardGrid board=your_board on_cell=|_, _| {} interactive=false />
                </div>
            </div>

            <ShotHistory state />
        </div>
    }
}

// ── Shot history ──────────────────────────────────────────────────────────────

/// One row of a history table: the In and Out cells and the result badge.
#[derive(Clone, Debug)]
struct HistoryRow {
    input: String,
    output: String,
    result: String,
    badge: &'static str,
}

const NEUTRAL_BADGE: &str = "bg-stone-200 text-stone-700 dark:bg-stone-700 dark:text-stone-300";

fn shot_row(shot: &Shot) -> HistoryRow {
    if shot.exit == 0 {
        // A trapped beam never leaves the board.
        return HistoryRow {
            input: shot.call.to_string(),
            output: "∞".to_string(),
            result: "Loop ∞".to_string(),
            badge: NEUTRAL_BADGE,
        };
    }
    HistoryRow {
        input: shot.call.to_string(),
        output: shot.exit.to_string(),
        result: shot.color.name().to_string(),
        badge: shot.color.css_class(),
    }
}

fn solo_row(event: &SoloEvent, size: u8) -> HistoryRow {
    match event {
        SoloEvent::Beam(shot) => shot_row(shot),
        SoloEvent::WrongGuess => HistoryRow {
            input: "Guess".to_string(),
            output: "—".to_string(),
            result: "Wrong".to_string(),
            badge: NEUTRAL_BADGE,
        },
        SoloEvent::WastedFlip { row, col } => {
            let (top, left) = cell_label(*row, *col, size);
            HistoryRow {
                input: format!("Flip ({top}, {left})"),
                output: "—".to_string(),
                result: "No mirror".to_string(),
                badge: NEUTRAL_BADGE,
            }
        }
    }
}

#[component]
fn ShotHistory(state: RwSignal<GameUiState>) -> impl IntoView {
    let shots = move |mine: bool| {
        Signal::derive(move || {
            state.with(|s| {
                s.shot_history
                    .iter()
                    .filter(|sh| sh.mine == mine)
                    .map(shot_row)
                    .collect::<Vec<_>>()
            })
        })
    };
    view! {
        <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
            <HistoryTable rows=shots(true) title="Your Shots" last="Color" empty="No shots yet." />
            <HistoryTable
                rows=shots(false)
                title="Opponent's Shots"
                last="Color"
                empty="No shots yet."
            />
        </div>
    }
}

#[component]
fn HistoryTable(
    rows: Signal<Vec<HistoryRow>>,
    title: &'static str,
    /// Heading of the result column.
    last: &'static str,
    empty: &'static str,
) -> impl IntoView {
    let has_rows = Signal::derive(move || rows.with(|r| !r.is_empty()));
    // Newest first, but keyed by each row's stable original index so only the
    // freshly added row mounts (and animates), the existing rows stay put.
    let indexed = Signal::derive(move || {
        let mut v: Vec<(usize, HistoryRow)> =
            rows.with(|r| r.iter().cloned().enumerate().collect());
        v.reverse();
        v
    });
    view! {
        <div>
            <h3 class="font-semibold mb-2 text-stone-300">{title}</h3>
            <Show
                when=move || has_rows.get()
                fallback=move || view! { <p class="text-xs text-stone-500">{empty}</p> }
            >
                <div class="overflow-x-auto rounded-xl bg-white ring-1 ring-stone-200 dark:bg-stone-800 dark:ring-stone-700">
                    <table class="game-table w-full text-sm border-collapse">
                        <thead>
                            <tr class="text-left text-stone-500 text-xs uppercase border-b border-stone-200 dark:border-stone-700">
                                <th class="px-4 py-2">"In"</th>
                                <th class="px-4 py-2">"Out"</th>
                                <th class="px-4 py-2">{last}</th>
                            </tr>
                        </thead>
                        <tbody>
                            <For each=move || indexed.get() key=|(idx, _)| *idx let:row>
                                {
                                    let r = row.1;
                                    view! {
                                        <tr class="row-appear border-t border-stone-200 dark:border-stone-700">
                                            <td class="px-4 py-2 font-mono text-stone-700 dark:text-stone-300">
                                                {r.input}
                                            </td>
                                            <td class="px-4 py-2 font-mono text-stone-700 dark:text-stone-300">
                                                {r.output}
                                            </td>
                                            <td class="px-4 py-2">
                                                <span class=format!(
                                                    "inline-block px-2 py-0.5 rounded text-xs font-semibold {}",
                                                    r.badge,
                                                )>{r.result}</span>
                                            </td>
                                        </tr>
                                    }
                                }
                            </For>
                        </tbody>
                    </table>
                </div>
            </Show>
        </div>
    }
}

// ── Single player mode ────────────────────────────────────────────────────────

// Single player mode runs entirely in the browser: the computer rolls a random
// board and the player deduces it by shooting beams, with no server or opponent
// involved.

/// Read a shared board from the `?b=` query param, if the URL carries a valid one.
#[cfg(target_arch = "wasm32")]
fn board_from_url() -> Option<Board> {
    let search = web_sys::window()?.location().search().ok()?;
    let code = search.strip_prefix('?')?.split('&').find_map(|part| {
        let (k, v) = part.split_once('=')?;
        (k == "b").then_some(v)
    })?;
    logic::decode_board(code)
}
#[cfg(not(target_arch = "wasm32"))]
fn board_from_url() -> Option<Board> {
    None
}

#[cfg(target_arch = "wasm32")]
fn now_ms() -> f64 {
    js_sys::Date::now()
}
#[cfg(not(target_arch = "wasm32"))]
fn now_ms() -> f64 {
    0.0
}

/// Trace a beam through `board` from `port`, as a shot you fired.
fn shoot_beam(board: &Board, port: u8) -> Shot {
    let (exit, color) = trace_beam(board, port);
    Shot {
        call: port,
        exit,
        color,
        mine: true,
    }
}

#[component]
pub fn SinglePlayerInner() -> impl IntoView {
    let rules = RwSignal::new(Rules::default());
    let answer = RwSignal::new(logic::random_board(&rules.get_untracked()));
    let guess_board = RwSignal::new(Board::new(rules.get_untracked().size));
    // One ordered history of everything that costs a beam: real shots, wrong
    // guesses, and wasted flips. The score is simply this log's length.
    let log = RwSignal::new(Vec::<SoloEvent>::new());
    let guess_selected = RwSignal::new(Option::<Piece>::None);
    // The round ends when the player either solves it or reveals the answer.
    let solved = RwSignal::new(false);
    let revealed = RwSignal::new(false);
    let last_guess_wrong = RwSignal::new(false);
    // Flip mechanic, mirroring multiplayer: you may flip one mirror per turn, and
    // shooting a beam ends the turn (refreshing the flip). `rotate_mode` arms the
    // next cell tap to attempt a flip instead of placing a guess piece.
    let rotate_mode = RwSignal::new(false);
    let flip_used = RwSignal::new(false);
    let last_flip = RwSignal::new(Option::<(u8, u8, bool)>::None);
    // Timed games score by how long the solve took.
    let started = RwSignal::new(now_ms());
    let finished = RwSignal::new(Option::<f64>::None);
    let now = RwSignal::new(now_ms());

    let over = move || solved.get() || revealed.get();
    let can_flip = move || !over() && !flip_used.get();
    let size = move || rules.with(|r| r.size);
    let elapsed = move || {
        let end = finished.get().unwrap_or_else(|| now.get());
        ((end - started.get()) / 1000.0).max(0.0) as u64
    };

    #[cfg(target_arch = "wasm32")]
    {
        if let Ok(handle) = set_interval_with_handle(
            move || now.set(now_ms()),
            std::time::Duration::from_millis(500),
        ) {
            on_cleanup(move || handle.clear());
        }
    }

    let finish = move || finished.set(Some(now_ms()));

    let on_shoot = move |port: u8| {
        if over() {
            return;
        }
        last_guess_wrong.set(false);
        last_flip.set(None);
        rotate_mode.set(false);
        let (exit, color) = trace_beam(&answer.get_untracked(), port);
        log.update(|v| {
            v.push(SoloEvent::Beam(Shot {
                call: port,
                exit,
                color,
                mine: true,
            }))
        });
        // Shooting ends the turn, a fresh flip is available next turn.
        flip_used.set(false);
    };

    // When flip mode is armed, a cell tap attempts to rotate the hidden mirror at
    // that cell; otherwise it places/removes a guess piece.
    let on_guess_cell = move |r: usize, c: usize| {
        if over() {
            return;
        }
        if rotate_mode.get_untracked() {
            if can_flip() {
                let mut accepted = false;
                answer.update(|b| {
                    if let Some(p) = b.get(r, c) {
                        if logic::is_slash(p) {
                            b.set(r, c, Some(rotate_piece(p)));
                            accepted = true;
                        }
                    }
                });
                flip_used.set(true);
                // A correct flip turns the real mirror, so flip the guess too.
                if accepted {
                    guess_board.update(|b| {
                        if let Some(p) = b.get(r, c).filter(|p| logic::is_slash(*p)) {
                            b.set(r, c, Some(rotate_piece(p)));
                        }
                    });
                }
                // Flipping a cell with no mirror costs a beam, log it.
                if !accepted {
                    log.update(|v| {
                        v.push(SoloEvent::WastedFlip {
                            row: r as u8,
                            col: c as u8,
                        })
                    });
                }
                last_flip.set(Some((r as u8, c as u8, accepted)));
                last_guess_wrong.set(false);
            }
            rotate_mode.set(false);
        } else {
            let sel = guess_selected.get_untracked();
            guess_board.update(|b| b.toggle(r, c, sel));
        }
    };

    let can_call = move || {
        let r = rules.get();
        guess_board.with(|b| logic::validate_board(b, &r))
    };

    let on_call = move || {
        if logic::boards_match(&guess_board.get_untracked(), &answer.get_untracked()) {
            finish();
            solved.set(true);
        } else {
            last_guess_wrong.set(true);
            // A wrong guess counts as a beam shot, log it.
            log.update(|v| v.push(SoloEvent::WrongGuess));
        }
    };

    let start_round = move |board: Board| {
        guess_board.set(Board::new(board.size));
        answer.set(board);
        log.set(Vec::new());
        guess_selected.set(None);
        solved.set(false);
        revealed.set(false);
        last_guess_wrong.set(false);
        rotate_mode.set(false);
        flip_used.set(false);
        last_flip.set(None);
        started.set(now_ms());
        finished.set(None);
    };
    let new_round = move || start_round(logic::random_board(&rules.get_untracked()));

    // A different board size or piece mix needs a fresh board.
    let dims = Memo::new(move |_| rules.with(|r| (r.size, r.counts)));
    let keep_board = StoredValue::new(false);
    Effect::watch(
        move || dims.get(),
        move |_, _, _| {
            if keep_board.get_value() {
                keep_board.set_value(false);
            } else {
                new_round();
            }
        },
        false,
    );

    // A shared link drops you onto its exact board. It's read once mounted so
    // the server-rendered page and the hydrated one agree.
    Effect::new(move |_| {
        if let Some(b) = board_from_url() {
            let shared = Rules {
                size: b.size,
                counts: board_counts(&b),
                ..rules.get_untracked()
            };
            keep_board.set_value(dims.get_untracked() != (shared.size, shared.counts));
            rules.set(shared);
            start_round(b);
        }
    });

    let on_new_board = move |_| {
        // Drop any ?b= share code so a fresh board isn't replaced on reload.
        #[cfg(target_arch = "wasm32")]
        if let Some(history) = web_sys::window().and_then(|w| w.history().ok()) {
            let _ = history.replace_state_with_url(
                &wasm_bindgen::JsValue::NULL,
                "",
                Some("/games/light-beams/single-player"),
            );
        }
        new_round();
    };

    let score = move || {
        if rules.with(|r| r.timed) {
            format!("{}s", elapsed())
        } else {
            beams_text(log.with(|v| v.len()))
        }
    };

    let rows = Signal::derive(move || {
        let sz = size();
        let v: Vec<HistoryRow> = log.with(|l| l.iter().map(|e| solo_row(e, sz)).collect());
        v
    });
    // The last beam fired breathes its entry and exit ports with the result color.
    let highlight = Signal::derive(move || {
        log.with(|v| {
            v.iter().rev().find_map(|e| match e {
                SoloEvent::Beam(s) => Some((s.call, s.exit, s.color)),
                _ => None,
            })
        })
    });

    view! {
        <div class="max-w-4xl mx-auto space-y-6">
            <div class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                <div class="flex items-center gap-2">
                    <h1 class="text-2xl font-bold dark:text-white">"Light Beams Single Player"</h1>
                    <RulesButton />
                </div>
                <div class="flex items-center gap-2 shrink-0">
                    {move || {
                        if rules.with(|r| r.timed) && !over() {
                            view! {
                                <span class="rounded-xl bg-stone-800 px-3 py-2 text-sm font-mono text-stone-200 ring-1 ring-stone-700">
                                    {move || format!("⏱ {}s", elapsed())}
                                </span>
                            }
                                .into_any()
                        } else {
                            view! { <span /> }.into_any()
                        }
                    }}
                    <a
                        href="/games/light-beams"
                        class="rounded-xl bg-stone-700 px-4 py-2 text-sm font-semibold text-stone-200 hover:bg-stone-600 touch-manipulation shadow"
                    >
                        "Back to Lobby"
                    </a>
                    <button
                        class="rounded-xl bg-stone-700 px-4 py-2 text-sm font-semibold text-stone-200 hover:bg-stone-600 touch-manipulation shadow"
                        on:click=on_new_board
                    >
                        "New Board"
                    </button>
                </div>
            </div>

            <p class="text-sm text-stone-400">
                "The computer hid a board. Shoot beams through the ports and read the exit port \
                and color to deduce where the pieces are. Build your guess on the board, then \
                call it."
            </p>

            <RulesConfig rules />

            {move || {
                if solved.get() {
                    view! {
                        <div class="rounded-xl border border-green-300 bg-green-100 text-green-800 px-4 py-3 font-semibold dark:border-green-700 dark:bg-green-900/30 dark:text-green-300">
                            {format!("Solved it in {}! You found the board.", score())}
                        </div>
                    }
                        .into_any()
                } else if revealed.get() {
                    view! {
                        <div class="rounded-xl border border-stone-600 bg-stone-800 px-4 py-3 text-stone-300 font-semibold">
                            {format!("Here's the board the computer hid. Score: {}.", score())}
                        </div>
                    }
                        .into_any()
                } else {
                    view! { <span /> }.into_any()
                }
            }}

            <div class="grid grid-cols-1 lg:grid-cols-2 gap-10">
                <div>
                    <h3 class="font-semibold mb-2 text-stone-300">
                        {move || if over() { "Answer" } else { "Guess Board" }}
                    </h3>
                    {move || {
                        if over() {
                            view! {
                                // Round finished, reveal the real board for comparison.
                                <BoardWithPorts
                                    board=Signal::derive(move || answer.get())
                                    on_port=|_| {}
                                    port_active=false
                                    on_cell=|_, _| {}
                                    cell_interactive=false
                                />
                            }
                                .into_any()
                        } else {
                            view! {
                                <BoardWithPorts
                                    board=Signal::derive(move || guess_board.get())
                                    on_port=on_shoot
                                    port_active=true
                                    on_cell=on_guess_cell
                                    cell_interactive=true
                                    highlight=highlight
                                />
                            }
                                .into_any()
                        }
                    }}

                    {move || {
                        if over() {
                            view! { <span /> }.into_any()
                        } else {
                            view! {
                                <div class="mt-4 space-y-3">
                                    <PiecePalette selected=guess_selected rules />
                                    <OrientationPicker selected=guess_selected />
                                    <div class="flex flex-wrap gap-2 items-center">
                                        {call_button(can_call, on_call)}
                                        {flip_button(
                                            can_flip,
                                            rotate_mode,
                                            move || {
                                                if flip_used.get() {
                                                    "Flip used (shoot to refresh)"
                                                } else if rotate_mode.get() {
                                                    "Tap a cell to flip it"
                                                } else {
                                                    "Flip a mirror"
                                                }
                                            },
                                        )}
                                        <button
                                            class="min-h-[44px] rounded-xl bg-stone-700 px-3 py-2 text-sm text-stone-300 hover:bg-stone-600 touch-manipulation shadow"
                                            on:click=move |_| {
                                                finish();
                                                revealed.set(true);
                                            }
                                        >
                                            "Give up / reveal"
                                        </button>
                                    </div>
                                    {move || {
                                        if last_guess_wrong.get() {
                                            view! {
                                                <p class="text-sm text-red-400">
                                                    "Not quite, that's not the board. Keep shooting."
                                                </p>
                                            }
                                                .into_any()
                                        } else {
                                            view! { <span /> }.into_any()
                                        }
                                    }}
                                    {move || {
                                        if let Some((row, col, accepted)) = last_flip.get() {
                                            let (top, left) = cell_label(row, col, size());
                                            let (msg, cls) = if accepted {
                                                (
                                                    format!("Mirror at ({top}, {left}) flipped!"),
                                                    "text-sm text-green-400",
                                                )
                                            } else {
                                                (
                                                    format!("No mirror at ({top}, {left}). That costs a beam."),
                                                    "text-sm text-stone-500",
                                                )
                                            };
                                            view! { <p class=cls>{msg}</p> }.into_any()
                                        } else {
                                            view! { <span /> }.into_any()
                                        }
                                    }}
                                </div>
                            }
                                .into_any()
                        }
                    }}
                </div>

                <div>
                    <HistoryTable rows title="History" last="Result" empty="No moves yet." />
                </div>
            </div>

            {move || {
                if over() {
                    view! {
                        <div class="flex flex-wrap items-start gap-3">
                            <button
                                class="rounded-xl bg-blue-500 px-6 py-2 text-white font-semibold hover:bg-blue-600 touch-manipulation shadow-md"
                                on:click=on_new_board
                            >
                                "Play Again"
                            </button>
                            <ShareBoard board=Signal::derive(move || answer.get()) />
                        </div>
                    }
                        .into_any()
                } else {
                    view! { <span /> }.into_any()
                }
            }}
        </div>
    }
}

// ── Sandbox ───────────────────────────────────────────────────────────────────

#[component]
pub fn SandboxInner() -> impl IntoView {
    let rules = RwSignal::new(Rules::default());
    let board = RwSignal::new(Board::new(4));
    let selected = RwSignal::new(Option::<Piece>::None);
    // The last test beam: (entry port, exit port, color).
    let test = RwSignal::new(Option::<(u8, u8, Color)>::None);

    // A new size or piece mix starts the design over.
    let dims = Memo::new(move |_| rules.with(|r| (r.size, r.counts)));
    Effect::watch(
        move || dims.get(),
        move |_, _, _| {
            board.set(Board::new(rules.get_untracked().size));
            test.set(None);
        },
        false,
    );

    let on_port = move |port: u8| {
        let shot = shoot_beam(&board.get_untracked(), port);
        test.set(Some((shot.call, shot.exit, shot.color)));
    };
    let on_cell = move |r: usize, c: usize| {
        let sel = selected.get_untracked();
        board.update(|b| b.toggle(r, c, sel));
        test.set(None);
    };
    let complete = move || {
        let r = rules.get();
        board.with(|b| logic::validate_board(b, &r))
    };

    view! {
        <div class="max-w-4xl mx-auto space-y-6">
            <div class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                <div class="flex items-center gap-2">
                    <h1 class="text-2xl font-bold dark:text-white">"Light Beams Sandbox"</h1>
                    <RulesButton />
                </div>
                <div class="flex items-center gap-2 shrink-0">
                    <a
                        href="/games/light-beams"
                        class="rounded-xl bg-stone-700 px-4 py-2 text-sm font-semibold text-stone-200 hover:bg-stone-600 touch-manipulation shadow"
                    >
                        "Back to Lobby"
                    </a>
                    <button
                        class="rounded-xl bg-stone-700 px-4 py-2 text-sm font-semibold text-stone-200 hover:bg-stone-600 touch-manipulation shadow"
                        on:click=move |_| {
                            board.set(Board::new(rules.get_untracked().size));
                            test.set(None);
                        }
                    >
                        "Clear Board"
                    </button>
                </div>
            </div>

            <p class="text-sm text-stone-400">
                "Design a board by hand: pick a piece, then place it on the grid. For a mirror, \
                choose its orientation first. Click a placed piece to remove it. Once every piece \
                is placed you can share the board as a single-player puzzle."
            </p>

            <RulesConfig rules />

            <PieceChecklist board rules />
            <PiecePalette selected rules large=true />
            <OrientationPicker selected />

            <BoardWithPorts
                board=Signal::derive(move || board.get())
                on_port
                port_active=true
                on_cell
                cell_interactive=true
                highlight=Signal::derive(move || test.get())
            />
            <p class="text-xs text-stone-500">
                "Tap a numbered port to shoot a test beam through your board and trace its path."
            </p>
            {move || {
                test.get()
                    .map(|(call, exit, color)| {
                        let result = if exit == 0 {
                            "trapped in a loop (∞)".to_string()
                        } else {
                            format!("exits port {exit} as {}", color.name())
                        };
                        view! {
                            <p class="text-sm text-stone-300">
                                <span class="font-semibold">{format!("Port {call} ")}</span>
                                {result}
                            </p>
                        }
                    })
            }}

            <div class="flex flex-col gap-3">
                {move || {
                    if complete() {
                        view! { <ShareBoard board=Signal::derive(move || board.get()) /> }
                            .into_any()
                    } else {
                        view! {
                            <p class="text-sm text-stone-500">
                                {move || rules.with(|r| piece_requirement_text(&r.counts))}
                            </p>
                        }
                            .into_any()
                    }
                }}
            </div>
        </div>
    }
}

// Lets the player turn a board into a shareable single player link. The board
// is encoded into an opaque `?b=` code so the answer can't be read at a glance
// from the URL.
#[component]
fn ShareBoard(board: Signal<Board>) -> impl IntoView {
    let link = RwSignal::new(Option::<String>::None);
    let copied = RwSignal::new(false);

    let make_link = move |_| {
        let code = logic::encode_board(&board.get_untracked());
        #[cfg(target_arch = "wasm32")]
        {
            let origin = web_sys::window()
                .and_then(|w| w.location().origin().ok())
                .unwrap_or_default();
            let url = format!("{origin}/games/light-beams/single-player?b={code}");
            // Copy straight to the clipboard so sharing is a single tap. The
            // Clipboard API is undefined in insecure (http) contexts; guard it
            // and only fall back to revealing the link for a manual copy when
            // the copy couldn't happen.
            let mut did_copy = false;
            if let Some(nav) = web_sys::window().map(|w| w.navigator()) {
                let clip = nav.clipboard();
                let clip_val: &wasm_bindgen::JsValue = clip.as_ref();
                if !clip_val.is_undefined() && !clip_val.is_null() {
                    let _ = clip.write_text(&url);
                    did_copy = true;
                }
            }
            if did_copy {
                link.set(None);
                copied.set(true);
                // Drop the "copied" badge after a moment so it can fire again.
                set_timeout(move || copied.set(false), std::time::Duration::from_secs(2));
            } else {
                copied.set(false);
                link.set(Some(url));
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            link.set(Some(format!("/games/light-beams/single-player?b={code}")));
        }
    };

    view! {
        <div class="flex flex-col gap-2">
            <button
                class="rounded-xl bg-stone-700 px-6 py-2 text-stone-200 font-semibold hover:bg-stone-600 touch-manipulation shadow"
                on:click=make_link
            >
                {move || if copied.get() { "Link copied!" } else { "Share this board" }}
            </button>
            {move || {
                link.get()
                    .map(|url| {
                        view! {
                            // Only shown when the clipboard wasn't available, so the player
                            // can still copy the link by hand.
                            <div class="space-y-1">
                                <input
                                    type="text"
                                    readonly=true
                                    prop:value=url
                                    class="w-full min-w-[16rem] rounded border border-stone-600 bg-stone-800 px-3 py-2 text-xs font-mono text-stone-300"
                                />
                                <p class="text-xs text-stone-500">
                                    "Copy this link to share the puzzle."
                                </p>
                            </div>
                        }
                    })
            }}
        </div>
    }
}

// ── Rule set picker ───────────────────────────────────────────────────────────

/// Trim piece counts so they fit on a board with `cells` cells, keeping the
/// earlier piece types first.
fn clamp_counts(counts: PieceCounts, cells: u32) -> PieceCounts {
    let mut left = cells;
    let mut out = PieceCounts::default();
    for ty in PieceType::ALL {
        let n = (counts.get(ty) as u32).min(left);
        out.set(ty, n as u8);
        left -= n;
    }
    out
}

/// Pick ruleset A, B or a custom board. `multiplayer` adds the options that only
/// matter with other players: who designs the board and asynchronous play.
#[component]
pub fn RulesConfig(rules: RwSignal<Rules>, #[prop(optional)] multiplayer: bool) -> impl IntoView {
    let custom_open = RwSignal::new(false);
    let a = Rules::classic_a();
    let b = Rules::ruleset_b();
    let preset = move || {
        rules.with(|r| {
            if r.size == a.size && r.counts == a.counts {
                0
            } else if r.size == b.size && r.counts == b.counts {
                1
            } else if custom_open.get() {
                2
            } else {
                3
            }
        })
    };
    let pick = move |p: Rules| {
        rules.update(|r| {
            r.size = p.size;
            r.counts = p.counts;
        });
        custom_open.set(false);
    };
    let preset_button = move |idx: u8,
                              label: &'static str,
                              on_click: Box<dyn Fn() + Send + Sync>| {
        view! {
            <button
                type="button"
                class=move || {
                    format!(
                        "rounded-lg px-3 py-1.5 text-sm font-semibold touch-manipulation transition-all {}",
                        if preset() == idx {
                            "bg-blue-500 text-white"
                        } else {
                            "bg-stone-700 text-stone-200 hover:bg-stone-600"
                        },
                    )
                }
                on:click=move |_| on_click()
            >
                {label}
            </button>
        }
    };
    let cells = move || rules.with(|r| r.size as u32 * r.size as u32);
    let total = move || rules.with(|r| r.counts.total());

    view! {
        <div class="rounded-xl border border-stone-700 bg-stone-800/60 p-4 space-y-3">
            <div class="flex flex-wrap items-center gap-2">
                <span class="text-sm font-semibold text-stone-300 mr-1">"Ruleset:"</span>
                {preset_button(0, "A · Classic 4×4", Box::new(move || pick(a)))}
                {preset_button(1, "B · 5×5 + 4-way", Box::new(move || pick(b)))}
                {preset_button(2, "Custom", Box::new(move || custom_open.update(|o| *o = !*o)))}
            </div>
            {move || {
                if !custom_open.get() {
                    return view! { <span /> }.into_any();
                }
                view! {
                    <div class="space-y-4 border-t border-stone-700 pt-3">
                        <div>
                            <label class="flex items-center justify-between text-sm text-stone-300">
                                <span>"Board size"</span>
                                <span class="font-mono text-stone-200">
                                    {move || rules.with(|r| format!("{0}×{0}", r.size))}
                                </span>
                            </label>
                            <input
                                type="range"
                                min=MIN_SIZE
                                max=MAX_SIZE
                                step="1"
                                class="w-full accent-blue-500"
                                prop:value=move || rules.with(|r| r.size.to_string())
                                on:input=move |ev| {
                                    if let Ok(n) = event_target_value(&ev).parse::<u8>() {
                                        let n = n.clamp(MIN_SIZE, MAX_SIZE);
                                        rules
                                            .update(|r| {
                                                r.size = n;
                                                r.counts = clamp_counts(r.counts, n as u32 * n as u32);
                                            });
                                    }
                                }
                            />
                        </div>
                        <div>
                            <div class="flex items-center justify-between mb-1">
                                <span class="text-sm text-stone-300">"Pieces"</span>
                                <span class=move || {
                                    format!(
                                        "text-xs font-mono {}",
                                        if rules.with(|r| r.is_valid()) {
                                            "text-stone-500"
                                        } else {
                                            "text-red-400"
                                        },
                                    )
                                }>{move || format!("{} / {} cells", total(), cells())}</span>
                            </div>
                            <div class="grid grid-cols-2 sm:grid-cols-3 gap-2">
                                {PieceType::ALL
                                    .into_iter()
                                    .map(|ty| {
                                        view! {
                                            <div class="flex items-center gap-2 rounded-lg bg-stone-800 px-2 py-1.5 ring-1 ring-stone-700">
                                                <div class="h-7 w-7 shrink-0 rounded bg-stone-700 p-1">
                                                    {piece_icon(ty.sample())}
                                                </div>
                                                <span class="flex-1 truncate text-xs text-stone-300">
                                                    {ty.sample().label()}
                                                </span>
                                                <button
                                                    type="button"
                                                    class="h-6 w-6 rounded bg-stone-700 text-stone-200 hover:bg-stone-600 touch-manipulation"
                                                    on:click=move |_| {
                                                        rules
                                                            .update(|r| {
                                                                let n = r.counts.get(ty);
                                                                r.counts.set(ty, n.saturating_sub(1));
                                                            })
                                                    }
                                                >
                                                    "−"
                                                </button>
                                                <span class="w-4 text-center font-mono text-sm text-stone-100">
                                                    {move || rules.with(|r| r.counts.get(ty))}
                                                </span>
                                                <button
                                                    type="button"
                                                    class="h-6 w-6 rounded bg-stone-700 text-stone-200 hover:bg-stone-600 touch-manipulation"
                                                    on:click=move |_| {
                                                        if total() < cells() {
                                                            rules
                                                                .update(|r| {
                                                                    let n = r.counts.get(ty);
                                                                    r.counts.set(ty, n + 1);
                                                                })
                                                        }
                                                    }
                                                >
                                                    "+"
                                                </button>
                                            </div>
                                        }
                                    })
                                    .collect_view()}
                            </div>
                        </div>
                        <div class="flex flex-wrap items-center gap-4 text-sm text-stone-300">
                            {if multiplayer {
                                view! {
                                    <label class="flex items-center gap-2">
                                        <span>"Board:"</span>
                                        <select
                                            class="rounded bg-stone-700 px-2 py-1 text-stone-100"
                                            on:change=move |ev| {
                                                let source = if event_target_value(&ev) == "computer" {
                                                    BoardSource::Computer
                                                } else {
                                                    BoardSource::Versus
                                                };
                                                rules.update(|r| r.source = source);
                                            }
                                        >
                                            <option
                                                value="versus"
                                                selected=move || {
                                                    rules.with(|r| matches!(r.source, BoardSource::Versus))
                                                }
                                            >
                                                "Design vs opponent"
                                            </option>
                                            <option
                                                value="computer"
                                                selected=move || {
                                                    rules.with(|r| matches!(r.source, BoardSource::Computer))
                                                }
                                            >
                                                "Computer-generated"
                                            </option>
                                        </select>
                                    </label>
                                    <label class="flex items-center gap-2">
                                        <input
                                            type="checkbox"
                                            class="accent-blue-500"
                                            prop:checked=move || {
                                                rules.with(|r| matches!(r.resolution, Resolution::Async))
                                            }
                                            on:change=move |ev| {
                                                let on = event_target_checked(&ev);
                                                rules
                                                    .update(|r| {
                                                        r.resolution = if on {
                                                            Resolution::Async
                                                        } else {
                                                            Resolution::Live
                                                        };
                                                    });
                                            }
                                        />
                                        <span>"Asynchronous (compare scores)"</span>
                                    </label>
                                }
                                    .into_any()
                            } else {
                                view! { <span /> }.into_any()
                            }} <label class="flex items-center gap-2">
                                <input
                                    type="checkbox"
                                    class="accent-blue-500"
                                    prop:checked=move || rules.with(|r| r.timed)
                                    on:change=move |ev| {
                                        let on = event_target_checked(&ev);
                                        rules.update(|r| r.timed = on);
                                    }
                                />
                                <span>"Timed (score by solve time)"</span>
                            </label>
                        </div>
                    </div>
                }
                    .into_any()
            }}
        </div>
    }
}

// ── Board cells ───────────────────────────────────────────────────────────────

/// Pixel size of one cell for a board of `n`×`n`, so bigger boards still fit.
fn cell_px(n: u8) -> u32 {
    match n {
        0..=4 => 54,
        5 => 44,
        6 => 38,
        7 => 34,
        8 => 30,
        9 => 28,
        _ => 26,
    }
}

fn cell_view(
    cell: impl Fn() -> Option<Piece> + Copy + Send + Sync + 'static,
    interactive: bool,
    px: u32,
    on_click: impl Fn() + Send + Sync + 'static,
) -> impl IntoView {
    view! {
        <button
            class=move || {
                let base = "rounded-xl flex items-center justify-center border-2 touch-manipulation transition-all p-1.5";
                let state_cls = if cell().is_some() {
                    "bg-stone-600 border-stone-500 shadow-inner"
                } else if interactive {
                    "bg-stone-700 border-stone-600 hover:bg-stone-600 hover:border-stone-500"
                } else {
                    "bg-stone-700 border-stone-600"
                };
                let cursor = if interactive { " cursor-pointer" } else { " cursor-default" };
                format!("{base} {state_cls}{cursor}")
            }
            style=format!("width:{px}px;height:{px}px")
            on:click=move |_| {
                if interactive {
                    on_click();
                }
            }
        >
            {move || {
                if let Some(p) = cell() {
                    view! { <div class="w-full h-full piece-pop">{piece_icon(p)}</div> }.into_any()
                } else {
                    view! { <span class="text-stone-600 text-lg select-none">"·"</span> }
                        .into_any()
                }
            }}
        </button>
    }
}

/// One row of cells.
fn cell_row(
    board: Signal<Board>,
    r: usize,
    n: usize,
    interactive: bool,
    on_cell: impl Fn(usize, usize) + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let px = cell_px(n as u8);
    view! {
        <div class="flex gap-1.5">
            {(0..n)
                .map(|c| {
                    let cell = move || board.with(|b| b.get(r, c));
                    cell_view(cell, interactive, px, move || on_cell(r, c))
                })
                .collect_view()}
        </div>
    }
}

// ── Flat grid ─────────────────────────────────────────────────────────────────

#[component]
fn BoardGrid(
    #[prop(into)] board: Signal<Board>,
    on_cell: impl Fn(usize, usize) + Copy + Send + Sync + 'static,
    interactive: bool,
) -> impl IntoView {
    let size = Memo::new(move |_| board.with(|b| b.size()));
    view! {
        <div class="inline-block bg-stone-900 p-2.5 rounded-2xl shadow-2xl ring-1 ring-stone-700">
            {move || {
                let n = size.get();
                view! {
                    <div class="flex flex-col gap-1.5">
                        {(0..n).map(|r| cell_row(board, r, n, interactive, on_cell)).collect_view()}
                    </div>
                }
            }}
        </div>
    }
}

// ── Board with port number buttons ────────────────────────────────────────────

//        1   2   3   4
//   16 [  ] [  ] [  ] [  ]  5
//   15 [  ] [  ] [  ] [  ]  6
//   14 [  ] [  ] [  ] [  ]  7
//   13 [  ] [  ] [  ] [  ]  8
//        12  11  10   9
//
// Bigger boards number the same way: 1..n across the top, then down the right,
// back along the bottom and up the left.

#[component]
fn BoardWithPorts(
    #[prop(into)] board: Signal<Board>,
    on_port: impl Fn(u8) + Copy + Send + Sync + 'static,
    #[prop(into)] port_active: Signal<bool>,
    on_cell: impl Fn(usize, usize) + Copy + Send + Sync + 'static,
    cell_interactive: bool,
    /// The most recent beam to highlight: (entry port, exit port, color). The two
    /// ports breathe with the color until a new beam replaces it.
    #[prop(optional, into)]
    highlight: Signal<Option<(u8, u8, Color)>>,
) -> impl IntoView {
    let size = Memo::new(move |_| board.with(|b| b.size()));

    // Is this port the entry or exit of the highlighted beam, and if so, in what
    // color should it glow? A trapped beam only lights its entry port.
    let lit = move |p: u8| {
        highlight.with(|h| match h {
            Some((call, exit, color)) if *call == p || *exit == p => Some((*color, *exit == 0)),
            _ => None,
        })
    };

    view! {
        <div class="inline-block select-none overflow-x-auto max-w-full">
            <div class="inline-block bg-stone-900 p-2.5 rounded-2xl shadow-2xl ring-1 ring-stone-700">
                {move || {
                    let n = size.get();
                    let cell = cell_px(n as u8);
                    let port = cell.min(34);
                    let port_btn = move |p: u8| {
                        view! {
                            <button
                                style=move || {
                                    let glow = lit(p)
                                        .map(|(color, _)| {
                                            format!("--beam-color: {};", color.glow())
                                        })
                                        .unwrap_or_default();
                                    format!("width:{port}px;height:{port}px;{glow}")
                                }
                                class=move || {
                                    let base = "rounded-full text-xs font-bold touch-manipulation transition-all";
                                    if let Some((color, looped)) = lit(p) {
                                        let anim = if looped {
                                            "beam-loop"
                                        } else {
                                            "beam-breathe"
                                        };
                                        format!("{base} {anim} {}", color.css_class())
                                    } else if port_active.get() {
                                        format!(
                                            "{base} bg-stone-500 text-white hover:bg-stone-400 shadow-md shadow-stone-900",
                                        )
                                    } else {
                                        format!("{base} bg-stone-700 text-stone-400 cursor-default")
                                    }
                                }
                                on:click=move |_| {
                                    if port_active.get_untracked() {
                                        on_port(p);
                                    }
                                }
                            >
                                {p}
                            </button>
                        }
                    };
                    let col_port = move |p: u8| {
                        view! {
                            // Wrap a port button in a cell-sized box so it lines up with its
                            // grid column / row.
                            <div class="flex justify-center" style=format!("width:{cell}px")>
                                {port_btn(p)}
                            </div>
                        }
                    };
                    let row_port = move |p: u8| {
                        view! {
                            <div class="flex items-center" style=format!("height:{cell}px")>
                                {port_btn(p)}
                            </div>
                        }
                    };
                    let m = n as u8;
                    let top: Vec<u8> = (1..=m).collect();
                    let right: Vec<u8> = (m + 1..=2 * m).collect();
                    let bottom: Vec<u8> = (2 * m + 1..=3 * m).rev().collect();
                    let left: Vec<u8> = (3 * m + 1..=4 * m).rev().collect();
                    view! {
                        <div class="flex items-center mb-1.5 gap-1.5">
                            <div class="shrink-0" style=format!("width:{port}px")></div>
                            <div class="flex gap-1.5">
                                {top.into_iter().map(col_port).collect_view()}
                            </div>
                            <div class="shrink-0" style=format!("width:{port}px")></div>
                        </div>
                        <div class="flex items-start gap-1.5">
                            <div class="flex flex-col gap-1.5">
                                {left.into_iter().map(row_port).collect_view()}
                            </div>
                            <div class="flex flex-col gap-1.5">
                                {(0..n)
                                    .map(|r| cell_row(board, r, n, cell_interactive, on_cell))
                                    .collect_view()}
                            </div>
                            <div class="flex flex-col gap-1.5">
                                {right.into_iter().map(row_port).collect_view()}
                            </div>
                        </div>
                        <div class="flex items-center mt-1.5 gap-1.5">
                            <div class="shrink-0" style=format!("width:{port}px")></div>
                            <div class="flex gap-1.5">
                                {bottom.into_iter().map(col_port).collect_view()}
                            </div>
                            <div class="shrink-0" style=format!("width:{port}px")></div>
                        </div>
                    }
                }}
            </div>
        </div>
    }
}

// ── Rules overlay ─────────────────────────────────────────────────────────────

/// The example board from the rule book, matching the worked example below:
///
/// ```text
///    1  2  3  4
/// 16 -  -  B ↑→ 5
/// 15 -  R  -  Y 6
/// 14 █  -  -  - 7
/// 13 -  -  /  - 8
///    12 11 10 9
/// ```
fn example_board() -> Board {
    let mut b = Board::new(4);
    b.set(0, 2, Some(Piece::Blue));
    b.set(0, 3, Some(Piece::Arrow(ArrowDir::UpRight)));
    b.set(1, 1, Some(Piece::Red));
    b.set(1, 3, Some(Piece::Yellow));
    b.set(2, 0, Some(Piece::Black));
    b.set(3, 2, Some(Piece::Slash(SlashDir::Forward)));
    b
}

/// A small inline piece icon, used in the legend.
fn legend_item(piece: Piece, label: &'static str) -> impl IntoView {
    view! {
        <div class="flex items-center gap-2">
            <div class="h-9 w-9 shrink-0 rounded-lg bg-stone-700 p-1.5 ring-1 ring-stone-600">
                {piece_icon(piece)}
            </div>
            <span class="text-sm text-stone-300">{label}</span>
        </div>
    }
}

/// The "?" rules button for Light Beams. Drop it anywhere (lobby, single
/// player, game room) to give players a rule book.
#[component]
pub fn RulesButton() -> impl IntoView {
    view! {
        <crate::games::rules::RulesButton title="How to play Light Beams">
            <LightBeamsRules />
        </crate::games::rules::RulesButton>
    }
}

#[component]
fn LightBeamsRules() -> impl IntoView {
    let example = Signal::derive(example_board);

    // (call, exit, color, explanation)
    let worked: &[(u8, u8, &str, &str)] = &[
        (
            4,
            4,
            "White",
            "Enters going down into the up-arrow head-on and bounces straight back.",
        ),
        (
            16,
            5,
            "Blue",
            "Passes through the blue glass, then through the arrow untouched (not hit head-on).",
        ),
        (
            1,
            12,
            "Black",
            "Travels straight down through the black piece, which absorbs all light.",
        ),
        (
            6,
            15,
            "Orange",
            "Passes through the yellow glass, then the red glass. Yellow + red = orange.",
        ),
        (
            13,
            3,
            "Blue",
            "Enters going right, bends upward off the slash mirror (/), then passes through \
             the blue glass on its way out.",
        ),
    ];

    view! {
        <section class="space-y-2">
            <h3 class="font-semibold text-white">"Goal"</h3>
            <p>
                "You and your opponent each hide pieces on a grid (4×4 in the classic "
                "game). Shoot beams of light through the ports and read where they exit "
                "and what color they become to deduce the hidden layout. Be the first to "
                "call your opponent's entire board correctly."
            </p>
        </section>

        <section class="space-y-2">
            <h3 class="font-semibold text-white">"Ports"</h3>
            <p>
                "Numbered ports ring the grid — sixteen on a 4×4 board, more on bigger "
                "ones. Light enters one and exits another. You call a port, and your "
                "opponent answers with the exit port and the color."
            </p>
        </section>

        <section class="space-y-3">
            <h3 class="font-semibold text-white">"The pieces"</h3>
            <p>
                "In the classic game each player places seven pieces: the three colored "
                "glasses, the black absorber, an arrow mirror, and two slash mirrors. "
                "Custom games can change how many of each appear."
            </p>
            <div class="grid grid-cols-2 gap-3 sm:grid-cols-3">
                {legend_item(Piece::Red, "Red glass")} {legend_item(Piece::Yellow, "Yellow glass")}
                {legend_item(Piece::Blue, "Blue glass")}
                {legend_item(Piece::Black, "Black absorbs light")}
                {legend_item(Piece::Slash(SlashDir::Forward), "Slash mirror (turns 90°)")}
                {legend_item(Piece::Arrow(ArrowDir::UpRight), "Arrow mirror")}
                {legend_item(Piece::FourArrow, "4-way mirror")}
            </div>
            <ul class="ml-4 list-disc space-y-1 text-stone-300">
                <li>
                    <span class="font-medium text-white">"Colored glass"</span>
                    " tints the beam. Colors mix as they stack (see below)."
                </li>
                <li>
                    <span class="font-medium text-white">"Black"</span>
                    " absorbs the beam. Any beam touching it exits black, no matter "
                    "what else it passed through."
                </li>
                <li>
                    <span class="font-medium text-white">"Slash mirror"</span>
                    " (/ or \\) bends light 90°. A flip can rotate it during play."
                </li>
                <li>
                    <span class="font-medium text-white">"Arrow mirror"</span>
                    " is a corner of two arrows. A beam hitting an arrow point "
                    "head-on bounces straight back; beams from the other directions "
                    "pass straight through. Its orientation is fixed at setup."
                </li>
                <li>
                    <span class="font-medium text-white">"4-way mirror"</span>
                    " is four arrows in one. A beam entering from any side bounces "
                    "straight back out the port it came in. (Ruleset B and custom games.)"
                </li>
            </ul>
        </section>

        <section class="space-y-2">
            <h3 class="font-semibold text-white">"Mixing colors"</h3>
            <div class="divide-y divide-stone-700 overflow-hidden rounded-lg ring-1 ring-stone-700">
                <div class="grid grid-cols-2 bg-stone-700/60 text-xs font-medium text-stone-300">
                    <div class="px-3 py-1.5">"Passes through"</div>
                    <div class="px-3 py-1.5">"Result"</div>
                </div>
                {[
                    ("Red + Yellow", "Orange", "bg-orange-500"),
                    ("Red + Blue", "Purple", "bg-purple-500"),
                    ("Yellow + Blue", "Green", "bg-green-500"),
                    ("Red + Yellow + Blue", "Brown", "bg-amber-800"),
                ]
                    .into_iter()
                    .map(|(mix, result, swatch)| {
                        view! {
                            <div class="grid grid-cols-2 items-center bg-stone-800">
                                <div class="px-3 py-1.5 text-stone-200">{mix}</div>
                                <div class="px-3 py-1.5">
                                    <span class=format!(
                                        "rounded px-2 py-0.5 text-xs font-semibold text-white {swatch}",
                                    )>{result}</span>
                                </div>
                            </div>
                        }
                    })
                    .collect_view()}
            </div>
        </section>

        <section class="space-y-3">
            <h3 class="font-semibold text-white">"Example board"</h3>
            <p>
                "Here is a filled board. Try following each beam below to see how the "
                "pieces shape the light."
            </p>
            <div class="flex justify-center">
                <BoardWithPorts
                    board=example
                    on_port=|_| {}
                    port_active=false
                    on_cell=|_, _| {}
                    cell_interactive=false
                />
            </div>
            <div class="divide-y divide-stone-700 overflow-hidden rounded-lg ring-1 ring-stone-700">
                <div class="grid grid-cols-[2.5rem_2.5rem_4rem_1fr] bg-stone-700/60 text-xs font-medium text-stone-300">
                    <div class="px-3 py-1.5">"Call"</div>
                    <div class="px-3 py-1.5">"Exit"</div>
                    <div class="px-3 py-1.5">"Color"</div>
                    <div class="px-3 py-1.5">"Why"</div>
                </div>
                {worked
                    .iter()
                    .map(|(call, exit, color, why)| {
                        view! {
                            <div class="grid grid-cols-[2.5rem_2.5rem_4rem_1fr] items-baseline bg-stone-800">
                                <div class="px-3 py-1.5 font-mono text-stone-200">{*call}</div>
                                <div class="px-3 py-1.5 font-mono text-stone-200">{*exit}</div>
                                <div class="px-3 py-1.5 text-stone-200">{*color}</div>
                                <div class="px-3 py-1.5 text-stone-300">{*why}</div>
                            </div>
                        }
                    })
                    .collect_view()}
            </div>
        </section>

        <section class="space-y-2">
            <h3 class="font-semibold text-white">"Taking a turn"</h3>
            <ul class="ml-4 list-disc space-y-1 text-stone-300">
                <li>
                    "Optionally " <span class="font-medium text-white">"flip"</span>
                    " one cell on the opponent's board. If it holds a slash mirror "
                    "it rotates (/ ↔ \\). They're told which cell you flipped."
                </li>
                <li>
                    <span class="font-medium text-white">"Shoot a beam"</span>
                    " through a port. The exit port and color are revealed. This "
                    "ends your turn."
                </li>
                <li>
                    "When you're sure, " <span class="font-medium text-white">"call the game"</span>
                    " with your full guess. An exact match wins; otherwise play " "continues."
                </li>
            </ul>
        </section>

        <section class="space-y-2">
            <h3 class="font-semibold text-white">"Rulesets & variations"</h3>
            <ul class="ml-4 list-disc space-y-1 text-stone-300">
                <li>
                    <span class="font-medium text-white">"Ruleset A"</span>
                    " is the classic 4×4 game above. "
                    <span class="font-medium text-white">"Ruleset B"</span>
                    " is 5×5 and adds the 4-way mirror. "
                    <span class="font-medium text-white">"Custom"</span>
                    " lets you set the board size (up to 10×10) and how many of each "
                    "piece appear."
                </li>
                <li>
                    <span class="font-medium text-white">"Computer-generated"</span>
                    " games hide one board that both players race to guess, instead "
                    "of designing boards for each other."
                </li>
                <li>
                    <span class="font-medium text-white">"Asynchronous"</span>
                    " games drop turns: each player solves independently and the "
                    "lower score wins once both finish. Score is beams used, or "
                    "solve time in a "
                    <span class="font-medium text-white">"timed"</span>
                    " game."
                </li>
                <li>
                    "With enough mirrors a beam can get stuck bouncing forever. A "
                    "trapped beam is reported as a "
                    <span class="font-medium text-white">"loop (∞)"</span> " with no exit."
                </li>
            </ul>
        </section>
    }
}
