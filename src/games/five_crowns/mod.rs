// Jackson Coxson
// Five Crowns: the rummy-style card game where the wild card changes every
// round. Rules: https://www.playmonster.com/wp-content/uploads/2019/09/5Crowns_RULES.pdf

pub mod logic;
#[cfg(feature = "ssr")]
pub mod server;

use leptos::prelude::*;
use leptos_router::hooks::{use_navigate, use_params};
use leptos_router::{components::A, params::Params};

use crate::games::{GameGate, RoomInfo};
use logic::{
    best_meld, hand_size, rank_label, sort_for_display, Card, ClientMsg, GameView, Meld,
    PlayerView, RoundResult, SeatInfo, ServerMsg, Stage, Standing, Suit, MIN_PLAYERS, ROUNDS,
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
    let ctx = use_context::<Context>()
        .ok_or_else(|| ServerFnError::new("No context"))?;
    ctx.game_state
        .create_five_crowns_room(id.clone(), name, passcode)
        .await;
    Ok(id)
}

#[server(FiveCrownsRoomExists)]
pub async fn room_exists(code: String) -> Result<bool, ServerFnError> {
    use crate::context::Context;
    let ctx = use_context::<Context>()
        .ok_or_else(|| ServerFnError::new("No context"))?;
    Ok(ctx.game_state.five_crowns_room_exists(&code).await)
}

#[server(ListFiveCrownsRooms)]
pub async fn list_rooms() -> Result<Vec<RoomInfo>, ServerFnError> {
    use crate::context::Context;
    let ctx = use_context::<Context>()
        .ok_or_else(|| ServerFnError::new("No context"))?;
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

/// A playing card. `small` is used for melds and the discard preview.
fn card_face(card: Card, wild: u8, small: bool) -> impl IntoView {
    let size = if small {
        "w-9 h-12 text-xs"
    } else {
        "w-14 h-20 text-base"
    };
    let ring = if card.is_wild(wild) {
        "ring-2 ring-amber-400"
    } else {
        "ring-1 ring-stone-300"
    };
    let symbol = card.suit.map(Suit::symbol).unwrap_or("♛");
    let rank = card.rank_label();
    view! {
        <div class=format!(
            "{size} {ring} {} relative flex select-none flex-col items-center justify-center rounded-lg bg-white font-bold shadow",
            suit_color(card.suit),
        )>
            <span class=if card.is_joker() {
                "text-[0.55rem] uppercase leading-none tracking-tight"
            } else {
                "leading-none"
            }>{rank}</span>
            <span class=if small { "text-base leading-none" } else { "text-2xl leading-none" }>
                {symbol}
            </span>
            {(card.is_wild(wild) && !small)
                .then(|| {
                    view! {
                        <span class="absolute -bottom-2 rounded bg-amber-400 px-1 text-[0.6rem] font-bold leading-tight text-stone-900">
                            "WILD"
                        </span>
                    }
                })}
        </div>
    }
}

fn card_back(label: String) -> impl IntoView {
    view! {
        <div class="flex h-20 w-14 flex-col items-center justify-center rounded-lg bg-gradient-to-br from-indigo-700 to-purple-800 text-amber-300 shadow ring-1 ring-indigo-400">
            <span class="text-2xl leading-none">"♛"</span>
            <span class="mt-1 text-[0.65rem] font-semibold text-indigo-100">{label}</span>
        </div>
    }
}

fn meld_view(meld: Meld, wild: u8) -> impl IntoView {
    view! {
        <div class="flex flex-wrap items-end gap-3">
            {meld
                .groups
                .into_iter()
                .map(|g| {
                    view! {
                        <div class="flex gap-0.5 rounded-lg bg-green-900/40 p-1 ring-1 ring-green-700">
                            {g.into_iter().map(|c| card_face(c, wild, true)).collect_view()}
                        </div>
                    }
                })
                .collect_view()}
            {(!meld.leftover.is_empty())
                .then(|| {
                    view! {
                        <div class="flex gap-0.5 rounded-lg bg-red-900/30 p-1 ring-1 ring-red-800">
                            {meld
                                .leftover
                                .into_iter()
                                .map(|c| card_face(c, wild, true))
                                .collect_view()}
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
    /// The card picked for discarding or moving.
    selected: Option<u8>,
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
                self.selected = None;
            }
            ServerMsg::Game(view) => {
                if self
                    .selected
                    .is_some_and(|id| !view.hand.iter().any(|c| c.id == id))
                {
                    self.selected = None;
                }
                self.phase = Phase::Game(view);
                self.error = None;
            }
            ServerMsg::Error(e) if is_fatal(&e) => self.phase = Phase::Closed(e),
            ServerMsg::Error(e) => self.error = Some(e),
        }
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
        if let Ok(msg) = serde_json::from_str::<ServerMsg>(&String::from(text)) {
            if let ServerMsg::Welcome { ref token, .. } = msg {
                if let Some(s) = storage() {
                    let _ = s.set_item(&token_key(&rid), token);
                }
            }
            state.update(|s| s.apply(msg));
        }
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

// ── Room ──────────────────────────────────────────────────────────────────────

#[component]
fn GameRoomInner(room_id: String, room_passcode: String) -> impl IntoView {
    let state = RwSignal::new(UiState {
        phase: Phase::Connecting,
        error: None,
        selected: None,
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

    view! {
        <div class="mx-auto max-w-5xl">
            <div class="mb-4 flex items-center justify-between">
                <h1 class="text-2xl font-bold dark:text-white">"Five Crowns"</h1>
                <span class="rounded bg-stone-100 px-3 py-1 font-mono text-sm dark:bg-stone-700 dark:text-white">
                    {room_id}
                </span>
            </div>
            {move || {
                state
                    .with(|s| s.error.clone())
                    .map(|err| {
                        view! {
                            <div class="mb-4 rounded border border-red-300 bg-red-100 px-4 py-2 text-sm text-red-800">
                                {err}
                            </div>
                        }
                    })
            }}
            {move || match state.with(|s| s.phase.clone()) {
                Phase::Connecting => {
                    view! {
                        <div class="flex flex-col items-center justify-center gap-3 py-16 text-stone-500 dark:text-stone-400">
                            <div class="h-8 w-8 animate-spin rounded-full border-4 border-stone-600 border-t-amber-500"></div>
                            <p>"Joining the table…"</p>
                        </div>
                    }
                        .into_any()
                }
                Phase::Reconnecting => {
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
                Phase::Closed(reason) => {
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
                Phase::Lobby { players, host, you } => {
                    view! { <LobbyView players host you /> }.into_any()
                }
                Phase::Game(view) => view! { <TableView view=*view state /> }.into_any(),
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
                    class="min-w-0 flex-1 rounded border border-stone-300 bg-white px-3 py-2 text-sm dark:border-stone-600 dark:bg-stone-700 dark:text-white"
                    on:input=move |ev| set_name.set(event_target_value(&ev))
                    prop:value=move || name.get()
                />
                <button
                    type="submit"
                    class="rounded bg-stone-600 px-4 py-2 text-sm text-white hover:bg-stone-500"
                >
                    "Rename"
                </button>
            </form>
            {if you == host {
                view! {
                    <button
                        class="w-full rounded bg-blue-500 py-3 font-semibold text-white hover:bg-blue-600 disabled:cursor-not-allowed disabled:opacity-50"
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

/// Move the selected card one place left or right and tell the server.
fn shift_selected(view: &GameView, selected: Option<u8>, by: isize) {
    let Some(id) = selected else {
        return;
    };
    let mut order: Vec<u8> = view.hand.iter().map(|c| c.id).collect();
    let Some(pos) = order.iter().position(|&c| c == id) else {
        return;
    };
    let to = pos as isize + by;
    if to < 0 || to as usize >= order.len() {
        return;
    }
    order.swap(pos, to as usize);
    send_msg(&ClientMsg::Arrange { order });
}

fn arrange_by(view: &GameView, by_rank: bool) {
    let wild = view.wild;
    let mut hand = view.hand.clone();
    if by_rank {
        hand.sort_by_key(|c| (c.is_wild(wild), c.rank, c.suit, c.id));
    } else {
        sort_for_display(&mut hand, wild);
    }
    send_msg(&ClientMsg::Arrange {
        order: hand.iter().map(|c| c.id).collect(),
    });
}

#[component]
fn TableView(view: GameView, state: RwSignal<UiState>) -> impl IntoView {
    let wild = view.wild;
    let my_turn = view.current == view.you && matches!(view.stage, Stage::Playing { .. });
    let drawn = matches!(view.stage, Stage::Playing { drawn: true });
    let name_of = {
        let players = view.players.clone();
        move |id: u8| {
            players
                .iter()
                .find(|p| p.id == id)
                .map(|p| p.nickname.clone())
                .unwrap_or_default()
        }
    };
    let turn_text = match &view.stage {
        Stage::Playing { .. } if my_turn && view.out.is_some() => {
            format!(
                "{} went out. Your last turn: draw, then discard. Whatever you can lay down is laid down for you.",
                name_of(view.out.unwrap_or_default())
            )
        }
        Stage::Playing { .. } if my_turn && !drawn => {
            "Your turn: draw from the deck or take the discard.".into()
        }
        Stage::Playing { .. } if my_turn => {
            "Pick a card to discard, or go out if the rest lays down.".into()
        }
        Stage::Playing { .. } => format!("{}'s turn.", name_of(view.current)),
        Stage::RoundOver { .. } => "Round over.".into(),
        Stage::GameOver { .. } => "Game over.".into(),
    };

    let view_sig = StoredValue::new(view.clone());
    let selected = move || state.with(|s| s.selected);
    // What laying down would leave right now: after discarding the selected
    // card once a card is drawn, otherwise the hand as it stands.
    let preview = move || {
        let v = view_sig.get_value();
        let mut cards = v.hand.clone();
        if drawn {
            let id = selected()?;
            cards.retain(|c| c.id != id);
        }
        Some(best_meld(&cards, v.wild))
    };
    let can_go_out = move || drawn && preview().is_some_and(|m| m.points == 0);

    let discard_top = view.discard_top;
    let take_discard = move |_| {
        if my_turn && !drawn && discard_top.is_some() {
            send_msg(&ClientMsg::Draw { from_discard: true });
        }
    };

    view! {
        <div class="grid gap-6 lg:grid-cols-[1fr_16rem]">
            <div class="space-y-5">
                <div class="flex flex-wrap items-center gap-3 text-sm">
                    <span class="rounded bg-stone-700 px-3 py-1 font-semibold text-white">
                        {format!("Round {}/{ROUNDS}", view.round)}
                    </span>
                    <span class="rounded bg-amber-400 px-3 py-1 font-semibold text-stone-900">
                        {format!("{}s are wild", rank_label(wild))}
                    </span>
                    <span class="text-stone-500 dark:text-stone-400">
                        {format!("{} cards each", hand_size(view.round))}
                    </span>
                </div>

                <div class="flex items-center gap-6 rounded-xl bg-green-900/60 p-5 ring-1 ring-green-800">
                    <button
                        class="disabled:cursor-default"
                        disabled=!(my_turn && !drawn)
                        on:click=|_| send_msg(&ClientMsg::Draw { from_discard: false })
                        title="Draw from the deck"
                    >
                        {card_back(format!("{} left", view.deck_size))}
                    </button>
                    <button
                        class="disabled:cursor-default"
                        disabled=!(my_turn && !drawn && discard_top.is_some())
                        on:click=take_discard
                        title="Take the top discard"
                    >
                        {match discard_top {
                            Some(c) => card_face(c, wild, false).into_any(),
                            None => {
                                view! {
                                    <div class="h-20 w-14 rounded-lg border-2 border-dashed border-green-700"></div>
                                }
                                    .into_any()
                            }
                        }}
                    </button>
                    <p class=if my_turn {
                        "text-sm font-semibold text-amber-300"
                    } else {
                        "text-sm text-green-100"
                    }>{turn_text}</p>
                </div>

                {match view.stage.clone() {
                    Stage::Playing { .. } => {
                        view! { <HandView view=view.clone() state my_turn drawn preview can_go_out /> }
                            .into_any()
                    }
                    Stage::RoundOver { results } => {
                        view! { <RoundResults view=view.clone() results /> }.into_any()
                    }
                    Stage::GameOver { results, standings } => {
                        view! { <FinalResults view=view.clone() results standings /> }.into_any()
                    }
                }}
            </div>

            <div class="space-y-5">
                <ScoreBoard view=view.clone() />
                <div class="rounded-lg border border-stone-200 bg-white p-3 text-xs dark:border-stone-700 dark:bg-stone-800">
                    <h3 class="mb-2 font-semibold text-stone-500 uppercase">"Table talk"</h3>
                    <ul class="space-y-1 text-stone-600 dark:text-stone-300">
                        {view.log.iter().rev().map(|l| view! { <li>{l.clone()}</li> }).collect_view()}
                    </ul>
                </div>
            </div>
        </div>
    }
}

#[component]
fn HandView(
    view: GameView,
    state: RwSignal<UiState>,
    my_turn: bool,
    drawn: bool,
    preview: impl Fn() -> Option<Meld> + Send + Sync + Copy + 'static,
    can_go_out: impl Fn() -> bool + Send + Sync + Copy + 'static,
) -> impl IntoView {
    let wild = view.wild;
    let view_sig = StoredValue::new(view.clone());
    let selected = move || state.with(|s| s.selected);
    let act = move |go_out: bool| {
        if let Some(card) = selected() {
            send_msg(&if go_out {
                ClientMsg::GoOut { card }
            } else {
                ClientMsg::Discard { card }
            });
        }
    };

    view! {
        <div class="space-y-4">
            <div class="flex flex-wrap gap-2 pt-2">
                {view
                    .hand
                    .iter()
                    .map(|&card| {
                        let id = card.id;
                        view! {
                            <button
                                class=move || {
                                    if selected() == Some(id) {
                                        "-translate-y-3 rounded-lg ring-4 ring-blue-500 transition-transform"
                                    } else {
                                        "rounded-lg transition-transform hover:-translate-y-1"
                                    }
                                }
                                on:click=move |_| {
                                    state
                                        .update(|s| {
                                            s.selected = if s.selected == Some(id) { None } else { Some(id) };
                                        })
                                }
                            >
                                {card_face(card, wild, false)}
                            </button>
                        }
                    })
                    .collect_view()}
            </div>

            <div class="flex flex-wrap items-center gap-2">
                {my_turn
                    .then(|| {
                        view! {
                            <button
                                class="rounded bg-blue-500 px-4 py-2 text-sm font-semibold text-white hover:bg-blue-600 disabled:cursor-not-allowed disabled:opacity-40"
                                disabled=move || !drawn || selected().is_none()
                                on:click=move |_| act(false)
                            >
                                "Discard"
                            </button>
                            <button
                                class="rounded bg-amber-500 px-4 py-2 text-sm font-semibold text-stone-900 hover:bg-amber-400 disabled:cursor-not-allowed disabled:opacity-40"
                                disabled=move || !can_go_out()
                                on:click=move |_| act(true)
                            >
                                "Go out"
                            </button>
                        }
                    })}
                <span class="mx-1 h-6 w-px bg-stone-600"></span>
                <button
                    class="rounded bg-stone-600 px-3 py-2 text-sm text-white hover:bg-stone-500 disabled:opacity-40"
                    disabled=move || selected().is_none()
                    on:click=move |_| shift_selected(&view_sig.get_value(), selected(), -1)
                    title="Move the selected card left"
                >
                    "◀"
                </button>
                <button
                    class="rounded bg-stone-600 px-3 py-2 text-sm text-white hover:bg-stone-500 disabled:opacity-40"
                    disabled=move || selected().is_none()
                    on:click=move |_| shift_selected(&view_sig.get_value(), selected(), 1)
                    title="Move the selected card right"
                >
                    "▶"
                </button>
                <button
                    class="rounded bg-stone-600 px-3 py-2 text-sm text-white hover:bg-stone-500"
                    on:click=move |_| arrange_by(&view_sig.get_value(), false)
                >
                    "Sort by suit"
                </button>
                <button
                    class="rounded bg-stone-600 px-3 py-2 text-sm text-white hover:bg-stone-500"
                    on:click=move |_| arrange_by(&view_sig.get_value(), true)
                >
                    "Sort by rank"
                </button>
            </div>

            <div class="rounded-lg border border-stone-200 bg-white p-3 dark:border-stone-700 dark:bg-stone-800">
                {move || match preview() {
                    Some(meld) => {
                        let label = if drawn {
                            format!("Laying down after that discard leaves {} points.", meld.points)
                        } else {
                            format!("Your best lay-down right now leaves {} points.", meld.points)
                        };
                        view! {
                            <p class="mb-2 text-xs text-stone-500 dark:text-stone-400">{label}</p>
                            {meld_view(meld, wild)}
                        }
                            .into_any()
                    }
                    None => {
                        view! {
                            <p class="text-xs text-stone-500 dark:text-stone-400">
                                "Select the card you'd discard to see how the rest lays down."
                            </p>
                        }
                            .into_any()
                    }
                }}
            </div>
        </div>
    }
}

#[component]
fn ScoreBoard(view: GameView) -> impl IntoView {
    let rounds_played = view.players.first().map_or(0, |p| p.rounds.len());
    let player_row = move |p: PlayerView| {
        let status = if view.current == p.id && matches!(view.stage, Stage::Playing { .. }) {
            "▶ "
        } else {
            ""
        };
        view! {
            <tr class="border-t border-stone-200 dark:border-stone-700">
                <td class="py-1 pr-2">
                    <span class=if p.connected { "" } else { "opacity-50" }>
                        {format!("{status}{}", p.nickname)}
                    </span>
                    {(p.id == view.dealer).then(|| view! { <span class="ml-1 text-[0.65rem] text-stone-400">"dealer"</span> })}
                    {(view.out == Some(p.id))
                        .then(|| view! { <span class="ml-1 text-[0.65rem] font-semibold text-amber-500">"out"</span> })}
                </td>
                <td class="py-1 text-center text-stone-500">{p.cards}</td>
                <td class="py-1 text-right font-semibold">{p.total}</td>
            </tr>
        }
    };
    view! {
        <div class="rounded-lg border border-stone-200 bg-white p-3 text-sm dark:border-stone-700 dark:bg-stone-800 dark:text-stone-100">
            <h3 class="mb-2 text-xs font-semibold text-stone-500 uppercase">
                {if rounds_played == 0 {
                    "Scores".to_string()
                } else {
                    format!("Scores after round {rounds_played}")
                }}
            </h3>
            <table class="w-full">
                <thead>
                    <tr class="text-xs text-stone-500">
                        <th class="text-left font-normal">"Player"</th>
                        <th class="font-normal">"Cards"</th>
                        <th class="text-right font-normal">"Total"</th>
                    </tr>
                </thead>
                <tbody>{view.players.clone().into_iter().map(player_row).collect_view()}</tbody>
            </table>
            <p class="mt-2 text-xs text-stone-500">"Lowest total after round 11 wins."</p>
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
fn FinalResults(view: GameView, results: Vec<RoundResult>, standings: Vec<Standing>) -> impl IntoView {
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
        <div class="container mx-auto px-4 py-6">
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
