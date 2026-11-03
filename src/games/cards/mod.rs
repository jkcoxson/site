// Jackson Coxson
// The shared card table: everything a multiplayer card game on the site needs
// besides its own rules. Games plug in a `TableView` (what one player sees)
// and a table component; this module supplies the lobby page with its room
// list, the passcode gate, the websocket with reconnects, the waiting room,
// the draggable hand, seats, table talk, final standings and card motion.

#[cfg(target_arch = "wasm32")]
pub mod fx;
#[cfg(feature = "ssr")]
pub mod server;

use leptos::prelude::*;
use leptos_router::hooks::{use_navigate, use_params};
use leptos_router::{components::A, params::Params};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::games::{GameGate, RoomInfo};

// ── Games ─────────────────────────────────────────────────────────────────────

/// The card games on the site. The slug names the game in URLs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameKind {
    FiveCrowns,
    PiratesWager,
    StackAttack,
    Golf,
}

impl GameKind {
    pub fn slug(self) -> &'static str {
        match self {
            GameKind::FiveCrowns => "five-crowns",
            GameKind::PiratesWager => "pirates-wager",
            GameKind::StackAttack => "stack-attack",
            GameKind::Golf => "golf",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        [
            GameKind::FiveCrowns,
            GameKind::PiratesWager,
            GameKind::StackAttack,
            GameKind::Golf,
        ]
            .into_iter()
            .find(|g| g.slug() == slug)
    }

    pub fn title(self) -> &'static str {
        match self {
            GameKind::FiveCrowns => "Five Crowns",
            GameKind::PiratesWager => "Pirate's Wager",
            GameKind::StackAttack => "Stack Attack",
            GameKind::Golf => "9 Holes of Golf",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            GameKind::FiveCrowns => {
                "Eleven rounds, five suits, and a wild card that changes every round. \
                 Make your whole hand into books and runs, and keep your score low."
            }
            GameKind::PiratesWager => {
                "A pirate trick-taking game. Each round, bid exactly how many tricks \
                 you'll win. Pirates, mermaids and the Captain fight over every trick."
            }
            GameKind::StackAttack => {
                "Race to empty your stock pile. Build shared piles from 1 up to 12, \
                 and use Wild cards to get unstuck."
            }
            GameKind::Golf => {
                "A family game of nine face-down cards. Swap and turn them up to end \
                 each hole with the lowest score. Matching columns score nothing."
            }
        }
    }

    pub fn players(self) -> (usize, usize) {
        match self {
            GameKind::FiveCrowns => (2, 7),
            GameKind::PiratesWager => (2, 8),
            GameKind::StackAttack => (2, 6),
            GameKind::Golf => (2, 6),
        }
    }
}

// ── Server functions ──────────────────────────────────────────────────────────

/// Open a room for `game` (a slug). `options` is the game's own settings as
/// JSON; games without any ignore it.
#[server(CreateCardRoom)]
pub async fn create_card_room(
    game: String,
    name: String,
    passcode: String,
    #[server(default)] options: String,
) -> Result<String, ServerFnError> {
    use crate::context::Context;
    let kind = GameKind::from_slug(&game).ok_or_else(|| ServerFnError::new("No such game."))?;
    if name.trim().is_empty() {
        return Err(ServerFnError::ServerError("Room name is required.".into()));
    }
    if passcode.trim().is_empty() {
        return Err(ServerFnError::ServerError("Passcode is required.".into()));
    }
    let id = server::room_code();
    let ctx = use_context::<Context>().ok_or_else(|| ServerFnError::new("No context"))?;
    ctx.game_state
        .create_card_room(kind, id.clone(), name, passcode, &options)
        .await
        .map_err(ServerFnError::new)?;
    Ok(id)
}

#[server(CardRoomExists)]
pub async fn card_room_exists(game: String, code: String) -> Result<bool, ServerFnError> {
    use crate::context::Context;
    let kind = GameKind::from_slug(&game).ok_or_else(|| ServerFnError::new("No such game."))?;
    let ctx = use_context::<Context>().ok_or_else(|| ServerFnError::new("No context"))?;
    Ok(ctx.game_state.card_room_exists(kind, &code).await)
}

#[server(ListCardRooms)]
pub async fn list_card_rooms(game: String) -> Result<Vec<RoomInfo>, ServerFnError> {
    use crate::context::Context;
    let kind = GameKind::from_slug(&game).ok_or_else(|| ServerFnError::new("No such game."))?;
    let ctx = use_context::<Context>().ok_or_else(|| ServerFnError::new("No context"))?;
    let rooms = ctx.game_state.list_card_rooms(kind).await;
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

// ── Wire messages ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeatInfo {
    pub id: u8,
    pub nickname: String,
    pub connected: bool,
}

/// The waiting room before a game starts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LobbyInfo {
    pub players: Vec<SeatInfo>,
    pub host: u8,
    pub you: u8,
    /// The room's settings, in words.
    pub about: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Standing {
    pub player: u8,
    pub total: i32,
    /// 1-based, ties share a place.
    pub place: u8,
}

/// What the server sends a card table. `V` is the game's own view.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ServerMsg<V> {
    /// Sent immediately on connect. `token` is used to reclaim this seat on reconnect.
    Welcome {
        player: u8,
        token: String,
    },
    Lobby(LobbyInfo),
    Game(Box<V>),
    Error(String),
}

/// One player's view of a game in progress, sent after every change so a
/// reconnecting client needs nothing else.
pub trait TableView:
    Clone + PartialEq + Send + Sync + Serialize + DeserializeOwned + 'static
{
    fn you(&self) -> u8;
    /// The latest move's sequence number, counting up with every move, and
    /// whether it dealt a fresh round.
    fn last_move(&self) -> Option<(u32, bool)>;
    /// Stand-in before the first update; never shown.
    fn placeholder() -> Self;
    /// How to animate the latest move.
    fn motion(&self) -> Motion;
}

// ── Motion ────────────────────────────────────────────────────────────────────

/// A place on the table, for card motion. See `fx` for the markup.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Spot {
    /// A pile by its `data-pile` name.
    Pile(&'static str),
    /// A card by id, where it was before the update.
    Card(u8),
    /// A player's seat.
    Seat(u8),
}

/// How to animate a move, beyond cards sliding from where they were to where
/// they are now, which always happens.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Motion {
    /// Cards that weren't on the page before fly in from here.
    pub arrive_from: Option<Spot>,
    /// Deal the arrivals one at a time.
    pub deal: bool,
    /// Things that were on the table fly off to a seat, or sink away and
    /// fade with no seat.
    pub leave: Vec<(Spot, Option<u8>)>,
}

/// Measure the table, run `update`, then animate the change with `motion`.
pub fn animate_change(motion: Motion, update: impl FnOnce()) {
    #[cfg(target_arch = "wasm32")]
    {
        let snap = fx::snapshot();
        update();
        fx::play(snap, motion, None);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = motion;
        update();
    }
}

/// Run `f` after `ms` milliseconds (in the browser; never on the server).
pub fn after(ms: i32, f: impl FnOnce() + 'static) {
    #[cfg(target_arch = "wasm32")]
    fx::after(ms, f);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (ms, f);
}

// ── Connection ────────────────────────────────────────────────────────────────

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
#[derive(Clone, PartialEq)]
enum Phase<V> {
    Connecting,
    Reconnecting,
    Lobby(LobbyInfo),
    Game(Box<V>),
    /// The server turned us away (no room, wrong passcode, full, started).
    Closed(String),
}

#[derive(Clone)]
struct UiState<V> {
    phase: Phase<V>,
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
impl<V: TableView> UiState<V> {
    fn apply(&mut self, msg: ServerMsg<V>) {
        match msg {
            ServerMsg::Welcome { .. } => self.error = None,
            ServerMsg::Lobby(info) => self.phase = Phase::Lobby(info),
            ServerMsg::Game(view) => {
                self.phase = Phase::Game(view);
                self.error = None;
            }
            ServerMsg::Error(e) if is_fatal(&e) => self.phase = Phase::Closed(e),
            ServerMsg::Error(e) => self.error = Some(e),
        }
    }

    /// Whether `view` carries a move we haven't seen yet.
    fn is_fresh(&self, view: &V) -> bool {
        let Some((seq, deal)) = view.last_move() else {
            return false;
        };
        match &self.phase {
            Phase::Game(old) => old.last_move().is_some_and(|(s, _)| seq > s),
            // The opening deal, straight from the waiting room.
            Phase::Lobby(_) => deal,
            _ => false,
        }
    }
}

#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;

#[cfg(target_arch = "wasm32")]
thread_local! {
    static GAME_WS: RefCell<Option<web_sys::WebSocket>> = const { RefCell::new(None) };
}

/// Send a message to the game server over the table's websocket.
pub fn send<T: Serialize>(msg: &T) {
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

/// Client messages every card game understands.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
enum CommonMsg {
    SetNickname { name: String },
    StartGame,
}

#[cfg(target_arch = "wasm32")]
fn token_key(kind: GameKind, room_id: &str) -> String {
    match kind {
        // Kept from before the shared table so open seats survive the update.
        GameKind::FiveCrowns => format!("fc_token_{room_id}"),
        _ => format!("{}_token_{room_id}", kind.slug()),
    }
}

#[cfg(target_arch = "wasm32")]
fn storage() -> Option<web_sys::Storage> {
    web_sys::window().and_then(|w| w.local_storage().ok().flatten())
}

#[cfg(target_arch = "wasm32")]
fn schedule_reconnect<V: TableView>(
    kind: GameKind,
    room_id: String,
    passcode: String,
    state: RwSignal<UiState<V>>,
    delay_ms: i32,
) {
    fx::after(delay_ms, move || {
        if state.with_untracked(|s| s.phase == Phase::Reconnecting) {
            connect_ws(kind, room_id, passcode, state, (delay_ms * 2).min(30_000));
        }
    });
}

#[cfg(target_arch = "wasm32")]
fn connect_ws<V: TableView>(
    kind: GameKind,
    room_id: String,
    passcode: String,
    state: RwSignal<UiState<V>>,
    next_delay_ms: i32,
) {
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
        .and_then(|s| s.get_item(&token_key(kind, &room_id)).ok().flatten())
        .map(|t| format!("?token={t}"))
        .unwrap_or_default();
    let url = format!(
        "{ws_proto}://{host}/games/{}/ws/{room_id}/{passcode}{token}",
        kind.slug()
    );

    let Ok(ws) = WebSocket::new(&url) else {
        state.update(|s| s.error = Some("Failed to connect to game server.".into()));
        return;
    };

    let rid = room_id.clone();
    let onmessage = Closure::<dyn FnMut(MessageEvent)>::new(move |e: MessageEvent| {
        let Ok(text) = e.data().dyn_into::<js_sys::JsString>() else {
            return;
        };
        let Ok(msg) = serde_json::from_str::<ServerMsg<V>>(&String::from(text)) else {
            return;
        };
        if let ServerMsg::Welcome { ref token, .. } = msg {
            if let Some(s) = storage() {
                let _ = s.set_item(&token_key(kind, &rid), token);
            }
        }
        if let ServerMsg::Game(ref view) = msg {
            // Measure the table before it changes so the change can be animated.
            let motion = if state.with_untracked(|s| s.is_fresh(view)) {
                view.motion()
            } else {
                Motion::default()
            };
            animate_change(motion, || state.update(|s| s.apply(msg)));
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
                let _ = s.remove_item(&token_key(kind, &room_id));
            }
            return;
        }
        state.update(|s| s.phase = Phase::Reconnecting);
        schedule_reconnect(
            kind,
            room_id.clone(),
            passcode.clone(),
            state,
            next_delay_ms,
        );
    });
    ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
    onclose.forget();

    GAME_WS.with(|cell| *cell.borrow_mut() = Some(ws));
}

// ── Room ──────────────────────────────────────────────────────────────────────

/// A game's table, drawn from the latest view.
pub type TableFn<V> = fn(Memo<V>) -> AnyView;

/// A game's "How to play" button (see `games::rules::RulesButton`).
pub type RulesFn = fn() -> AnyView;

fn game_room<V: TableView>(
    kind: GameKind,
    room_id: String,
    room_passcode: String,
    table: TableFn<V>,
    rules: RulesFn,
) -> impl IntoView {
    let state = RwSignal::new(UiState::<V> {
        phase: Phase::Connecting,
        error: None,
    });

    #[cfg(target_arch = "wasm32")]
    {
        let rc = room_id.clone();
        let rp = room_passcode.clone();
        Effect::new(move |_| connect_ws(kind, rc.clone(), rp.clone(), state, 1_000));
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
            Phase::Lobby(_) => Screen::Lobby,
            Phase::Game(_) => Screen::Game,
        })
    });
    // The latest table, kept through a reconnect so the page doesn't jump.
    let game = Memo::new(move |prev: Option<&V>| {
        state.with(|s| match &s.phase {
            Phase::Game(view) => (**view).clone(),
            _ => prev.cloned().unwrap_or_else(V::placeholder),
        })
    });
    let lobby_href = format!("/games/{}", kind.slug());

    view! {
        <div class="mx-auto max-w-5xl">
            <div class="mb-3 flex items-center justify-between">
                <div class="flex items-center gap-2">
                    <h1 class="text-xl font-bold sm:text-2xl dark:text-white">{kind.title()}</h1>
                    {rules()}
                </div>
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
                                href=lobby_href.clone()
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
                                Phase::Lobby(info) => Some(info.clone()),
                                _ => None,
                            })
                            .map(|info| waiting_room(kind, info))
                    };
                    view! { {lobby} }.into_any()
                }
                Screen::Game => table(game),
            }}
        </div>
    }
}

fn waiting_room(kind: GameKind, info: LobbyInfo) -> impl IntoView {
    let LobbyInfo {
        players,
        host,
        you,
        about,
    } = info;
    let (min, max) = kind.players();
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
                    {format!("Players ({count}/{max})")}
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
                {(!about.is_empty())
                    .then(|| {
                        view! { <p class="mt-3 text-xs text-stone-500 dark:text-stone-400">{about}</p> }
                    })}
            </div>
            <form
                class="flex gap-2"
                on:submit=move |ev| {
                    ev.prevent_default();
                    send(&CommonMsg::SetNickname { name: name.get_untracked() });
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
                        disabled=count < min
                        on:click=|_| send(&CommonMsg::StartGame)
                    >
                        {if count < min {
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
                {format!("Share the room code and passcode so friends can join. Up to {max} players.")}
            </p>
        </div>
    }
}

// ── Pages ─────────────────────────────────────────────────────────────────────

#[derive(Params, PartialEq, Clone)]
struct RoomParams {
    code: Option<String>,
}

/// The page for one room: checks it exists, asks for the passcode, then
/// joins the table.
pub fn room_page<V: TableView>(kind: GameKind, table: TableFn<V>, rules: RulesFn) -> impl IntoView {
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
    let exists = Resource::new(code, move |c| async move {
        if c.is_empty() {
            return Ok(false);
        }
        card_room_exists(kind.slug().into(), c).await
    });
    let lobby_href = format!("/games/{}", kind.slug());

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
                                game_room(kind, c, passcode.get_untracked(), table, rules).into_any()
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
                                            href=lobby_href.clone()
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

/// A game's lobby page: open rooms and a form to create one. `options` goes
/// inside the form; it should hold an input named `options` when the game
/// has settings.
pub fn lobby_page(kind: GameKind, options: AnyView, rules: RulesFn) -> impl IntoView {
    use crate::app::{Footer, NavBar};

    let create = ServerAction::<CreateCardRoom>::new();
    let nav = use_navigate();
    Effect::new(move |_| {
        if let Some(Ok(id)) = create.value().get() {
            nav(&format!("/games/{}/{id}", kind.slug()), Default::default());
        }
    });
    let rooms = Resource::new(|| (), move |_| list_card_rooms(kind.slug().into()));

    view! {
        <NavBar />
        <div class="container mx-auto max-w-xl space-y-6 px-4 py-8">
            <GameGate>
                <div class="space-y-6">
                    <div>
                        <div class="flex items-center gap-2">
                            <h1 class="text-3xl font-bold dark:text-white">{kind.title()}</h1>
                            {rules()}
                        </div>
                        <p class="mt-1 text-sm text-stone-500 dark:text-stone-400">{kind.blurb()}</p>
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
                                                    {list.into_iter().map(|r| room_row(kind, r)).collect_view()}
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
                            <input type="hidden" name="game" value=kind.slug() />
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
                            {options}
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

fn room_row(kind: GameKind, room: RoomInfo) -> impl IntoView {
    let joinable = !room.in_progress && room.players < room.capacity;
    let href = format!("/games/{}/{}", kind.slug(), room.id);
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

// ── Seats ─────────────────────────────────────────────────────────────────────

/// One player's seat along the top of the table.
#[derive(Debug, Clone, PartialEq)]
pub struct SeatChip {
    pub id: u8,
    pub name: String,
    pub you: bool,
    pub connected: bool,
    /// It's this player's move; the seat glows.
    pub active: bool,
    /// Running score; `None` for a seat that doesn't score.
    pub score: Option<i32>,
    /// Small labels under the score: text and Tailwind classes.
    pub tags: Vec<(String, &'static str)>,
}

pub const TAG_DARK: &str = "bg-stone-600 text-white";
pub const TAG_GOLD: &str = "bg-amber-500 font-semibold text-stone-900";
pub const TAG_GREEN: &str = "bg-green-600 font-semibold text-white";
pub const TAG_RED: &str = "bg-red-600 font-semibold text-white";
pub const TAG_SOFT: &str = "bg-stone-200 text-stone-700 dark:bg-stone-700 dark:text-stone-200";

/// Everyone at the table with their running score; whose move it is glows.
/// Seats carry `data-seat` so cards can fly to and from them.
pub fn seat_strip(seats: impl Fn() -> Vec<SeatChip> + Send + Sync + 'static) -> impl IntoView {
    view! {
        <div class="-mx-1 flex gap-2 overflow-x-auto px-1 py-1">
            {move || {
                seats()
                    .into_iter()
                    .map(|p| {
                        let class = format!(
                            "shrink-0 rounded-lg px-3 py-1.5 text-sm transition-colors {} {}",
                            if p.active {
                                "bg-amber-400/20 ring-2 ring-amber-400"
                            } else {
                                "bg-white ring-1 ring-stone-200 dark:bg-stone-800 dark:ring-stone-700"
                            },
                            if p.connected { "" } else { "opacity-50" },
                        );
                        view! {
                            <div data-seat=p.id class=class>
                                <div class="flex max-w-[9rem] items-center gap-1 font-semibold dark:text-stone-100">
                                    <span class="truncate">{p.name.clone()}</span>
                                    {p.you
                                        .then(|| view! { <span class="text-xs font-normal text-stone-400">"(you)"</span> })}
                                </div>
                                <div class="flex items-center gap-1.5 text-xs text-stone-500 dark:text-stone-400">
                                    {p.score
                                        .map(|score| {
                                            view! {
                                                <span>
                                                    <span class="font-bold text-stone-800 dark:text-stone-100">{score}</span>
                                                    " pts"
                                                </span>
                                            }
                                        })}
                                    {p.tags
                                        .into_iter()
                                        .map(|(text, class)| {
                                            view! {
                                                <span class=format!("rounded px-1 text-[0.6rem] {class}")>{text}</span>
                                            }
                                        })
                                        .collect_view()}
                                    {(!p.connected).then(|| view! { <span class="text-[0.6rem]">"away"</span> })}
                                </div>
                            </div>
                        }
                    })
                    .collect_view()
            }}
        </div>
    }
}

/// Recent happenings, newest first.
pub fn table_talk(log: impl Fn() -> Vec<String> + Send + Sync + 'static) -> impl IntoView {
    view! {
        <div class="rounded-lg border border-stone-200 bg-white p-3 text-xs lg:self-start dark:border-stone-700 dark:bg-stone-800">
            <h3 class="mb-2 font-semibold text-stone-500 uppercase">"Table talk"</h3>
            <ul class="space-y-1 text-stone-600 dark:text-stone-300">
                {move || log().into_iter().rev().map(|l| view! { <li>{l}</li> }).collect_view()}
            </ul>
        </div>
    }
}

/// The final standings with a play-again button. `names` maps player ids to
/// nicknames.
pub fn final_standings(
    you: u8,
    standings: Vec<Standing>,
    names: Vec<(u8, String)>,
    play_again: impl Fn() + 'static,
) -> impl IntoView {
    let mut standings = standings;
    standings.sort_by_key(|s| (s.place, s.player));
    let you_won = standings.iter().any(|s| s.player == you && s.place == 1);
    view! {
        <div class="space-y-4">
            <p class=if you_won {
                "text-center text-3xl font-bold text-green-500"
            } else {
                "text-center text-3xl font-bold text-stone-400"
            }>{if you_won { "You win!" } else { "Game over" }}</p>
            <ol class="mx-auto max-w-sm space-y-1">
                {standings
                    .into_iter()
                    .map(|s| {
                        let name = names
                            .iter()
                            .find(|(id, _)| *id == s.player)
                            .map(|(_, n)| n.clone())
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
                on:click=move |_| play_again()
            >
                "Play again"
            </button>
        </div>
    }
}

// ── Hand ──────────────────────────────────────────────────────────────────────

/// A card that can sit in a hand. Ids are unique within a game.
pub trait HandCard: Clone + PartialEq + Send + Sync + 'static {
    fn id(&self) -> u8;
}

/// The player's own arrangement of their hand. Rearranging shows at once and
/// is sent to the server with `send`, so it survives a reload.
#[derive(Clone, Copy)]
pub struct HandOrder {
    /// Our order while dragging, and until the server has caught up.
    local: RwSignal<Option<Vec<u8>>>,
    send: fn(Vec<u8>),
}

impl HandOrder {
    pub fn new(send: fn(Vec<u8>)) -> Self {
        Self {
            local: RwSignal::new(None),
            send,
        }
    }

    /// `hand` in the order the player last chose.
    pub fn apply<C: HandCard>(&self, mut hand: Vec<C>) -> Vec<C> {
        if let Some(ids) = self.local.get() {
            hand.sort_by_key(|c| ids.iter().position(|&i| i == c.id()).unwrap_or(usize::MAX));
        }
        hand
    }

    /// Put the hand in the order of `ids`, sliding the cards into place.
    pub fn arrange(&self, ids: Vec<u8>) {
        let local = self.local;
        let set = ids.clone();
        animate_change(Motion::default(), move || local.set(Some(set)));
        (self.send)(ids);
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

/// The player's hand: tap a card to select it, drag to rearrange. Cards
/// overlap to fit a phone. `cards` is the hand in the server's order;
/// `dimmed` cards are drawn faded (cards you can't play right now).
pub fn hand_view<C, R, V, D>(
    cards: Memo<Vec<C>>,
    selected: RwSignal<Option<u8>>,
    order: HandOrder,
    render: R,
    dimmed: D,
) -> impl IntoView
where
    C: HandCard,
    R: Fn(C) -> V + Copy + Send + Sync + 'static,
    V: IntoView + 'static,
    D: Fn(u8) -> bool + Copy + Send + Sync + 'static,
{
    let local = order.local;
    let drag = StoredValue::new(None::<Drag>);
    let hand_ref = NodeRef::<leptos::html::Div>::new();

    let server_ids =
        Memo::new(move |_| cards.with(|h| h.iter().map(|c| c.id()).collect::<Vec<u8>>()));
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

    let shown = move || order.apply(cards.get());

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
                local.set(Some(
                    order
                        .apply(cards.get_untracked())
                        .iter()
                        .map(|c| c.id())
                        .collect(),
                ));
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
                        let hand2 = hand.clone();
                        let (gx, cid) = (d.grab_dx, d.id);
                        fx::play(
                            snap,
                            Motion::default(),
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
                    (order.send)(ids);
                }
            } else if ev.type_() == "pointerup" {
                selected.update(|s| *s = if *s == Some(d.id) { None } else { Some(d.id) });
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = ev;
    };

    view! {
        <div
            node_ref=hand_ref
            class="card-hand pt-4 pb-1"
            style=move || format!("--n: {}", cards.with(|h| h.len()).max(2))
            on:pointermove=on_move
            on:pointerup=on_up
            on:pointercancel=on_up
        >
            <For
                each=shown
                key=|c| c.id()
                children=move |card| {
                    let id = card.id();
                    view! {
                        <button
                            data-card=id
                            class=move || {
                                let mut class = String::from("rounded-lg");
                                if selected.get() == Some(id) {
                                    class.push_str(" card-lift");
                                }
                                if dimmed(id) {
                                    class.push_str(" card-dim");
                                }
                                class
                            }
                            on:pointerdown=move |ev| on_down(id, ev)
                            on:click=move |ev| {
                                // Pointers are handled above; this is for keyboards.
                                if ev.detail() == 0 {
                                    selected.update(|s| *s = if *s == Some(id) { None } else { Some(id) });
                                }
                            }
                            on:contextmenu=|ev| ev.prevent_default()
                        >
                            {render(card)}
                        </button>
                    }
                }
            />
        </div>
    }
}
