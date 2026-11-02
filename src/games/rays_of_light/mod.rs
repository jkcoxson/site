// Jackson Coxson

pub mod logic;

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use logic::{rotate_piece, same_piece_type, ArrowDir, Board, Color, Piece, ResyncPhase, SlashDir};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ServerMsg {
    Welcome {
        player: u8,
        token: String,
    },
    WaitingForOpponent,
    GameStart,
    YourTurn,
    OpponentTurn,
    RayResult {
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
    /// Full state snapshot after reconnect.
    GameResync {
        player: u8,
        phase: ResyncPhase,
    },
    OpponentReconnected,
    OpponentDisconnected,
    GameOver {
        you_win: bool,
        opponent_board: Board,
    },
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

#[derive(Clone, Debug, PartialEq, Default)]
pub enum Phase {
    #[default]
    Connecting,
    WaitingForOpponent,
    Setup,
    Playing {
        your_turn: bool,
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

#[derive(Clone, Debug)]
pub struct Shot {
    pub call: u8,
    pub exit: u8,
    pub color: Color,
    pub mine: bool,
}

/// A single entry in the single-player history log. Every entry counts as one
/// "ray" toward the score, so wrong guesses and wasted flips are recorded here
/// alongside real shots to explain where the tally comes from.
#[derive(Clone, Debug)]
enum SoloEvent {
    Ray(Shot),
    WrongGuess,
    WastedFlip { row: u8, col: u8 },
}

#[derive(Clone, Debug, Default)]
pub struct GameUiState {
    pub phase: Phase,
    pub player: Option<u8>,
    pub room_id: String,
    pub your_board: Board,
    pub guess_board: Board,
    /// The opponent's actual board, revealed when the game ends.
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
}

/// Errors the server sends immediately before closing the socket. Reconnecting
/// after one of these is pointless, so the client treats them as terminal.
fn is_fatal_error(msg: &str) -> bool {
    matches!(msg, "Room not found." | "Wrong passcode." | "Room is full.")
}

impl GameUiState {
    fn new(room_id: String) -> Self {
        Self {
            room_id,
            ..Default::default()
        }
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
            ServerMsg::WaitingForOpponent => self.phase = Phase::WaitingForOpponent,
            ServerMsg::GameStart => {
                self.phase = Phase::Setup;
                self.your_board = Default::default();
                self.guess_board = Default::default();
                self.shot_history.clear();
                self.opponent_locked = false;
                self.flip_used_this_turn = false;
            }
            ServerMsg::YourTurn => {
                self.phase = Phase::Playing { your_turn: true };
                // A new turn begins — refresh the flip allowance.
                self.flip_used_this_turn = false;
            }
            ServerMsg::OpponentTurn => self.phase = Phase::Playing { your_turn: false },
            ServerMsg::RayResult { call, exit, color } => {
                self.shot_history.push(Shot {
                    call,
                    exit,
                    color,
                    mine: true,
                });
                self.phase = Phase::Playing { your_turn: false };
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
                self.phase = Phase::Playing { your_turn: false };
            }
            ServerMsg::OpponentCalledGame => {}
            ServerMsg::RotateResult { row, col, accepted } => {
                self.last_rotate_result = Some((row, col, accepted));
                // Your one flip for this turn is now spent.
                self.flip_used_this_turn = true;
                // Flipping a cell with no mirror loses your turn.
                if !accepted {
                    self.phase = Phase::Playing { your_turn: false };
                }
            }
            ServerMsg::YourMirrorRotated { row, col, accepted } => {
                if accepted {
                    let r = row as usize;
                    let c = col as usize;
                    if let Some(p) = self.your_board[r][c] {
                        self.your_board[r][c] = Some(rotate_piece(p));
                    }
                }
                self.last_mirror_attempt = Some((row, col, accepted));
            }
            ServerMsg::GameResync { player, phase } => {
                self.player = Some(player);
                self.phase = match &phase {
                    ResyncPhase::WaitingForOpponent => Phase::WaitingForOpponent,
                    ResyncPhase::Setup { .. } => Phase::Setup,
                    ResyncPhase::Playing { your_turn, .. } => Phase::Playing {
                        your_turn: *your_turn,
                    },
                    ResyncPhase::Over { you_win, .. } => Phase::GameOver { you_win: *you_win },
                };
                match phase {
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
                        self.shot_history = shots
                            .into_iter()
                            .map(|s| Shot {
                                call: s.call,
                                exit: s.exit,
                                color: s.color,
                                mine: s.mine,
                            })
                            .collect();
                    }
                    ResyncPhase::Over {
                        your_board,
                        opponent_board,
                        shots,
                        ..
                    } => {
                        self.your_board = your_board;
                        self.opponent_board = Some(opponent_board);
                        self.shot_history = shots
                            .into_iter()
                            .map(|s| Shot {
                                call: s.call,
                                exit: s.exit,
                                color: s.color,
                                mine: s.mine,
                            })
                            .collect();
                    }
                    _ => {}
                }
                self.opponent_disconnected = false;
            }
            ServerMsg::OpponentReconnected => {
                self.opponent_disconnected = false;
            }
            ServerMsg::OpponentDisconnected => {
                self.opponent_disconnected = true;
            }
            ServerMsg::GameOver {
                you_win,
                opponent_board,
            } => {
                self.opponent_board = Some(opponent_board);
                self.phase = Phase::GameOver { you_win };
            }
            ServerMsg::Error(e) => {
                // Connection-rejection errors are terminal — the server closes
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

#[cfg(target_arch = "wasm32")]
fn token_storage_key(room_id: &str) -> String {
    format!("rol_token_{room_id}")
}

/// Convert internal `(row, col)` board indices into the port numbers a player
/// actually reads off the edges of the board: the number printed above the
/// column and the number printed beside the row. The top edge is labelled
/// 1-4 left-to-right and the left edge 16-13 top-to-bottom, so the top-left
/// cell reads as (1, 16). Returns `(top, left)`.
fn cell_label(row: u8, col: u8) -> (u8, u8) {
    (col + 1, 16 - row)
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
        // Don't reconnect once the game is over or the connection was rejected
        // (room missing, wrong passcode, full) — there's nothing to rejoin.
        let terminal =
            state.with_untracked(|s| matches!(s.phase, Phase::GameOver { .. } | Phase::Closed));
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
                    <h1 class="text-2xl font-bold dark:text-white">"Rays of Light"</h1>
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

            {move || match state.with(|s| s.phase.clone()) {
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
                Phase::WaitingForOpponent => {
                    view! {
                        <div class="text-center py-16 space-y-3">
                            <p class="text-lg dark:text-white">"Waiting for opponent..."</p>
                            <p class="text-sm text-stone-500 dark:text-stone-400">
                                "Share the room code above."
                            </p>
                        </div>
                    }
                        .into_any()
                }
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
                Phase::Playing { your_turn } => {
                    view! { <PlayView state your_turn /> }.into_any()
                }
                Phase::GameOver { you_win } => {
                    view! { <GameOverView state you_win /> }.into_any()
                }
                Phase::Closed => {
                    view! {
                        <div class="text-center py-16 space-y-3">
                            <p class="text-lg dark:text-white">"Couldn't join this game."</p>
                            <a
                                href="/games/rays-of-light"
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

// ── Piece visual icon (SVG) ───────────────────────────────────────────────────

fn piece_icon(piece: Piece) -> AnyView {
    match piece {
        Piece::Red => view! {
            <svg viewBox="0 0 32 32" class="w-full h-full" xmlns="http://www.w3.org/2000/svg">
                <circle cx="16" cy="17" r="11" fill="rgba(0,0,0,0.25)"/>
                <circle cx="16" cy="16" r="11" fill="#ef4444"/>
                <circle cx="12" cy="11" r="4" fill="rgba(255,255,255,0.35)"/>
            </svg>
        }
        .into_any(),
        Piece::Yellow => view! {
            <svg viewBox="0 0 32 32" class="w-full h-full" xmlns="http://www.w3.org/2000/svg">
                <circle cx="16" cy="17" r="11" fill="rgba(0,0,0,0.25)"/>
                <circle cx="16" cy="16" r="11" fill="#eab308"/>
                <circle cx="12" cy="11" r="4" fill="rgba(255,255,255,0.35)"/>
            </svg>
        }
        .into_any(),
        Piece::Blue => view! {
            <svg viewBox="0 0 32 32" class="w-full h-full" xmlns="http://www.w3.org/2000/svg">
                <circle cx="16" cy="17" r="11" fill="rgba(0,0,0,0.25)"/>
                <circle cx="16" cy="16" r="11" fill="#3b82f6"/>
                <circle cx="12" cy="11" r="4" fill="rgba(255,255,255,0.35)"/>
            </svg>
        }
        .into_any(),
        Piece::Black => view! {
            <svg viewBox="0 0 32 32" class="w-full h-full" xmlns="http://www.w3.org/2000/svg">
                <circle cx="16" cy="17" r="11" fill="rgba(0,0,0,0.5)"/>
                <circle cx="16" cy="16" r="11" fill="#1c1917"/>
                <circle cx="12" cy="11" r="3" fill="rgba(255,255,255,0.18)"/>
            </svg>
        }
        .into_any(),
        Piece::Slash(SlashDir::Forward) => view! {
            // / mirror — line from bottom-left to top-right
            <svg viewBox="0 0 32 32" class="w-full h-full" xmlns="http://www.w3.org/2000/svg">
                <line x1="8" y1="26" x2="24" y2="6" stroke="rgba(0,0,0,0.3)" stroke-width="5" stroke-linecap="round"/>
                <line x1="8" y1="25" x2="24" y2="5" stroke="#94a3b8" stroke-width="5" stroke-linecap="round"/>
                <line x1="10" y1="22" x2="22" y2="8" stroke="rgba(255,255,255,0.5)" stroke-width="2" stroke-linecap="round"/>
            </svg>
        }
        .into_any(),
        Piece::Slash(SlashDir::Backward) => view! {
            // \ mirror — line from top-left to bottom-right
            <svg viewBox="0 0 32 32" class="w-full h-full" xmlns="http://www.w3.org/2000/svg">
                <line x1="8" y1="6" x2="24" y2="26" stroke="rgba(0,0,0,0.3)" stroke-width="5" stroke-linecap="round"/>
                <line x1="8" y1="5" x2="24" y2="25" stroke="#94a3b8" stroke-width="5" stroke-linecap="round"/>
                <line x1="10" y1="8" x2="22" y2="22" stroke="rgba(255,255,255,0.5)" stroke-width="2" stroke-linecap="round"/>
            </svg>
        }
        .into_any(),
        Piece::Arrow(dir) => {
            // A corner of two arrows, each pointing outward from the centre.
            // 0 = up, 90 = right, 180 = down, 270 = left.
            let (d1, d2): (i32, i32) = match dir {
                ArrowDir::UpRight => (0, 90),
                ArrowDir::DownRight => (180, 90),
                ArrowDir::DownLeft => (180, 270),
                ArrowDir::UpLeft => (0, 270),
            };
            // One arrow pointing up from the centre; rotated per direction.
            let arrow = |deg: i32| {
                view! {
                    <g transform=format!("rotate({deg} 16 16)")>
                        <line
                            x1="16" y1="16" x2="16" y2="6"
                            stroke="#94a3b8" stroke-width="3" stroke-linecap="round"
                        />
                        <polyline
                            points="12,10 16,5 20,10"
                            fill="none" stroke="#94a3b8" stroke-width="3"
                            stroke-linecap="round" stroke-linejoin="round"
                        />
                    </g>
                }
            };
            view! {
                <svg viewBox="0 0 32 32" class="w-full h-full" xmlns="http://www.w3.org/2000/svg">
                    {arrow(d1)}
                    {arrow(d2)}
                </svg>
            }
            .into_any()
        }
    }
}

// ── Piece palette ─────────────────────────────────────────────────────────────

// One entry per piece TYPE — arrows and slashes cycle orientation on the board.
const PALETTE_PIECES: &[Piece] = &[
    Piece::Red,
    Piece::Yellow,
    Piece::Blue,
    Piece::Black,
    Piece::Slash(SlashDir::Forward),
    Piece::Arrow(ArrowDir::UpRight),
];

#[component]
fn PiecePalette(selected: RwSignal<Option<Piece>>) -> impl IntoView {
    view! {
        <div class="flex flex-wrap gap-2">
            {PALETTE_PIECES
                .iter()
                .copied()
                .map(|p| {
                    let is_sel = move || {
                        selected.with(|s| s.map(|sel| same_piece_type(sel, p)).unwrap_or(false))
                    };
                    view! {
                        <button
                            title=p.label()
                            class=move || {
                                format!(
                                    "w-12 h-12 rounded-xl p-1.5 border-2 touch-manipulation transition-all shadow-sm {}",
                                    if is_sel() {
                                        "border-blue-400 scale-110 ring-2 ring-blue-300 bg-stone-700"
                                    } else {
                                        "border-stone-500 bg-stone-700 hover:bg-stone-600 hover:border-stone-400"
                                    },
                                )
                            }
                            on:click=move |_| {
                                selected.update(|s| {
                                    *s = if s.map(|sel| same_piece_type(sel, p)).unwrap_or(false) {
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
                .collect_view()}
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

// ── Setup view ────────────────────────────────────────────────────────────────

#[component]
fn SetupView(state: RwSignal<GameUiState>) -> impl IntoView {
    let selected = RwSignal::new(Option::<Piece>::None);
    let locked = RwSignal::new(false);

    let on_lock = move |_| {
        let board = state.with_untracked(|s| s.your_board);
        if !logic::validate_board(&board) {
            state.update(|s| {
                s.error =
                    Some("Place one of each piece (and two slash mirrors) before locking.".into());
            });
            return;
        }
        locked.set(true);
        #[cfg(target_arch = "wasm32")]
        send_msg(&ClientMsg::LockBoard { board });
    };

    view! {
        <div class="space-y-5">
            <p class="text-stone-400 text-sm">
                "Pick a piece, then place it on the board. For a mirror, choose its orientation first. Click a placed piece to remove it."
            </p>

            <PiecePalette selected />
            <OrientationPicker selected />

            <div class="overflow-x-auto">
                <BoardGrid
                    board=Signal::derive(move || state.with(|s| s.your_board))
                    on_cell=move |r, c| {
                        if locked.get_untracked() {
                            return;
                        }
                        let sel = selected.get_untracked();
                        state.update(|s| {
                            let cell = s.your_board[r][c];
                            // Direct placement: same piece removes, anything else replaces.
                            s.your_board[r][c] = if cell == sel { None } else { sel };
                        });
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
                        view! {
                            <span class="text-sm text-green-400">"Opponent is ready!"</span>
                        }
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

// ── Play view ─────────────────────────────────────────────────────────────────

#[component]
fn PlayView(state: RwSignal<GameUiState>, your_turn: bool) -> impl IntoView {
    let guess_selected = RwSignal::new(Option::<Piece>::None);

    // The most recent ray each side fired — its entry and exit ports breathe
    // with the resulting color until the next shot.
    let my_highlight = Signal::derive(move || {
        state.with(|s| {
            s.shot_history
                .iter()
                .rev()
                .find(|sh| sh.mine)
                .map(|sh| (sh.call, sh.exit, sh.color))
        })
    });
    let opp_highlight = Signal::derive(move || {
        state.with(|s| {
            s.shot_history
                .iter()
                .rev()
                .find(|sh| !sh.mine)
                .map(|sh| (sh.call, sh.exit, sh.color))
        })
    });

    let on_shoot = move |port: u8| {
        #[cfg(target_arch = "wasm32")]
        send_msg(&ClientMsg::ShootRay { port });
        #[cfg(not(target_arch = "wasm32"))]
        let _ = port;
    };

    let on_guess_cell = move |r: usize, c: usize| {
        let sel = guess_selected.get_untracked();
        state.update(|s| {
            let cell = s.guess_board[r][c];
            // Direct placement: same piece removes, anything else replaces.
            s.guess_board[r][c] = if cell == sel { None } else { sel };
        });
    };

    let rotate_mode = RwSignal::new(false);
    // You may flip exactly one mirror, only on your own turn.
    let can_flip = move || your_turn && !state.with(|s| s.flip_used_this_turn);
    let on_guess_cell_or_rotate = move |r: usize, c: usize| {
        if rotate_mode.get_untracked() {
            if can_flip() {
                #[cfg(target_arch = "wasm32")]
                send_msg(&ClientMsg::RequestRotate {
                    row: r as u8,
                    col: c as u8,
                });
            }
            rotate_mode.set(false);
        } else {
            on_guess_cell(r, c);
        }
    };

    // You may only call the game on your own turn.
    let can_call = move || your_turn && state.with(|s| logic::validate_board(&s.guess_board));

    let on_call = move |_| {
        if can_call() {
            #[cfg(target_arch = "wasm32")]
            {
                let guess = state.with_untracked(|s| s.guess_board);
                send_msg(&ClientMsg::CallGame { guess });
            }
        }
    };

    view! {
        <div class="space-y-6">
            {move || {
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
            }}

            <div class="grid grid-cols-1 lg:grid-cols-2 gap-10">
                <div>
                    <h3 class="font-semibold mb-2 text-stone-300">"Your Board"</h3>
                    <BoardWithPorts
                        board=Signal::derive(move || state.with(|s| s.your_board))
                        on_port=|_| {}
                        port_active=false
                        on_cell=|_, _| {}
                        cell_interactive=false
                        highlight=opp_highlight
                    />
                    <div class=if your_turn {
                        "mt-3 text-base font-semibold text-green-400"
                    } else {
                        "mt-3 text-base font-semibold text-stone-500"
                    }>
                        {if your_turn { "Your turn. Shoot a ray!" } else { "Opponent's turn..." }}
                    </div>
                    {move || {
                        if let Some((row, col, accepted)) = state.with(|s| s.last_mirror_attempt) {
                            let (top, left) = cell_label(row, col);
                            let msg = if accepted {
                                format!(
                                    "Opponent found your mirror at ({top}, {left}) and flipped it!",
                                )
                            } else {
                                format!(
                                    "Opponent tried to flip ({top}, {left}). No mirror there.",
                                )
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
                            "Click a port to shoot a ray."
                        } else {
                            "Build your guess while opponent shoots."
                        }}
                    </p>
                    <BoardWithPorts
                        board=Signal::derive(move || state.with(|s| s.guess_board))
                        on_port=on_shoot
                        port_active=your_turn
                        on_cell=on_guess_cell_or_rotate
                        cell_interactive=true
                        highlight=my_highlight
                    />
                    <div class="mt-4 space-y-3">
                        // Guess board piece palette
                        <div class="flex flex-wrap gap-1.5">
                            {PALETTE_PIECES
                                .iter()
                                .copied()
                                .map(|p| {
                                    let is_sel = move || {
                                        guess_selected.with(|s| {
                                            s.map(|sel| same_piece_type(sel, p)).unwrap_or(false)
                                        })
                                    };
                                    view! {
                                        <button
                                            title=p.label()
                                            class=move || {
                                                format!(
                                                    "w-10 h-10 rounded-lg p-1 border-2 touch-manipulation transition-all {}",
                                                    if is_sel() {
                                                        "border-blue-400 scale-110 bg-stone-700"
                                                    } else {
                                                        "border-stone-600 bg-stone-800 hover:bg-stone-700"
                                                    },
                                                )
                                            }
                                            on:click=move |_| {
                                                guess_selected.update(|s| {
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
                                .collect_view()}
                        </div>
                        <OrientationPicker selected=guess_selected />

                        <div class="flex flex-wrap gap-2 items-center">
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
                                on:click=on_call
                                disabled=move || !can_call()
                            >
                                "Call Game!"
                            </button>
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
                                {move || {
                                    if state.with(|s| s.flip_used_this_turn) {
                                        "Flip used this turn"
                                    } else if !your_turn {
                                        "Flip mirror (your turn only)"
                                    } else if rotate_mode.get() {
                                        "Tap a cell to flip it"
                                    } else {
                                        "Flip a mirror"
                                    }
                                }}
                            </button>
                        </div>

                        {move || {
                            if state.with(|s| s.last_guess_wrong) {
                                view! {
                                    <p class="text-sm text-red-400">"Wrong guess, game continues."</p>
                                }
                                    .into_any()
                            } else {
                                view! { <span /> }.into_any()
                            }
                        }}
                        {move || {
                            if let Some((row, col, accepted)) = state.with(|s| s.last_rotate_result) {
                                let (top, left) = cell_label(row, col);
                                let msg = if accepted {
                                    format!(
                                        "Correct! You found a mirror at ({top}, {left}) and flipped it.",
                                    )
                                } else {
                                    format!(
                                        "No mirror at ({top}, {left}). You lose your turn.",
                                    )
                                };
                                let cls = if accepted {
                                    "text-sm text-green-400"
                                } else {
                                    "text-sm text-stone-500"
                                };
                                view! { <p class=cls>{msg}</p> }.into_any()
                            } else {
                                view! { <span /> }.into_any()
                            }
                        }}
                    </div>
                </div>
            </div>

            <ShotHistory state />
        </div>
    }
}

// ── Game over view ────────────────────────────────────────────────────────────

#[component]
fn GameOverView(state: RwSignal<GameUiState>, you_win: bool) -> impl IntoView {
    let your_board = Signal::derive(move || state.with(|s| s.your_board));
    let guess_board = Signal::derive(move || state.with(|s| s.guess_board));
    let answer_board = Signal::derive(move || state.with(|s| s.opponent_board.unwrap_or_default()));
    view! {
        <div class="space-y-8">
            <div class="text-center space-y-3">
                <div class=if you_win {
                    "text-4xl font-bold text-green-500"
                } else {
                    "text-4xl font-bold text-red-500"
                }>{if you_win { "You Win!" } else { "You Lose." }}</div>
                <a
                    href="/games/rays-of-light"
                    class="inline-block rounded bg-blue-500 px-6 py-2 text-white hover:bg-blue-600"
                >
                    "Play Again"
                </a>
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

// ── Single player mode ─────────────────────────────────────────────────────────

// Single player mode runs entirely in the browser: the computer rolls a random
// board and the player deduces it by shooting rays, with no server or opponent
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

#[component]
pub fn SinglePlayerInner() -> impl IntoView {
    // A shared link drops you onto its exact board; otherwise roll a fresh one.
    let answer = RwSignal::new(board_from_url().unwrap_or_else(logic::random_board));
    let guess_board = RwSignal::new(Board::default());
    // One ordered history of everything that costs a ray: real shots, wrong
    // guesses, and wasted flips. The score is simply this log's length.
    let log = RwSignal::new(Vec::<SoloEvent>::new());
    let guess_selected = RwSignal::new(Option::<Piece>::None);
    // The round ends when the player either solves it or reveals the answer.
    let solved = RwSignal::new(false);
    let revealed = RwSignal::new(false);
    let last_guess_wrong = RwSignal::new(false);
    // Flip mechanic, mirroring multiplayer: you may flip one mirror per turn, and
    // shooting a ray ends the turn (refreshing the flip). `rotate_mode` arms the
    // next cell tap to attempt a flip instead of placing a guess piece.
    let rotate_mode = RwSignal::new(false);
    let flip_used = RwSignal::new(false);
    let last_flip = RwSignal::new(Option::<(u8, u8, bool)>::None);

    let over = move || solved.get() || revealed.get();
    let can_flip = move || !over() && !flip_used.get();

    let on_shoot = move |port: u8| {
        if over() {
            return;
        }
        last_guess_wrong.set(false);
        last_flip.set(None);
        rotate_mode.set(false);
        let (exit, color) = logic::trace_ray(&answer.get_untracked(), port);
        log.update(|v| {
            v.push(SoloEvent::Ray(Shot {
                call: port,
                exit,
                color,
                mine: true,
            }))
        });
        // Shooting ends the turn — a fresh flip is available next turn.
        flip_used.set(false);
    };

    let place_guess = move |r: usize, c: usize| {
        let sel = guess_selected.get_untracked();
        guess_board.update(|b| {
            b[r][c] = if b[r][c] == sel { None } else { sel };
        });
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
                    if let Some(p) = b[r][c] {
                        if logic::is_slash(p) {
                            b[r][c] = Some(logic::rotate_piece(p));
                            accepted = true;
                        }
                    }
                });
                flip_used.set(true);
                // Flipping a cell with no mirror costs a ray — log it.
                if !accepted {
                    log.update(|v| v.push(SoloEvent::WastedFlip { row: r as u8, col: c as u8 }));
                }
                last_flip.set(Some((r as u8, c as u8, accepted)));
                last_guess_wrong.set(false);
            }
            rotate_mode.set(false);
        } else {
            place_guess(r, c);
        }
    };

    let can_call = move || guess_board.with(logic::validate_board);

    let on_call = move |_| {
        if !can_call() {
            return;
        }
        if logic::boards_match(&guess_board.get_untracked(), &answer.get_untracked()) {
            solved.set(true);
        } else {
            last_guess_wrong.set(true);
            // A wrong guess counts as a ray shot — log it.
            log.update(|v| v.push(SoloEvent::WrongGuess));
        }
    };

    let new_round = move |_| {
        // Drop any ?b= share code so a fresh board isn't replaced on reload.
        #[cfg(target_arch = "wasm32")]
        if let Some(history) = web_sys::window().and_then(|w| w.history().ok()) {
            let _ = history.replace_state_with_url(
                &wasm_bindgen::JsValue::NULL,
                "",
                Some("/games/rays-of-light/single-player"),
            );
        }
        answer.set(logic::random_board());
        guess_board.set(Board::default());
        log.set(Vec::new());
        guess_selected.set(None);
        solved.set(false);
        revealed.set(false);
        last_guess_wrong.set(false);
        rotate_mode.set(false);
        flip_used.set(false);
        last_flip.set(None);
    };

    let log_sig = Signal::derive(move || log.get());
    // The last ray fired breathes its entry and exit ports with the result color.
    let highlight = Signal::derive(move || {
        log.with(|v| {
            v.iter().rev().find_map(|e| match e {
                SoloEvent::Ray(s) => Some((s.call, s.exit, s.color)),
                _ => None,
            })
        })
    });

    view! {
        <div class="max-w-4xl mx-auto space-y-6">
            <div class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                <div class="flex items-center gap-2">
                    <h1 class="text-2xl font-bold dark:text-white">"Rays of Light Single Player"</h1>
                    <RulesButton />
                </div>
                <div class="flex items-center gap-2 shrink-0">
                    <a
                        href="/games/rays-of-light"
                        class="rounded-xl bg-stone-700 px-4 py-2 text-sm font-semibold text-stone-200 hover:bg-stone-600 touch-manipulation shadow"
                    >
                        "Back to Lobby"
                    </a>
                    <button
                        class="rounded-xl bg-stone-700 px-4 py-2 text-sm font-semibold text-stone-200 hover:bg-stone-600 touch-manipulation shadow"
                        on:click=new_round
                    >
                        "New Board"
                    </button>
                </div>
            </div>

            <p class="text-sm text-stone-400">
                "The computer hid a board. Shoot rays through the ports and read the exit port \
                and color to deduce where the pieces are. Build your guess on the board, then \
                call it."
            </p>

            {move || {
                let n = log.with(|v| v.len());
                let rays = if n == 1 { "1 ray".to_string() } else { format!("{n} rays") };
                if solved.get() {
                    view! {
                        <div class="rounded-xl border border-green-700 bg-green-900/30 px-4 py-3 text-green-300 font-semibold">
                            {format!("Solved it in {rays}! You found the board.")}
                        </div>
                    }
                        .into_any()
                } else if revealed.get() {
                    view! {
                        <div class="rounded-xl border border-stone-600 bg-stone-800 px-4 py-3 text-stone-300 font-semibold">
                            {format!("Here's the board the computer hid. You shot {rays}.")}
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
                            // Round finished — reveal the real board for comparison.
                            view! {
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
                                    <div class="flex flex-wrap gap-1.5">
                                        {PALETTE_PIECES
                                            .iter()
                                            .copied()
                                            .map(|p| {
                                                let is_sel = move || {
                                                    guess_selected
                                                        .with(|s| {
                                                            s.map(|sel| same_piece_type(sel, p)).unwrap_or(false)
                                                        })
                                                };
                                                view! {
                                                    <button
                                                        title=p.label()
                                                        class=move || {
                                                            format!(
                                                                "w-10 h-10 rounded-lg p-1 border-2 touch-manipulation transition-all {}",
                                                                if is_sel() {
                                                                    "border-blue-400 scale-110 bg-stone-700"
                                                                } else {
                                                                    "border-stone-600 bg-stone-800 hover:bg-stone-700"
                                                                },
                                                            )
                                                        }
                                                        on:click=move |_| {
                                                            guess_selected
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
                                            .collect_view()}
                                    </div>
                                    <OrientationPicker selected=guess_selected />
                                    <div class="flex flex-wrap gap-2 items-center">
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
                                            on:click=on_call
                                            disabled=move || !can_call()
                                        >
                                            "Call Game!"
                                        </button>
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
                                            {move || {
                                                if flip_used.get() {
                                                    "Flip used (shoot to refresh)"
                                                } else if rotate_mode.get() {
                                                    "Tap a cell to flip it"
                                                } else {
                                                    "Flip a mirror"
                                                }
                                            }}
                                        </button>
                                        <button
                                            class="min-h-[44px] rounded-xl bg-stone-700 px-3 py-2 text-sm text-stone-300 hover:bg-stone-600 touch-manipulation shadow"
                                            on:click=move |_| revealed.set(true)
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
                                            let (top, left) = cell_label(row, col);
                                            let (msg, cls) = if accepted {
                                                (
                                                    format!("Mirror at ({top}, {left}) flipped!"),
                                                    "text-sm text-green-400",
                                                )
                                            } else {
                                                (
                                                    format!(
                                                        "No mirror at ({top}, {left}). That costs a ray.",
                                                    ),
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
                    <SoloLog log=log_sig title="History" />
                </div>
            </div>

            {move || {
                if over() {
                    view! {
                        <div class="flex flex-wrap items-start gap-3">
                            <button
                                class="rounded-xl bg-blue-500 px-6 py-2 text-white font-semibold hover:bg-blue-600 touch-manipulation shadow-md"
                                on:click=new_round
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

// Lets the player turn the finished board into a shareable single player link. The
// board is encoded into an opaque `?b=` code so the answer can't be read at a
// glance from the URL.
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
            let url = format!("{origin}/games/rays-of-light/single-player?b={code}");
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
            link.set(Some(format!("/games/rays-of-light/single-player?b={code}")));
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
                // Only shown when the clipboard wasn't available, so the player
                // can still copy the link by hand.
                link.get()
                    .map(|url| {
                        view! {
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

// ── Shot history ──────────────────────────────────────────────────────────────

#[component]
fn ShotHistory(state: RwSignal<GameUiState>) -> impl IntoView {
    let mine = Signal::derive(move || {
        state.with(|s| {
            s.shot_history
                .iter()
                .filter(|sh| sh.mine)
                .cloned()
                .collect::<Vec<_>>()
        })
    });
    let theirs = Signal::derive(move || {
        state.with(|s| {
            s.shot_history
                .iter()
                .filter(|sh| !sh.mine)
                .cloned()
                .collect::<Vec<_>>()
        })
    });
    view! {
        <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
            <ShotTable shots=mine title="Your Shots" />
            <ShotTable shots=theirs title="Opponent's Shots" />
        </div>
    }
}

#[component]
fn ShotTable(shots: Signal<Vec<Shot>>, title: &'static str) -> impl IntoView {
    let has_shots = Signal::derive(move || shots.with(|s| !s.is_empty()));
    // Newest first, but keyed by each shot's stable original index so only the
    // freshly added row mounts (and animates) — the existing rows stay put.
    let rows = Signal::derive(move || {
        let mut indexed: Vec<(usize, Shot)> = shots.with(|s| s.iter().cloned().enumerate().collect());
        indexed.reverse();
        indexed
    });
    view! {
        <div>
            <h3 class="font-semibold mb-2 text-stone-300">{title}</h3>
            <Show
                when=move || has_shots.get()
                fallback=|| view! { <p class="text-xs text-stone-500">"No shots yet."</p> }
            >
                <div class="overflow-x-auto rounded-xl bg-stone-800 ring-1 ring-stone-700">
                    <table class="w-full text-sm border-collapse">
                        <thead>
                            <tr class="text-left text-stone-500 text-xs uppercase border-b border-stone-700">
                                <th class="px-4 py-2">"In"</th>
                                <th class="px-4 py-2">"Out"</th>
                                <th class="px-4 py-2">"Color"</th>
                            </tr>
                        </thead>
                        <tbody>
                            <For each=move || rows.get() key=|(idx, _)| *idx let:row>
                                {
                                    let shot = row.1;
                                    let call = shot.call;
                                    let exit = shot.exit;
                                    let color_css = shot.color.css_class();
                                    let color_name = shot.color.name();
                                    view! {
                                        <tr class="row-appear border-t border-stone-700">
                                            <td class="px-4 py-2 font-mono text-stone-300">{call}</td>
                                            <td class="px-4 py-2 font-mono text-stone-300">{exit}</td>
                                            <td class="px-4 py-2">
                                                <span class=format!(
                                                    "inline-block px-2 py-0.5 rounded text-xs font-semibold {color_css}",
                                                )>{color_name}</span>
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

/// Single-player history: rays plus the mistakes that also cost a ray (wrong
/// guesses and wasted flips), newest first, so the score is fully accounted for.
#[component]
fn SoloLog(log: Signal<Vec<SoloEvent>>, title: &'static str) -> impl IntoView {
    let has_rows = Signal::derive(move || log.with(|v| !v.is_empty()));
    // Newest first, keyed by stable original index so only the freshly added
    // row mounts (and animates) — existing rows stay put.
    let rows = Signal::derive(move || {
        let mut indexed: Vec<(usize, SoloEvent)> =
            log.with(|v| v.iter().cloned().enumerate().collect());
        indexed.reverse();
        indexed
    });
    view! {
        <div>
            <h3 class="font-semibold mb-2 text-stone-300">{title}</h3>
            <Show
                when=move || has_rows.get()
                fallback=|| view! { <p class="text-xs text-stone-500">"No moves yet."</p> }
            >
                <div class="overflow-x-auto rounded-xl bg-stone-800 ring-1 ring-stone-700">
                    <table class="w-full text-sm border-collapse">
                        <thead>
                            <tr class="text-left text-stone-500 text-xs uppercase border-b border-stone-700">
                                <th class="px-4 py-2">"In"</th>
                                <th class="px-4 py-2">"Out"</th>
                                <th class="px-4 py-2">"Result"</th>
                            </tr>
                        </thead>
                        <tbody>
                            <For each=move || rows.get() key=|(idx, _)| *idx let:row>
                                {
                                    // Each event maps to (In, Out, result badge text, badge css).
                                    let (in_cell, out_cell, result, badge_css) = match row.1 {
                                        SoloEvent::Ray(shot) => (
                                            shot.call.to_string(),
                                            shot.exit.to_string(),
                                            shot.color.name().to_string(),
                                            shot.color.css_class(),
                                        ),
                                        SoloEvent::WrongGuess => (
                                            "Guess".to_string(),
                                            "—".to_string(),
                                            "Wrong".to_string(),
                                            "bg-stone-700 text-stone-300",
                                        ),
                                        SoloEvent::WastedFlip { row, col } => {
                                            let (top, left) = cell_label(row, col);
                                            (
                                            format!("Flip ({top}, {left})"),
                                            "—".to_string(),
                                            "No mirror".to_string(),
                                            "bg-stone-700 text-stone-300",
                                        )
                                        }
                                    };
                                    view! {
                                        <tr class="row-appear border-t border-stone-700">
                                            <td class="px-4 py-2 font-mono text-stone-300">{in_cell}</td>
                                            <td class="px-4 py-2 font-mono text-stone-300">{out_cell}</td>
                                            <td class="px-4 py-2">
                                                <span class=format!(
                                                    "inline-block px-2 py-0.5 rounded text-xs font-semibold {badge_css}",
                                                )>{result}</span>
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

// ── Board cell ────────────────────────────────────────────────────────────────

fn cell_view(
    cell: impl Fn() -> Option<Piece> + Copy + Send + Sync + 'static,
    interactive: bool,
    on_click: impl Fn() + Send + Sync + 'static,
) -> impl IntoView {
    view! {
        <button
            class=move || {
                let base = "w-14 h-14 rounded-xl flex items-center justify-center border-2 touch-manipulation transition-all p-1.5";
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
            on:click=move |_| {
                if interactive {
                    on_click();
                }
            }
        >
            {move || {
                if let Some(p) = cell() {
                    view! { <div class="w-full h-full piece-pop">{piece_icon(p)}</div> }
                        .into_any()
                } else {
                    view! { <span class="text-stone-600 text-lg select-none">"·"</span> }
                        .into_any()
                }
            }}
        </button>
    }
}

// ── Flat 4×4 grid ─────────────────────────────────────────────────────────────

#[component]
fn BoardGrid(
    board: Signal<Board>,
    on_cell: impl Fn(usize, usize) + Copy + Send + Sync + 'static,
    interactive: bool,
) -> impl IntoView {
    view! {
        <div class="inline-block bg-stone-900 p-2.5 rounded-2xl shadow-2xl ring-1 ring-stone-700">
            <div class="inline-grid grid-cols-4 gap-1.5">
                {(0..4usize)
                    .flat_map(|r| (0..4usize).map(move |c| (r, c)))
                    .map(|(r, c)| {
                        let cell = move || board.with(|b| b[r][c]);
                        cell_view(cell, interactive, move || on_cell(r, c))
                    })
                    .collect_view()}
            </div>
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

#[component]
fn BoardWithPorts(
    board: Signal<Board>,
    on_port: impl Fn(u8) + Copy + Send + Sync + 'static,
    port_active: bool,
    on_cell: impl Fn(usize, usize) + Copy + Send + Sync + 'static,
    cell_interactive: bool,
    /// The most recent ray to highlight: (entry port, exit port, color). The two
    /// ports breathe with the color until a new ray replaces it.
    #[prop(optional)]
    highlight: Signal<Option<(u8, u8, Color)>>,
) -> impl IntoView {
    // Is this port the entry or exit of the highlighted ray, and if so, in what
    // color should it glow?
    let lit_color = move |p: u8| {
        highlight.with(|h| match h {
            Some((call, exit, color)) if *call == p || *exit == p => Some(*color),
            _ => None,
        })
    };

    let port_btn = move |p: u8| {
        view! {
            <button
                class=move || {
                    let base = "w-9 h-9 rounded-full text-xs font-bold touch-manipulation transition-all";
                    if let Some(color) = lit_color(p) {
                        // A used port glows with the ray's color, on top of the
                        // normal active/idle styling.
                        format!("{base} ray-breathe {}", color.css_class())
                    } else if port_active {
                        format!(
                            "{base} bg-blue-500 text-white hover:bg-blue-400 shadow-md shadow-blue-900",
                        )
                    } else {
                        format!("{base} bg-stone-700 text-stone-400 cursor-default")
                    }
                }
                style=move || {
                    lit_color(p)
                        .map(|color| format!("--ray-color: {}", color.glow()))
                        .unwrap_or_default()
                }
                on:click=move |_| {
                    if port_active {
                        on_port(p);
                    }
                }
            >
                {p}
            </button>
        }
    };

    // Wrap a port button in a cell-sized box so it lines up with its grid column / row.
    let col_port = move |p: u8| view! { <div class="w-14 flex justify-center">{port_btn(p)}</div> };
    let row_port = move |p: u8| view! { <div class="h-14 flex items-center">{port_btn(p)}</div> };

    view! {
        <div class="inline-block select-none overflow-x-auto max-w-full">
            <div class="inline-block bg-stone-900 p-2.5 rounded-2xl shadow-2xl ring-1 ring-stone-700">
                // Top ports: 1 2 3 4
                <div class="flex items-center mb-1.5 gap-1.5">
                    <div class="w-9 shrink-0"></div>
                    <div class="grid grid-cols-4 gap-1.5">
                        {[1u8, 2, 3, 4].into_iter().map(col_port).collect_view()}
                    </div>
                    <div class="w-9 shrink-0"></div>
                </div>
                <div class="flex items-start gap-1.5">
                    // Left ports: 16 15 14 13
                    <div class="grid grid-rows-4 gap-1.5">
                        {[16u8, 15, 14, 13].into_iter().map(row_port).collect_view()}
                    </div>
                    // Grid cells
                    <div class="inline-grid grid-cols-4 gap-1.5">
                        {(0..4usize)
                            .flat_map(|r| (0..4usize).map(move |c| (r, c)))
                            .map(|(r, c)| {
                                let cell = move || board.with(|b| b[r][c]);
                                cell_view(cell, cell_interactive, move || on_cell(r, c))
                            })
                            .collect_view()}
                    </div>
                    // Right ports: 5 6 7 8
                    <div class="grid grid-rows-4 gap-1.5">
                        {[5u8, 6, 7, 8].into_iter().map(row_port).collect_view()}
                    </div>
                </div>
                // Bottom ports: 12 11 10 9
                <div class="flex items-center mt-1.5 gap-1.5">
                    <div class="w-9 shrink-0"></div>
                    <div class="grid grid-cols-4 gap-1.5">
                        {[12u8, 11, 10, 9].into_iter().map(col_port).collect_view()}
                    </div>
                    <div class="w-9 shrink-0"></div>
                </div>
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
    let mut b: Board = Default::default();
    b[0][2] = Some(Piece::Blue);
    b[0][3] = Some(Piece::Arrow(ArrowDir::UpRight));
    b[1][1] = Some(Piece::Red);
    b[1][3] = Some(Piece::Yellow);
    b[2][0] = Some(Piece::Black);
    b[3][2] = Some(Piece::Slash(SlashDir::Forward));
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

/// A round "?" book button that opens the rules overlay. Self-contained — drop
/// it anywhere (lobby, single player, game room) to give players a rule book.
#[component]
pub fn RulesButton() -> impl IntoView {
    let show = RwSignal::new(false);
    view! {
        <button
            title="How to play"
            aria-label="How to play"
            on:click=move |_| show.set(true)
            class="relative flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-stone-100 text-stone-600 hover:bg-stone-200 dark:bg-stone-700 dark:text-stone-200 dark:hover:bg-stone-600 transition-colors shadow-sm"
        >
            <svg
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
                class="h-5 w-5"
            >
                <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z" />
            </svg>
            <span class="absolute inset-0 flex items-center justify-center text-[10px] font-bold leading-none">
                "?"
            </span>
        </button>
        <RulesOverlay show=show />
    }
}

#[component]
fn RulesOverlay(show: RwSignal<bool>) -> impl IntoView {
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

    move || {
        if !show.get() {
            return view! { <span /> }.into_any();
        }
        view! {
            <div
                class="fixed inset-0 z-50 flex items-start justify-center overflow-y-auto bg-black/60 p-4 backdrop-blur-sm"
                on:click=move |_| show.set(false)
            >
                <div
                    class="my-6 w-full max-w-2xl rounded-2xl bg-stone-800 text-stone-200 shadow-2xl ring-1 ring-stone-700"
                    on:click=move |ev| ev.stop_propagation()
                >
                    // Header — sticky so the close button is always reachable.
                    <div class="sticky top-0 flex items-center justify-between rounded-t-2xl border-b border-stone-700 bg-stone-800/95 px-5 py-3 backdrop-blur">
                        <h2 class="text-lg font-bold text-white">"How to play Rays of Light"</h2>
                        <button
                            title="Close"
                            aria-label="Close"
                            on:click=move |_| show.set(false)
                            class="flex h-8 w-8 items-center justify-center rounded-full text-stone-400 hover:bg-stone-700 hover:text-white"
                        >
                            <svg
                                viewBox="0 0 24 24"
                                fill="none"
                                stroke="currentColor"
                                stroke-width="2"
                                stroke-linecap="round"
                                class="h-5 w-5"
                            >
                                <line x1="6" y1="6" x2="18" y2="18" />
                                <line x1="18" y1="6" x2="6" y2="18" />
                            </svg>
                        </button>
                    </div>

                    <div class="space-y-6 px-5 py-5 text-sm leading-relaxed">
                        <section class="space-y-2">
                            <h3 class="font-semibold text-white">"Goal"</h3>
                            <p>
                                "You and your opponent each hide pieces on a 4×4 grid. Shoot rays of "
                                "light through the ports and read where they exit and what color they "
                                "become to deduce the hidden layout. Be the first to call your "
                                "opponent's entire board correctly."
                            </p>
                        </section>

                        <section class="space-y-2">
                            <h3 class="font-semibold text-white">"Ports"</h3>
                            <p>
                                "Sixteen numbered ports ring the grid. Light enters one and exits "
                                "another. You call a port, and your opponent answers with the exit "
                                "port and the color."
                            </p>
                        </section>

                        <section class="space-y-3">
                            <h3 class="font-semibold text-white">"The pieces"</h3>
                            <p>
                                "Each player places seven pieces: the three colored glasses, the "
                                "black absorber, an arrow mirror, and two slash mirrors."
                            </p>
                            <div class="grid grid-cols-2 gap-3 sm:grid-cols-3">
                                {legend_item(Piece::Red, "Red glass")}
                                {legend_item(Piece::Yellow, "Yellow glass")}
                                {legend_item(Piece::Blue, "Blue glass")}
                                {legend_item(Piece::Black, "Black absorbs light")}
                                {legend_item(Piece::Slash(SlashDir::Forward), "Slash mirror (turns 90°)")}
                                {legend_item(Piece::Arrow(ArrowDir::UpRight), "Arrow mirror")}
                            </div>
                            <ul class="ml-4 list-disc space-y-1 text-stone-300">
                                <li>
                                    <span class="font-medium text-white">"Colored glass"</span>
                                    " tints the ray. Colors mix as they stack (see below)."
                                </li>
                                <li>
                                    <span class="font-medium text-white">"Black"</span>
                                    " absorbs the ray. Any ray touching it exits black, no matter "
                                    "what else it passed through."
                                </li>
                                <li>
                                    <span class="font-medium text-white">"Slash mirror"</span>
                                    " (/ or \\) bends light 90°. A flip can rotate it during play."
                                </li>
                                <li>
                                    <span class="font-medium text-white">"Arrow mirror"</span>
                                    " is a corner of two arrows. A ray hitting an arrow point "
                                    "head-on bounces straight back; rays from the other directions "
                                    "pass straight through. Its orientation is fixed at setup."
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
                                "Here is a filled board. Try following each ray below to see how the "
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
                                    "Optionally "
                                    <span class="font-medium text-white">"flip"</span>
                                    " one cell on the opponent's board. If it holds a slash mirror "
                                    "it rotates (/ ↔ \\). They're told which cell you flipped."
                                </li>
                                <li>
                                    <span class="font-medium text-white">"Shoot a ray"</span>
                                    " through a port. The exit port and color are revealed. This "
                                    "ends your turn."
                                </li>
                                <li>
                                    "When you're sure, "
                                    <span class="font-medium text-white">"call the game"</span>
                                    " with your full guess. An exact match wins; otherwise play "
                                    "continues."
                                </li>
                            </ul>
                        </section>

                        <div class="flex justify-end pt-1">
                            <button
                                on:click=move |_| show.set(false)
                                class="rounded-xl bg-blue-500 px-5 py-2 font-semibold text-white hover:bg-blue-600 touch-manipulation"
                            >
                                "Got it"
                            </button>
                        </div>
                    </div>
                </div>
            </div>
        }
            .into_any()
    }
}
