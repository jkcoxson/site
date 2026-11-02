// Jackson Coxson
// Five Crowns: the rummy-style card game where the wild card changes every
// round. Rules: https://www.playmonster.com/wp-content/uploads/2019/09/5Crowns_RULES.pdf

#[cfg(target_arch = "wasm32")]
mod fx;
pub mod logic;
#[cfg(feature = "ssr")]
pub mod server;

use leptos::prelude::*;
use leptos_router::hooks::{use_navigate, use_params};
use leptos_router::{components::A, params::Params};

use crate::games::{GameGate, RoomInfo};
use logic::{
    best_meld, hand_size, rank_label, sort_for_display, Action, ActionKind, Card, ClientMsg,
    GameView, Meld, RoundResult, SeatInfo, ServerMsg, Stage, Standing, Suit, MIN_PLAYERS, ROUNDS,
};

// ── Server functions ──────────────────────────────────────────────────────────

#[server(CreateFiveCrownsRoom)]
pub async fn create_room(name: String, passcode: String) -> Result<String, ServerFnError> {
    use crate::context::Context;
    use rand::RngExt;
    if name.trim().is_empty() {
        return Err(ServerFnError::ServerError("Room name is required.".into()));
    }
    if passcode.trim().is_empty() {
        return Err(ServerFnError::ServerError("Passcode is required.".into()));
    }
    let id = {
        let mut rng = rand::rng();
        let chars: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
        (0..6)
            .map(|_| chars[rng.random_range(0..chars.len())] as char)
            .collect::<String>()
    };
    let ctx = use_context::<Context>().ok_or_else(|| ServerFnError::new("No context"))?;
    ctx.game_state
        .create_five_crowns_room(id.clone(), name, passcode)
        .await;
    Ok(id)
}

#[server(FiveCrownsRoomExists)]
pub async fn room_exists(code: String) -> Result<bool, ServerFnError> {
    use crate::context::Context;
    let ctx = use_context::<Context>().ok_or_else(|| ServerFnError::new("No context"))?;
    Ok(ctx.game_state.five_crowns_room_exists(&code).await)
}

#[server(ListFiveCrownsRooms)]
pub async fn list_rooms() -> Result<Vec<RoomInfo>, ServerFnError> {
    use crate::context::Context;
    let ctx = use_context::<Context>().ok_or_else(|| ServerFnError::new("No context"))?;
    let rooms = ctx.game_state.list_five_crowns_rooms().await;
    Ok(rooms
        .into_iter()
        .map(|r| RoomInfo {
            id: r.id,
            name: r.name,
            players: r.players,
            capacity: r.capacity,
            in_progress: r.in_progress,
            mode: r.mode,
        })
        .collect())
}

// ── Cards ─────────────────────────────────────────────────────────────────────

fn suit_color(suit: Option<Suit>) -> &'static str {
    match suit {
        Some(Suit::Stars) => "text-yellow-500",
        Some(Suit::Hearts) => "text-red-600",
        Some(Suit::Clubs) => "text-green-600",
        Some(Suit::Spades) => "text-stone-900",
        Some(Suit::Diamonds) => "text-blue-600",
        None => "text-purple-600",
    }
}

/// A full-size card for the hand and the discard pile. The rank and suit sit
/// in the top-left corner so they still show when cards overlap.
fn card_face(card: Card, wild: u8) -> impl IntoView {
    let ring = if card.is_wild(wild) {
        "ring-2 ring-amber-400"
    } else {
        "ring-1 ring-stone-300"
    };
    let symbol = card.suit.map(Suit::symbol).unwrap_or("♛");
    let corner = if card.is_joker() {
        "JK"
    } else {
        card.rank_label()
    };
    view! {
        <div class=format!(
            "fc-card {ring} {} relative select-none overflow-hidden rounded-lg bg-white font-bold shadow-md",
            suit_color(card.suit),
        )>
            <div class="absolute left-1 top-1 flex flex-col items-center leading-none">
                <span class="text-[0.95rem] tracking-tighter sm:text-lg">{corner}</span>
                <span class="text-xs sm:text-sm">{symbol}</span>
            </div>
            <div class="absolute inset-0 flex items-center justify-center pl-2 pt-3 text-2xl sm:text-3xl">
                {symbol}
            </div>
            {card
                .is_wild(wild)
                .then(|| {
                    view! {
                        <span class="absolute inset-x-0 bottom-0 bg-amber-400 text-center text-[0.55rem] font-bold leading-tight text-stone-900">
                            "WILD"
                        </span>
                    }
                })}
        </div>
    }
}

/// A small card for melds and results.
fn small_card(card: Card, wild: u8) -> impl IntoView {
    let ring = if card.is_wild(wild) {
        "ring-2 ring-amber-400"
    } else {
        "ring-1 ring-stone-300"
    };
    let symbol = card.suit.map(Suit::symbol).unwrap_or("♛");
    let rank = if card.is_joker() {
        "JK"
    } else {
        card.rank_label()
    };
    view! {
        <div class=format!(
            "h-11 w-8 {ring} {} flex select-none flex-col items-center justify-center rounded-md bg-white text-xs font-bold leading-none shadow",
            suit_color(card.suit),
        )>
            <span>{rank}</span>
            <span class="text-sm">{symbol}</span>
        </div>
    }
}

fn card_back(label: String) -> impl IntoView {
    view! {
        <div class="fc-card flex select-none flex-col items-center justify-center rounded-lg bg-gradient-to-br from-indigo-700 to-purple-800 text-amber-300 shadow-md ring-1 ring-indigo-400">
            <span class="text-2xl leading-none">"♛"</span>
            <span class="mt-1 text-[0.65rem] font-semibold text-indigo-100">{label}</span>
        </div>
    }
}

fn meld_view(meld: Meld, wild: u8) -> impl IntoView {
    view! {
        <div class="flex flex-wrap items-end gap-2">
            {meld
                .groups
                .into_iter()
                .map(|g| {
                    view! {
                        <div class="flex gap-0.5 rounded-lg bg-green-900/40 p-1 ring-1 ring-green-700">
                            {g.into_iter().map(|c| small_card(c, wild)).collect_view()}
                        </div>
                    }
                })
                .collect_view()}
            {(!meld.leftover.is_empty())
                .then(|| {
                    view! {
                        <div class="flex gap-0.5 rounded-lg bg-red-900/30 p-1 ring-1 ring-red-800">
                            {meld.leftover.into_iter().map(|c| small_card(c, wild)).collect_view()}
                        </div>
                    }
                })}
        </div>
    }
}

// ── Client state ──────────────────────────────────────────────────────────────

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
#[derive(Clone, PartialEq)]
enum Phase {
    Connecting,
    Reconnecting,
    Lobby {
        players: Vec<SeatInfo>,
        host: u8,
        you: u8,
    },
    Game(Box<GameView>),
    /// The server turned us away (no room, wrong passcode, full, started).
    Closed(String),
}

#[derive(Clone)]
struct UiState {
    phase: Phase,
    error: Option<String>,
}

/// Which screen to show. Kept apart from the data so the table isn't rebuilt
/// on every update, which would restart animations and drop a drag.
#[derive(Clone, PartialEq)]
enum Screen {
    Connecting,
    Reconnecting,
    Closed(String),
    Lobby,
    Game,
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn is_fatal(msg: &str) -> bool {
    matches!(
        msg,
        "Room not found." | "Wrong passcode." | "Room is full." | "Game already in progress."
    )
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
impl UiState {
    fn apply(&mut self, msg: ServerMsg) {
        match msg {
            ServerMsg::Welcome { .. } => self.error = None,
            ServerMsg::Lobby { players, host, you } => {
                self.phase = Phase::Lobby { players, host, you };
            }
            ServerMsg::Game(view) => {
                self.phase = Phase::Game(view);
                self.error = None;
            }
            ServerMsg::Error(e) if is_fatal(&e) => self.phase = Phase::Closed(e),
            ServerMsg::Error(e) => self.error = Some(e),
        }
    }

    /// The table move in `view` worth animating, if it's one we haven't seen.
    fn fresh_action(&self, view: &GameView) -> Option<Action> {
        let action = view.last?;
        let new = match &self.phase {
            Phase::Game(old) => old.last.is_some_and(|a| action.seq > a.seq),
            // The opening deal, straight from the waiting room.
            Phase::Lobby { .. } => action.kind == ActionKind::Deal,
            _ => false,
        };
        new.then_some(action)
    }
}

#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;

#[cfg(target_arch = "wasm32")]
thread_local! {
    static GAME_WS: RefCell<Option<web_sys::WebSocket>> = const { RefCell::new(None) };
}

fn send_msg(msg: &ClientMsg) {
    #[cfg(target_arch = "wasm32")]
    {
        let Ok(json) = serde_json::to_string(msg) else {
            return;
        };
        GAME_WS.with(|ws| {
            if let Some(ws) = ws.borrow().as_ref() {
                let _ = ws.send_with_str(&json);
            }
        });
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = msg;
}

#[cfg(target_arch = "wasm32")]
fn token_key(room_id: &str) -> String {
    format!("fc_token_{room_id}")
}

#[cfg(target_arch = "wasm32")]
fn storage() -> Option<web_sys::Storage> {
    web_sys::window().and_then(|w| w.local_storage().ok().flatten())
}

#[cfg(target_arch = "wasm32")]
fn schedule_reconnect(room_id: String, passcode: String, state: RwSignal<UiState>, delay_ms: i32) {
    wasm_bindgen_futures::spawn_local(async move {
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
fn connect_ws(room_id: String, passcode: String, state: RwSignal<UiState>, next_delay_ms: i32) {
    use wasm_bindgen::{closure::Closure, JsCast};
    use web_sys::{MessageEvent, WebSocket};

    let Some(window) = web_sys::window() else {
        return;
    };
    let location = window.location();
    let ws_proto = if location.protocol().unwrap_or_default() == "https:" {
        "wss"
    } else {
        "ws"
    };
    let host = location.host().unwrap_or_default();
    let token = storage()
        .and_then(|s| s.get_item(&token_key(&room_id)).ok().flatten())
        .map(|t| format!("?token={t}"))
        .unwrap_or_default();
    let url = format!("{ws_proto}://{host}/games/five-crowns/ws/{room_id}/{passcode}{token}");

    let Ok(ws) = WebSocket::new(&url) else {
        state.update(|s| s.error = Some("Failed to connect to game server.".into()));
        return;
    };

    let rid = room_id.clone();
    let onmessage = Closure::<dyn FnMut(MessageEvent)>::new(move |e: MessageEvent| {
        let Ok(text) = e.data().dyn_into::<js_sys::JsString>() else {
            return;
        };
        let Ok(msg) = serde_json::from_str::<ServerMsg>(&String::from(text)) else {
            return;
        };
        if let ServerMsg::Welcome { ref token, .. } = msg {
            if let Some(s) = storage() {
                let _ = s.set_item(&token_key(&rid), token);
            }
        }
        if let ServerMsg::Game(ref view) = msg {
            // Measure the table before it changes so the change can be animated.
            let action = state.with_untracked(|s| s.fresh_action(view));
            let you = view.you;
            let snap = fx::snapshot();
            state.update(|s| s.apply(msg));
            fx::play(snap, action, you, None);
            return;
        }
        state.update(|s| s.apply(msg));
    });
    ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();

    let onclose = Closure::<dyn FnMut(web_sys::CloseEvent)>::new(move |_| {
        // Nothing to rejoin once the server has turned us away.
        if state.with_untracked(|s| matches!(s.phase, Phase::Closed(_))) {
            if let Some(s) = storage() {
                let _ = s.remove_item(&token_key(&room_id));
            }
            return;
        }
        state.update(|s| s.phase = Phase::Reconnecting);
        schedule_reconnect(room_id.clone(), passcode.clone(), state, next_delay_ms);
    });
    ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
    onclose.forget();

    GAME_WS.with(|cell| *cell.borrow_mut() = Some(ws));
}

/// Stand-in for the table before the first update; never shown.
fn empty_view() -> GameView {
    GameView {
        you: 0,
        host: 0,
        round: 1,
        wild: 3,
        dealer: 0,
        current: 0,
        hand: Vec::new(),
        discard_top: None,
        deck_size: 0,
        players: Vec::new(),
        out: None,
        stage: Stage::Playing { drawn: false },
        log: Vec::new(),
        last: None,
    }
}

// ── Room ──────────────────────────────────────────────────────────────────────

#[component]
fn GameRoomInner(room_id: String, room_passcode: String) -> impl IntoView {
    let state = RwSignal::new(UiState {
        phase: Phase::Connecting,
        error: None,
    });

    #[cfg(target_arch = "wasm32")]
    {
        let rc = room_id.clone();
        let rp = room_passcode.clone();
        Effect::new(move |_| connect_ws(rc.clone(), rp.clone(), state, 1_000));
        on_cleanup(move || {
            GAME_WS.with(|cell| {
                if let Some(ws) = cell.borrow_mut().take() {
                    let _ = ws.close();
                }
            });
        });
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = room_passcode;

    let screen = Memo::new(move |_| {
        state.with(|s| match &s.phase {
            Phase::Connecting => Screen::Connecting,
            Phase::Reconnecting => Screen::Reconnecting,
            Phase::Closed(reason) => Screen::Closed(reason.clone()),
            Phase::Lobby { .. } => Screen::Lobby,
            Phase::Game(_) => Screen::Game,
        })
    });
    // The latest table, kept through a reconnect so the page doesn't jump.
    let game = Memo::new(move |prev: Option<&GameView>| {
        state.with(|s| match &s.phase {
            Phase::Game(view) => (**view).clone(),
            _ => prev.cloned().unwrap_or_else(empty_view),
        })
    });

    view! {
        <div class="mx-auto max-w-5xl">
            <div class="mb-3 flex items-center justify-between">
                <h1 class="text-xl font-bold sm:text-2xl dark:text-white">"Five Crowns"</h1>
                <span class="rounded bg-stone-100 px-3 py-1 font-mono text-sm dark:bg-stone-700 dark:text-white">
                    {room_id}
                </span>
            </div>
            {move || {
                state
                    .with(|s| s.error.clone())
                    .map(|err| {
                        view! {
                            <div class="mb-3 rounded border border-red-300 bg-red-100 px-4 py-2 text-sm text-red-800">
                                {err}
                            </div>
                        }
                    })
            }}
            {move || match screen.get() {
                Screen::Connecting => {
                    view! {
                        <div class="flex flex-col items-center justify-center gap-3 py-16 text-stone-500 dark:text-stone-400">
                            <div class="h-8 w-8 animate-spin rounded-full border-4 border-stone-600 border-t-amber-500"></div>
                            <p>"Joining the table…"</p>
                        </div>
                    }
                        .into_any()
                }
                Screen::Reconnecting => {
                    view! {
                        <div class="space-y-3 py-16 text-center">
                            <p class="text-lg dark:text-white">"Reconnecting..."</p>
                            <p class="text-sm text-stone-500 dark:text-stone-400">
                                "Connection lost. Your seat is saved."
                            </p>
                        </div>
                    }
                        .into_any()
                }
                Screen::Closed(reason) => {
                    view! {
                        <div class="space-y-3 py-16 text-center">
                            <p class="text-lg dark:text-white">"Couldn't join this game."</p>
                            <p class="text-sm text-stone-500 dark:text-stone-400">{reason}</p>
                            <A
                                href="/games/five-crowns"
                                attr:class="inline-block text-sm text-blue-600 hover:underline dark:text-blue-400"
                            >
                                "Back to the lobby"
                            </A>
                        </div>
                    }
                        .into_any()
                }
                Screen::Lobby => {
                    let lobby = move || {
                        state
                            .with(|s| match &s.phase {
                                Phase::Lobby { players, host, you } => {
                                    Some((players.clone(), *host, *you))
                                }
                                _ => None,
                            })
                            .map(|(players, host, you)| view! { <LobbyView players host you /> })
                    };
                    view! { {lobby} }.into_any()
                }
                Screen::Game => view! { <TableView game /> }.into_any(),
            }}
        </div>
    }
}

#[component]
fn LobbyView(players: Vec<SeatInfo>, host: u8, you: u8) -> impl IntoView {
    let my_name = players
        .iter()
        .find(|p| p.id == you)
        .map(|p| p.nickname.clone())
        .unwrap_or_default();
    let (name, set_name) = signal(my_name);
    let count = players.len();
    view! {
        <div class="mx-auto max-w-md space-y-6">
            <div class="rounded-lg border border-stone-200 bg-white p-5 dark:border-stone-700 dark:bg-stone-800">
                <h2 class="mb-3 text-lg font-semibold dark:text-stone-100">
                    {format!("Players ({count}/{})", logic::MAX_PLAYERS)}
                </h2>
                <ul class="space-y-2">
                    {players
                        .into_iter()
                        .map(|p| {
                            view! {
                                <li class="flex items-center justify-between rounded border border-stone-200 px-3 py-2 dark:border-stone-600">
                                    <span class="flex items-center gap-2 dark:text-stone-100">
                                        <span class=if p.connected {
                                            "h-2 w-2 rounded-full bg-green-500"
                                        } else {
                                            "h-2 w-2 rounded-full bg-stone-500"
                                        }></span>
                                        {p.nickname.clone()}
                                        {(p.id == you)
                                            .then(|| view! { <span class="text-xs text-stone-400">"(you)"</span> })}
                                    </span>
                                    {(p.id == host)
                                        .then(|| {
                                            view! {
                                                <span class="text-xs font-semibold text-amber-500">"Host"</span>
                                            }
                                        })}
                                </li>
                            }
                        })
                        .collect_view()}
                </ul>
            </div>
            <form
                class="flex gap-2"
                on:submit=move |ev| {
                    ev.prevent_default();
                    send_msg(&ClientMsg::SetNickname { name: name.get_untracked() });
                }
            >
                <input
                    type="text"
                    maxlength="20"
                    placeholder="Your name"
                    class="min-w-0 flex-1 rounded border border-stone-300 bg-white px-3 py-2 text-base dark:border-stone-600 dark:bg-stone-700 dark:text-white"
                    on:input=move |ev| set_name.set(event_target_value(&ev))
                    prop:value=move || name.get()
                />
                <button
                    type="submit"
                    class="touch-manipulation rounded bg-stone-600 px-4 py-2 text-sm text-white hover:bg-stone-500"
                >
                    "Rename"
                </button>
            </form>
            {if you == host {
                view! {
                    <button
                        class="w-full touch-manipulation rounded bg-blue-500 py-3 font-semibold text-white hover:bg-blue-600 disabled:cursor-not-allowed disabled:opacity-50"
                        disabled=count < MIN_PLAYERS
                        on:click=|_| send_msg(&ClientMsg::StartGame)
                    >
                        {if count < MIN_PLAYERS {
                            "Waiting for another player..."
                        } else {
                            "Deal the first round"
                        }}
                    </button>
                }
                    .into_any()
            } else {
                view! {
                    <p class="text-center text-sm text-stone-500 dark:text-stone-400">
                        "Waiting for the host to start..."
                    </p>
                }
                    .into_any()
            }}
            <p class="text-center text-xs text-stone-500">
                "Share the room code and passcode so friends can join. Up to seven players."
            </p>
        </div>
    }
}

// ── Table ─────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum StageKind {
    Playing,
    RoundOver,
    GameOver,
}

#[component]
fn TableView(game: Memo<GameView>) -> impl IntoView {
    let wild = Memo::new(move |_| game.with(|g| g.wild));
    let my_turn = Memo::new(move |_| {
        game.with(|g| g.current == g.you && matches!(g.stage, Stage::Playing { .. }))
    });
    let drawn =
        Memo::new(move |_| game.with(|g| matches!(g.stage, Stage::Playing { drawn: true })));
    let can_draw = move || my_turn.get() && !drawn.get();
    let stage = Memo::new(move |_| {
        game.with(|g| match g.stage {
            Stage::Playing { .. } => StageKind::Playing,
            Stage::RoundOver { .. } => StageKind::RoundOver,
            Stage::GameOver { .. } => StageKind::GameOver,
        })
    });

    let turn_text = move || {
        game.with(|g| {
            let name_of = |id: u8| {
                g.players
                    .iter()
                    .find(|p| p.id == id)
                    .map(|p| p.nickname.clone())
                    .unwrap_or_default()
            };
            match &g.stage {
                Stage::Playing { .. } if my_turn.get() && g.out.is_some() => format!(
                    "{} went out! Last turn: draw, then discard. The rest is laid down for you.",
                    name_of(g.out.unwrap_or_default())
                ),
                Stage::Playing { drawn: false } if my_turn.get() => {
                    "Your turn: draw from the deck or take the discard.".into()
                }
                Stage::Playing { .. } if my_turn.get() => {
                    "Pick a card to discard, or go out if the rest lays down.".into()
                }
                Stage::Playing { .. } => format!("{}'s turn.", name_of(g.current)),
                Stage::RoundOver { .. } => "Round over.".into(),
                Stage::GameOver { .. } => "Game over.".into(),
            }
        })
    };

    view! {
        <div class="fc-table grid gap-4 lg:grid-cols-[1fr_16rem]">
            <div class="min-w-0 space-y-3">
                <div class="flex flex-wrap items-center gap-2 text-sm">
                    <span class="rounded bg-stone-700 px-3 py-1 font-semibold text-white">
                        {move || format!("Round {}/{ROUNDS}", game.with(|g| g.round))}
                    </span>
                    <span class="rounded bg-amber-400 px-3 py-1 font-semibold text-stone-900">
                        {move || format!("{}s are wild", rank_label(wild.get()))}
                    </span>
                </div>

                <SeatStrip game />

                <div class="flex items-center gap-4 rounded-xl bg-green-900/60 p-3 ring-1 ring-green-800 sm:gap-6 sm:p-5">
                    <button
                        id="fc-deck"
                        class=move || {
                            if can_draw() { "fc-glow shrink-0 touch-manipulation" } else { "shrink-0 cursor-default" }
                        }
                        disabled=move || !can_draw()
                        on:click=|_| send_msg(&ClientMsg::Draw { from_discard: false })
                        title="Draw from the deck"
                    >
                        {move || card_back(format!("{} left", game.with(|g| g.deck_size)))}
                    </button>
                    <button
                        id="fc-discard"
                        class=move || {
                            if can_draw() && game.with(|g| g.discard_top.is_some()) {
                                "fc-glow shrink-0 touch-manipulation"
                            } else {
                                "shrink-0 cursor-default"
                            }
                        }
                        disabled=move || !(can_draw() && game.with(|g| g.discard_top.is_some()))
                        on:click=|_| send_msg(&ClientMsg::Draw { from_discard: true })
                        title="Take the top discard"
                    >
                        {move || match game.with(|g| g.discard_top) {
                            Some(c) => card_face(c, wild.get_untracked()).into_any(),
                            None => {
                                view! { <div class="fc-card rounded-lg border-2 border-dashed border-green-700"></div> }
                                    .into_any()
                            }
                        }}
                    </button>
                    <p class=move || {
                        if my_turn.get() {
                            "text-sm font-semibold text-amber-300"
                        } else {
                            "text-sm text-green-100"
                        }
                    }>{turn_text}</p>
                </div>

                {move || match stage.get() {
                    StageKind::Playing => view! { <HandView game my_turn drawn /> }.into_any(),
                    StageKind::RoundOver => {
                        let results_view = move || {
                            let g = game.get();
                            let Stage::RoundOver { results } = g.stage.clone() else {
                                return None;
                            };
                            Some(view! { <RoundResults view=g results /> })
                        };
                        view! { {results_view} }
                            .into_any()
                    }
                    StageKind::GameOver => {
                        let results_view = move || {
                            let g = game.get();
                            let Stage::GameOver { results, standings } = g.stage.clone() else {
                                return None;
                            };
                            Some(view! { <FinalResults view=g results standings /> })
                        };
                        view! { {results_view} }
                            .into_any()
                    }
                }}
            </div>

            <div class="rounded-lg border border-stone-200 bg-white p-3 text-xs lg:self-start dark:border-stone-700 dark:bg-stone-800">
                <h3 class="mb-2 font-semibold text-stone-500 uppercase">"Table talk"</h3>
                <ul class="space-y-1 text-stone-600 dark:text-stone-300">
                    {move || {
                        game.with(|g| {
                            g.log.iter().rev().map(|l| view! { <li>{l.clone()}</li> }).collect_view()
                        })
                    }}
                </ul>
            </div>
        </div>
    }
}

/// Everyone at the table with their running score; whose turn it is glows.
#[component]
fn SeatStrip(game: Memo<GameView>) -> impl IntoView {
    view! {
        <div class="-mx-1 flex gap-2 overflow-x-auto px-1 py-1">
            {move || {
                game.with(|g| {
                    let playing = matches!(g.stage, Stage::Playing { .. });
                    g.players
                        .iter()
                        .map(|p| {
                            let turn = playing && g.current == p.id;
                            let class = format!(
                                "shrink-0 rounded-lg px-3 py-1.5 text-sm transition-colors {} {}",
                                if turn {
                                    "bg-amber-400/20 ring-2 ring-amber-400"
                                } else {
                                    "bg-white ring-1 ring-stone-200 dark:bg-stone-800 dark:ring-stone-700"
                                },
                                if p.connected { "" } else { "opacity-50" },
                            );
                            view! {
                                <div data-fc-seat=p.id class=class>
                                    <div class="flex max-w-[9rem] items-center gap-1 font-semibold dark:text-stone-100">
                                        <span class="truncate">{p.nickname.clone()}</span>
                                        {(p.id == g.you)
                                            .then(|| view! { <span class="text-xs font-normal text-stone-400">"(you)"</span> })}
                                    </div>
                                    <div class="flex items-center gap-1.5 text-xs text-stone-500 dark:text-stone-400">
                                        <span>
                                            <span class="font-bold text-stone-800 dark:text-stone-100">{p.total}</span>
                                            " pts"
                                        </span>
                                        {(p.id == g.dealer)
                                            .then(|| view! { <span class="rounded bg-stone-600 px-1 text-[0.6rem] text-white">"dealer"</span> })}
                                        {(g.out == Some(p.id))
                                            .then(|| view! { <span class="rounded bg-amber-500 px-1 text-[0.6rem] font-semibold text-stone-900">"out"</span> })}
                                        {(!p.connected).then(|| view! { <span class="text-[0.6rem]">"away"</span> })}
                                    </div>
                                </div>
                            }
                        })
                        .collect_view()
                })
            }}
        </div>
    }
}

/// A card being dragged around the hand.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
#[derive(Clone, Copy)]
struct Drag {
    id: u8,
    pointer: i32,
    start_x: f64,
    start_y: f64,
    /// How far into the card the pointer went down.
    grab_dx: f64,
    moved: bool,
}

/// Put `hand` in the order of `ids`; cards not listed go last.
fn ordered(mut hand: Vec<Card>, ids: &[u8]) -> Vec<Card> {
    hand.sort_by_key(|c| ids.iter().position(|&i| i == c.id).unwrap_or(usize::MAX));
    hand
}

#[component]
fn HandView(game: Memo<GameView>, my_turn: Memo<bool>, drawn: Memo<bool>) -> impl IntoView {
    let selected = RwSignal::new(None::<u8>);
    // Our own order while dragging, and until the server has caught up.
    let local = RwSignal::new(None::<Vec<u8>>);
    let drag = StoredValue::new(None::<Drag>);
    let hand_ref = NodeRef::<leptos::html::Div>::new();
    let wild = move || game.with(|g| g.wild);

    let server_ids =
        Memo::new(move |_| game.with(|g| g.hand.iter().map(|c| c.id).collect::<Vec<u8>>()));
    Effect::new(move |_| {
        let ids = server_ids.get();
        if selected
            .get_untracked()
            .is_some_and(|id| !ids.contains(&id))
        {
            selected.set(None);
        }
        if drag.get_value().is_some_and(|d| d.moved) {
            return;
        }
        if let Some(mine) = local.get_untracked() {
            let mut a = mine.clone();
            let mut b = ids.clone();
            a.sort_unstable();
            b.sort_unstable();
            if mine == ids || a != b {
                local.set(None);
            }
        }
    });

    let cards = move || {
        let hand = game.with(|g| g.hand.clone());
        match local.get() {
            Some(ids) => ordered(hand, &ids),
            None => hand,
        }
    };

    // Reorder the hand here at once, then tell the server.
    let arrange = move |ids: Vec<u8>| {
        #[cfg(target_arch = "wasm32")]
        {
            let snap = fx::snapshot();
            let you = game.with_untracked(|g| g.you);
            local.set(Some(ids.clone()));
            fx::play(snap, None, you, None);
        }
        #[cfg(not(target_arch = "wasm32"))]
        local.set(Some(ids.clone()));
        send_msg(&ClientMsg::Arrange { order: ids });
    };
    let sort = move |by_rank: bool| {
        let w = wild();
        let mut hand = cards();
        if by_rank {
            hand.sort_by_key(|c| (c.is_wild(w), c.rank, c.suit, c.id));
        } else {
            sort_for_display(&mut hand, w);
        }
        arrange(hand.iter().map(|c| c.id).collect());
    };

    let on_down = move |id: u8, ev: web_sys::PointerEvent| {
        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            if ev.button() != 0 || drag.get_value().is_some() {
                return;
            }
            let (Some(hand), Some(target)) = (
                hand_ref.get_untracked(),
                ev.current_target()
                    .and_then(|t| t.dyn_into::<web_sys::Element>().ok()),
            ) else {
                return;
            };
            let grab_dx = ev.client_x() as f64 - target.get_bounding_client_rect().left();
            let _ = hand.set_pointer_capture(ev.pointer_id());
            drag.set_value(Some(Drag {
                id,
                pointer: ev.pointer_id(),
                start_x: ev.client_x() as f64,
                start_y: ev.client_y() as f64,
                grab_dx,
                moved: false,
            }));
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = (id, ev);
    };

    let on_move = move |ev: web_sys::PointerEvent| {
        #[cfg(target_arch = "wasm32")]
        {
            let Some(mut d) = drag.get_value() else {
                return;
            };
            if d.pointer != ev.pointer_id() {
                return;
            }
            let Some(hand) = hand_ref.get_untracked() else {
                return;
            };
            let (x, y) = (ev.client_x() as f64, ev.client_y() as f64);
            if !d.moved {
                if (x - d.start_x).hypot(y - d.start_y) < 8.0 {
                    return;
                }
                d.moved = true;
                drag.set_value(Some(d));
                local.set(Some(cards().iter().map(|c| c.id).collect()));
                fx::lift(d.id);
            }
            let dy = y - d.start_y;
            if let Some(to) = fx::drop_index(&hand, d.id, x, d.grab_dx) {
                let mut ids = local.get_untracked().unwrap_or_default();
                if let Some(from) = ids.iter().position(|&i| i == d.id) {
                    if from != to && to < ids.len() {
                        let snap = fx::snapshot();
                        let id = ids.remove(from);
                        ids.insert(to, id);
                        local.set(Some(ids));
                        let you = game.with_untracked(|g| g.you);
                        let hand2 = hand.clone();
                        let (gx, cid) = (d.grab_dx, d.id);
                        fx::play(
                            snap,
                            None,
                            you,
                            Some(Box::new(move || fx::follow(&hand2, cid, x, gx, dy))),
                        );
                    }
                }
            }
            fx::follow(&hand, d.id, x, d.grab_dx, dy);
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = ev;
    };

    let on_up = move |ev: web_sys::PointerEvent| {
        #[cfg(target_arch = "wasm32")]
        {
            let Some(d) = drag.get_value() else {
                return;
            };
            if d.pointer != ev.pointer_id() {
                return;
            }
            drag.set_value(None);
            if let Some(hand) = hand_ref.get_untracked() {
                let _ = hand.release_pointer_capture(ev.pointer_id());
            }
            if d.moved {
                fx::settle(d.id);
                if let Some(ids) = local.get_untracked() {
                    send_msg(&ClientMsg::Arrange { order: ids });
                }
            } else if ev.type_() == "pointerup" {
                selected.update(|s| *s = if *s == Some(d.id) { None } else { Some(d.id) });
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = ev;
    };

    let preview = move || {
        let mut hand = game.with(|g| g.hand.clone());
        if drawn.get() {
            let id = selected.get()?;
            hand.retain(|c| c.id != id);
        }
        Some(best_meld(&hand, wild()))
    };
    let can_go_out = move || drawn.get() && preview().is_some_and(|m| m.points == 0);
    let act = move |go_out: bool| {
        if let Some(card) = selected.get_untracked() {
            send_msg(&if go_out {
                ClientMsg::GoOut { card }
            } else {
                ClientMsg::Discard { card }
            });
        }
    };

    view! {
        <div class="space-y-3">
            <div
                node_ref=hand_ref
                class="fc-hand pt-4 pb-1"
                style=move || format!("--n: {}", game.with(|g| g.hand.len()).max(2))
                on:pointermove=on_move
                on:pointerup=on_up
                on:pointercancel=on_up
            >
                <For
                    each=cards
                    key=|c| c.id
                    children=move |card| {
                        let id = card.id;
                        view! {
                            <button
                                data-fc-card=id
                                class=move || if selected.get() == Some(id) { "fc-lift rounded-lg" } else { "rounded-lg" }
                                on:pointerdown=move |ev| on_down(id, ev)
                                on:click=move |ev| {
                                    // Pointers are handled above; this is for keyboards.
                                    if ev.detail() == 0 {
                                        selected.update(|s| *s = if *s == Some(id) { None } else { Some(id) });
                                    }
                                }
                                on:contextmenu=|ev| ev.prevent_default()
                            >
                                {card_face(card, wild())}
                            </button>
                        }
                    }
                />
            </div>
            <p class="text-xs text-stone-500 dark:text-stone-400">
                "Tap a card to pick it. Drag cards to rearrange your hand."
            </p>

            {move || {
                my_turn
                    .get()
                    .then(|| {
                        view! {
                            <div class="grid grid-cols-2 gap-2">
                                <button
                                    class="touch-manipulation rounded-lg bg-blue-500 py-3 font-semibold text-white hover:bg-blue-600 disabled:cursor-not-allowed disabled:opacity-40"
                                    disabled=move || !drawn.get() || selected.get().is_none()
                                    on:click=move |_| act(false)
                                >
                                    "Discard"
                                </button>
                                <button
                                    class="touch-manipulation rounded-lg bg-amber-500 py-3 font-semibold text-stone-900 hover:bg-amber-400 disabled:cursor-not-allowed disabled:opacity-40"
                                    disabled=move || !can_go_out()
                                    on:click=move |_| act(true)
                                >
                                    "Go out"
                                </button>
                            </div>
                        }
                    })
            }}
            <div class="flex gap-2">
                <button
                    class="flex-1 touch-manipulation rounded bg-stone-600 px-3 py-2 text-sm text-white hover:bg-stone-500"
                    on:click=move |_| sort(false)
                >
                    "Sort by suit"
                </button>
                <button
                    class="flex-1 touch-manipulation rounded bg-stone-600 px-3 py-2 text-sm text-white hover:bg-stone-500"
                    on:click=move |_| sort(true)
                >
                    "Sort by rank"
                </button>
            </div>

            <div class="rounded-lg border border-stone-200 bg-white p-3 dark:border-stone-700 dark:bg-stone-800">
                {move || match preview() {
                    Some(meld) => {
                        let label = if drawn.get() {
                            format!("Laying down after that discard leaves {} points.", meld.points)
                        } else {
                            format!("Your best lay-down right now leaves {} points.", meld.points)
                        };
                        view! {
                            <p class="mb-2 text-xs text-stone-500 dark:text-stone-400">{label}</p>
                            {meld_view(meld, wild())}
                        }
                            .into_any()
                    }
                    None => {
                        view! {
                            <p class="text-xs text-stone-500 dark:text-stone-400">
                                "Pick the card you'd discard to see how the rest lays down."
                            </p>
                        }
                            .into_any()
                    }
                }}
            </div>
        </div>
    }
}

fn results_list(view: &GameView, results: Vec<RoundResult>) -> impl IntoView {
    let wild = view.wild;
    let players = view.players.clone();
    results
        .into_iter()
        .map(|r| {
            let name = players
                .iter()
                .find(|p| p.id == r.player)
                .map(|p| p.nickname.clone())
                .unwrap_or_default();
            view! {
                <div class="space-y-2 rounded-lg border border-stone-200 p-3 dark:border-stone-700">
                    <div class="flex items-center justify-between text-sm dark:text-stone-100">
                        <span class="font-semibold">
                            {name}
                            {r.went_out.then(|| view! { <span class="ml-2 text-xs text-amber-500">"went out"</span> })}
                        </span>
                        <span class=if r.meld.points == 0 { "text-green-500" } else { "text-red-400" }>
                            {format!("+{}", r.meld.points)}
                        </span>
                    </div>
                    {meld_view(r.meld, wild)}
                </div>
            }
        })
        .collect_view()
}

#[component]
fn RoundResults(view: GameView, results: Vec<RoundResult>) -> impl IntoView {
    let host = view.you == view.host;
    view! {
        <div class="space-y-3">
            <h2 class="text-lg font-semibold dark:text-white">
                {format!("Round {} results", view.round)}
            </h2>
            {results_list(&view, results)}
            {if host {
                view! {
                    <button
                        class="w-full rounded bg-blue-500 py-3 font-semibold text-white hover:bg-blue-600"
                        on:click=|_| send_msg(&ClientMsg::NextRound)
                    >
                        {format!(
                            "Deal round {} ({}s wild)",
                            view.round + 1,
                            rank_label(hand_size(view.round + 1)),
                        )}
                    </button>
                }
                    .into_any()
            } else {
                view! {
                    <p class="text-center text-sm text-stone-500">"Waiting for the host to deal..."</p>
                }
                    .into_any()
            }}
        </div>
    }
}

#[component]
fn FinalResults(
    view: GameView,
    results: Vec<RoundResult>,
    standings: Vec<Standing>,
) -> impl IntoView {
    let mut standings = standings;
    standings.sort_by_key(|s| (s.place, s.player));
    let you_won = standings
        .iter()
        .any(|s| s.player == view.you && s.place == 1);
    let players = view.players.clone();
    view! {
        <div class="space-y-4">
            <p class=if you_won {
                "text-center text-3xl font-bold text-green-500"
            } else {
                "text-center text-3xl font-bold text-stone-300"
            }>{if you_won { "You win!" } else { "Game over" }}</p>
            <ol class="mx-auto max-w-sm space-y-1">
                {standings
                    .into_iter()
                    .map(|s| {
                        let name = players
                            .iter()
                            .find(|p| p.id == s.player)
                            .map(|p| p.nickname.clone())
                            .unwrap_or_default();
                        view! {
                            <li class="flex justify-between rounded bg-stone-800 px-3 py-2 text-stone-100">
                                <span>{format!("{}. {name}", s.place)}</span>
                                <span class="font-semibold">{s.total}</span>
                            </li>
                        }
                    })
                    .collect_view()}
            </ol>
            <button
                class="w-full rounded bg-blue-500 py-3 font-semibold text-white hover:bg-blue-600"
                on:click=|_| send_msg(&ClientMsg::PlayAgain)
            >
                "Play again"
            </button>
            <h3 class="pt-2 text-sm font-semibold text-stone-500 uppercase">"Final round"</h3>
            {results_list(&view, results)}
        </div>
    }
}

// ── Pages ─────────────────────────────────────────────────────────────────────

#[component]
pub fn FiveCrownsLobby() -> impl IntoView {
    use crate::app::{Footer, NavBar};

    let create = ServerAction::<CreateFiveCrownsRoom>::new();
    let nav = use_navigate();
    Effect::new(move |_| {
        if let Some(Ok(id)) = create.value().get() {
            nav(&format!("/games/five-crowns/{id}"), Default::default());
        }
    });
    let rooms = Resource::new(|| (), |_| list_rooms());

    view! {
        <NavBar />
        <div class="container mx-auto max-w-xl space-y-6 px-4 py-8">
            <GameGate>
                <div class="space-y-6">
                    <div>
                        <h1 class="text-3xl font-bold dark:text-white">"Five Crowns"</h1>
                        <p class="mt-1 text-sm text-stone-500 dark:text-stone-400">
                            "Eleven rounds, five suits, and a wild card that changes every round. \
                            Make your whole hand into books and runs, and keep your score low."
                        </p>
                    </div>
                    <div class="rounded-lg border border-stone-200 bg-white p-5 dark:border-stone-700 dark:bg-stone-800">
                        <div class="mb-3 flex items-center justify-between">
                            <h2 class="text-lg font-semibold dark:text-stone-100">"Open Rooms"</h2>
                            <button
                                class="touch-manipulation text-xs text-blue-500 hover:underline"
                                on:click=move |_| rooms.refetch()
                            >
                                "Refresh"
                            </button>
                        </div>
                        <Suspense fallback=|| {
                            view! { <p class="text-sm text-stone-400">"Loading..."</p> }
                        }>
                            {move || {
                                rooms
                                    .get()
                                    .map(|result| match result {
                                        Err(_) => {
                                            view! { <p class="text-sm text-red-500">"Error loading rooms."</p> }
                                                .into_any()
                                        }
                                        Ok(list) if list.is_empty() => {
                                            view! {
                                                <p class="text-sm text-stone-400 dark:text-stone-500">
                                                    "No open rooms yet. Create one below."
                                                </p>
                                            }
                                                .into_any()
                                        }
                                        Ok(list) => {
                                            view! {
                                                <div class="space-y-2">
                                                    {list.into_iter().map(room_row).collect_view()}
                                                </div>
                                            }
                                                .into_any()
                                        }
                                    })
                            }}
                        </Suspense>
                    </div>
                    <div class="rounded-lg border border-stone-200 bg-white p-5 dark:border-stone-700 dark:bg-stone-800">
                        <h2 class="mb-3 text-lg font-semibold dark:text-stone-100">"Create a Room"</h2>
                        <ActionForm action=create attr:class="space-y-3">
                            <input
                                type="text"
                                name="name"
                                placeholder="Room name (visible in the list)"
                                maxlength="32"
                                class="w-full rounded border border-stone-300 bg-white px-3 py-2 text-sm dark:border-stone-600 dark:bg-stone-700 dark:text-white"
                            />
                            <input
                                type="text"
                                name="passcode"
                                placeholder="Passcode (required to join)"
                                maxlength="20"
                                class="w-full rounded border border-stone-300 bg-white px-3 py-2 text-sm dark:border-stone-600 dark:bg-stone-700 dark:text-white"
                            />
                            <button
                                type="submit"
                                class="w-full touch-manipulation rounded bg-blue-500 py-2 text-sm font-semibold text-white hover:bg-blue-600 disabled:cursor-not-allowed disabled:opacity-50"
                                disabled=move || create.pending().get()
                            >
                                {move || if create.pending().get() { "Creating..." } else { "Create Room" }}
                            </button>
                            {move || {
                                create
                                    .value()
                                    .get()
                                    .and_then(|v| v.err())
                                    .map(|e| view! { <p class="text-xs text-red-500">{e.to_string()}</p> })
                            }}
                        </ActionForm>
                    </div>
                </div>
            </GameGate>
        </div>
        <Footer />
    }
}

fn room_row(room: RoomInfo) -> impl IntoView {
    let joinable = !room.in_progress && room.players < room.capacity;
    let href = format!("/games/five-crowns/{}", room.id);
    view! {
        <div class="flex items-center justify-between rounded border border-stone-200 px-3 py-2 dark:border-stone-600">
            <div class="min-w-0">
                <span class="truncate font-semibold dark:text-stone-100">{room.name.clone()}</span>
                <span class="ml-2 font-mono text-xs text-stone-400">{room.id.clone()}</span>
                <p class="text-xs text-stone-500">
                    {format!("{} · {}/{} players", room.mode, room.players, room.capacity)}
                </p>
            </div>
            {if joinable {
                view! {
                    <A
                        href=href
                        attr:class="ml-2 shrink-0 touch-manipulation rounded bg-blue-500 px-3 py-1 text-xs text-white hover:bg-blue-600"
                    >
                        "Join"
                    </A>
                }
                    .into_any()
            } else {
                view! {
                    <span class="ml-2 shrink-0 text-xs text-stone-400">
                        {if room.in_progress { "In progress" } else { "Full" }}
                    </span>
                }
                    .into_any()
            }}
        </div>
    }
}

#[derive(Params, PartialEq, Clone)]
struct RoomParams {
    code: Option<String>,
}

#[component]
pub fn FiveCrownsRoom() -> impl IntoView {
    use crate::app::{Footer, NavBar};
    let params = use_params::<RoomParams>();
    let code = move || {
        params.with(|p| {
            p.as_ref()
                .ok()
                .and_then(|p| p.code.clone())
                .unwrap_or_default()
        })
    };
    let (passcode, set_passcode) = signal(String::new());
    let (entered, set_entered) = signal(false);
    let exists = Resource::new(code, |c| async move {
        if c.is_empty() {
            return Ok(false);
        }
        room_exists(c).await
    });

    view! {
        <NavBar />
        <div class="container mx-auto px-3 py-4 sm:px-4 sm:py-6">
            <GameGate>
                <Suspense fallback=|| {
                    view! { <p class="pt-16 text-center text-sm text-stone-400">"Checking room..."</p> }
                }>
                    {move || {
                        let c = code();
                        match exists.get() {
                            Some(Ok(true)) if entered.get() => {
                                view! { <GameRoomInner room_id=c room_passcode=passcode.get_untracked() /> }
                                    .into_any()
                            }
                            Some(Ok(true)) => {
                                view! {
                                    <form
                                        class="mx-auto max-w-xs space-y-4 pt-16"
                                        on:submit=move |ev| {
                                            ev.prevent_default();
                                            if !passcode.get_untracked().is_empty() {
                                                set_entered.set(true);
                                            }
                                        }
                                    >
                                        <h2 class="text-xl font-bold dark:text-white">{format!("Enter Passcode for {c}")}</h2>
                                        <input
                                            type="text"
                                            placeholder="Passcode"
                                            class="w-full rounded border border-stone-300 bg-white px-3 py-2 dark:border-stone-600 dark:bg-stone-700 dark:text-white"
                                            on:input=move |ev| set_passcode.set(event_target_value(&ev))
                                            prop:value=move || passcode.get()
                                        />
                                        <button
                                            type="submit"
                                            class="w-full rounded bg-blue-500 py-2 text-white hover:bg-blue-600"
                                        >
                                            "Enter"
                                        </button>
                                    </form>
                                }
                                    .into_any()
                            }
                            Some(Ok(false)) => {
                                view! {
                                    <div class="mx-auto max-w-xs space-y-4 pt-16 text-center">
                                        <h2 class="text-xl font-bold dark:text-white">"No game found"</h2>
                                        <p class="text-sm text-stone-500 dark:text-stone-400">
                                            "There's no room with the code " <span class="font-mono">{c}</span> "."
                                        </p>
                                        <A
                                            href="/games/five-crowns"
                                            attr:class="inline-block rounded bg-blue-500 px-4 py-2 text-sm text-white hover:bg-blue-600"
                                        >
                                            "Back to the lobby"
                                        </A>
                                    </div>
                                }
                                    .into_any()
                            }
                            _ => {
                                view! {
                                    <p class="pt-16 text-center text-sm text-red-500">
                                        "Couldn't check the room. Try again."
                                    </p>
                                }
                                    .into_any()
                            }
                        }
                    }}
                </Suspense>
            </GameGate>
        </div>
        <Footer />
    }
}
