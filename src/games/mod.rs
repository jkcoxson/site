// Jackson Coxson

pub mod five_crowns;
pub mod light_beams;
pub mod rays_of_light;
#[cfg(feature = "ssr")]
pub mod rooms;
#[cfg(feature = "ssr")]
pub mod server;

use leptos::prelude::*;
use leptos_router::hooks::use_params;
use leptos_router::{components::A, hooks::use_navigate, params::Params};
use serde::{Deserialize, Serialize};

// ── Shared types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoomInfo {
    pub id: String,
    pub name: String,
    pub players: u8,
    pub capacity: u8,
    pub in_progress: bool,
    pub mode: String,
}

// ── Server functions ──────────────────────────────────────────────────────────

#[server(CreateRoom)]
pub async fn create_room(
    name: String,
    passcode: String,
    rules: String,
) -> Result<String, ServerFnError> {
    use crate::context::Context;
    use crate::games::light_beams::logic::Rules;
    use rand::RngExt;
    if name.trim().is_empty() {
        return Err(ServerFnError::ServerError("Room name is required.".into()));
    }
    if passcode.trim().is_empty() {
        return Err(ServerFnError::ServerError("Passcode is required.".into()));
    }
    let rules: Rules = serde_json::from_str(&rules).unwrap_or_default();
    if !rules.is_valid() {
        return Err(ServerFnError::ServerError(
            "Invalid rules: check the board size and piece counts.".into(),
        ));
    }
    // Generate id in a scope so ThreadRng is dropped before .await
    let id = {
        let mut rng = rand::rng();
        let chars: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
        (0..6)
            .map(|_| chars[rng.random_range(0..chars.len())] as char)
            .collect::<String>()
    };
    let ctx = use_context::<Context>().ok_or_else(|| {
        ServerFnError::<leptos::server_fn::error::NoCustomError>::ServerError(
            "No context".to_string(),
        )
    })?;
    ctx.game_state.create_room(id.clone(), name, passcode, rules).await;
    Ok(id)
}

#[server(RoomExists)]
pub async fn room_exists(code: String) -> Result<bool, ServerFnError> {
    use crate::context::Context;
    let ctx = use_context::<Context>().ok_or_else(|| {
        ServerFnError::<leptos::server_fn::error::NoCustomError>::ServerError(
            "No context".to_string(),
        )
    })?;
    Ok(ctx.game_state.room_exists(&code).await)
}

#[server(ListRooms)]
pub async fn list_rooms() -> Result<Vec<RoomInfo>, ServerFnError> {
    use crate::context::Context;
    let ctx = use_context::<Context>().ok_or_else(|| {
        ServerFnError::<leptos::server_fn::error::NoCustomError>::ServerError(
            "No context".to_string(),
        )
    })?;
    let infos = ctx.game_state.list_rooms().await;
    Ok(infos
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

// ── Hydration gate ──────────────────────────────────────────────────────────

/// Wraps a game's interactive content with a loading overlay that stays up until
/// the WASM bundle finishes downloading and Leptos hydrates the page. The pages
/// are server-rendered, so the board and buttons appear before their click
/// handlers are wired up; without this the controls look ready but do nothing.
///
/// Drop it *inside* a page, around the game content only — it covers what it
/// wraps, so keeping it between the NavBar and Footer leaves those untouched.
#[component]
pub fn GameGate(children: Children) -> impl IntoView {
    // This effect runs only on the client, after hydration completes — exactly
    // when the wrapped controls become interactive. On the server `hydrated`
    // stays false, so the overlay is part of the SSR HTML shown during download.
    let hydrated = RwSignal::new(false);
    Effect::new(move |_| hydrated.set(true));

    view! {
        <div class="relative">
            {move || {
                if hydrated.get() {
                    view! { <span /> }.into_any()
                } else {
                    view! {
                        <div class="absolute inset-0 z-20 flex flex-col items-center justify-center gap-4 rounded-xl bg-stone-900/80 backdrop-blur-sm">
                            <div class="h-10 w-10 animate-spin rounded-full border-4 border-stone-600 border-t-yellow-500" />
                            <p class="text-sm font-semibold text-stone-300">"Loading game…"</p>
                        </div>
                    }
                        .into_any()
                }
            }}
            {children()}
        </div>
    }
}

// ── Games lobby page ──────────────────────────────────────────────────────────

#[component]
pub fn GamesLobby() -> impl IntoView {
    use crate::app::{Footer, NavBar};
    view! {
        <NavBar />
        <div class="container mx-auto px-4 py-12 max-w-xl">
            <h1 class="text-3xl font-bold mb-6 dark:text-white">"Games"</h1>
            <div class="rounded-lg border border-stone-200 dark:border-stone-700 bg-white dark:bg-stone-800 p-6">
                <h2 class="text-xl font-semibold mb-2 dark:text-stone-100">"Rays of Light"</h2>
                <p class="text-stone-600 dark:text-stone-400 mb-4 text-sm">
                    "A two-player deduction game. Place colored glass and mirrors on your board, \
                    then deduce your opponent's layout by shooting rays of light."
                </p>
                <A
                    href="/games/rays-of-light"
                    attr:class="inline-block rounded bg-blue-500 px-4 py-2 text-white hover:bg-blue-600"
                >
                    "Play"
                </A>
            </div>
        </div>
        <Footer />
    }
}

// ── Rays of Light lobby ───────────────────────────────────────────────────────

#[component]
pub fn RaysOfLightLobby() -> impl IntoView {
    use crate::app::{Footer, NavBar};

    let (room_name, set_room_name) = signal(String::new());
    let (passcode, set_passcode) = signal(String::new());
    let create = ServerAction::<CreateRoom>::new();
    let nav = use_navigate();

    Effect::new(move |_| {
        if let Some(Ok(id)) = create.value().get() {
            nav(&format!("/games/rays-of-light/{id}"), Default::default());
        }
    });

    let rooms = Resource::new(|| (), |_| list_rooms());

    view! {
        <NavBar />
        <div class="container mx-auto px-4 py-8 max-w-xl space-y-6">
            <GameGate>
            <div class="flex items-center gap-2">
                <h1 class="text-3xl font-bold dark:text-white">"Rays of Light"</h1>
                <rays_of_light::RulesButton />
            </div>

            // Single player mode
            <div class="rounded-lg border border-stone-200 dark:border-stone-700 bg-white dark:bg-stone-800 p-5">
                <div class="flex items-center justify-between gap-4">
                    <div class="min-w-0">
                        <h2 class="text-lg font-semibold dark:text-stone-100">"Single player"</h2>
                        <p class="text-sm text-stone-500 dark:text-stone-400">
                            "Deduce a board the computer hides, no opponent needed."
                        </p>
                    </div>
                    <A
                        href="/games/rays-of-light/single-player"
                        attr:class="shrink-0 rounded bg-blue-500 px-4 py-2 text-sm text-white hover:bg-blue-600"
                    >
                        "Single Player"
                    </A>
                </div>
            </div>

            // Open room browser
            <div class="rounded-lg border border-stone-200 dark:border-stone-700 bg-white dark:bg-stone-800 p-5">
                <div class="flex items-center justify-between mb-3">
                    <h2 class="text-lg font-semibold dark:text-stone-100">"Open Rooms"</h2>
                    <button
                        class="text-xs text-blue-500 hover:underline touch-manipulation"
                        on:click=move |_| rooms.refetch()
                    >
                        "Refresh"
                    </button>
                </div>
                <Suspense fallback=|| view! { <p class="text-sm text-stone-400">"Loading..."</p> }>
                    {move || {
                        rooms.get().map(|result| match result {
                            Err(_) => {
                                view! {
                                    <p class="text-sm text-red-500">"Error loading rooms."</p>
                                }
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
                                        {list
                                            .into_iter()
                                            .map(|room| {
                                                let id = room.id.clone();
                                                let full = room.players >= 2;
                                                view! {
                                                    <div class="flex items-center justify-between rounded border border-stone-200 dark:border-stone-600 px-3 py-2">
                                                        <div class="min-w-0">
                                                            <span class="font-semibold dark:text-stone-100 truncate">
                                                                {room.name.clone()}
                                                            </span>
                                                            <span class="ml-2 text-xs text-stone-400 font-mono">
                                                                {id.clone()}
                                                            </span>
                                                        </div>
                                                        <div class="flex shrink-0 items-center gap-2 ml-2">
                                                            <span class=if full {
                                                                "text-xs text-stone-400"
                                                            } else {
                                                                "text-xs text-green-600 dark:text-green-400"
                                                            }>
                                                                {if room.in_progress {
                                                                    "In progress"
                                                                } else {
                                                                    "Waiting"
                                                                }}
                                                            </span>
                                                            {if !full {
                                                                let nav2 = use_navigate();
                                                                let rid = id.clone();
                                                                view! {
                                                                    <button
                                                                        class="rounded bg-blue-500 px-3 py-1 text-xs text-white hover:bg-blue-600 touch-manipulation"
                                                                        on:click=move |_| {
                                                                            nav2(
                                                                                &format!(
                                                                                    "/games/rays-of-light/{rid}",
                                                                                ),
                                                                                Default::default(),
                                                                            );
                                                                        }
                                                                    >
                                                                        "Join"
                                                                    </button>
                                                                }
                                                                    .into_any()
                                                            } else {
                                                                view! { <span /> }.into_any()
                                                            }}
                                                        </div>
                                                    </div>
                                                }
                                            })
                                            .collect_view()}
                                    </div>
                                }
                                    .into_any()
                            }
                        })
                    }}
                </Suspense>
            </div>

            // Create room
            <div class="rounded-lg border border-stone-200 dark:border-stone-700 bg-white dark:bg-stone-800 p-5">
                <h2 class="text-lg font-semibold mb-3 dark:text-stone-100">"Create a Room"</h2>
                <ActionForm action=create attr:class="space-y-3">
                    <input
                        type="text"
                        name="name"
                        placeholder="Room name (visible in the list)"
                        maxlength="32"
                        class="w-full rounded border border-stone-300 dark:border-stone-600 bg-white dark:bg-stone-700 px-3 py-2 text-sm dark:text-white"
                        on:input=move |ev| set_room_name.set(event_target_value(&ev))
                        prop:value=move || room_name.get()
                    />
                    <input
                        type="text"
                        name="passcode"
                        placeholder="Passcode (required to join)"
                        maxlength="20"
                        class="w-full rounded border border-stone-300 dark:border-stone-600 bg-white dark:bg-stone-700 px-3 py-2 text-sm dark:text-white"
                        on:input=move |ev| set_passcode.set(event_target_value(&ev))
                        prop:value=move || passcode.get()
                    />
                    <button
                        type="submit"
                        class="w-full rounded bg-blue-500 py-2 text-sm font-semibold text-white hover:bg-blue-600 disabled:opacity-50 disabled:cursor-not-allowed touch-manipulation"
                        disabled=move || create.pending().get()
                    >
                        {move || {
                            if create.pending().get() { "Creating..." } else { "Create Room" }
                        }}
                    </button>
                    {move || {
                        create.value().get().and_then(|v| v.err()).map(|e| {
                            view! { <p class="text-xs text-red-500">{e.to_string()}</p> }
                        })
                    }}
                </ActionForm>
            </div>
            </GameGate>
        </div>
        <Footer />
    }
}

// ── Single player mode page ────────────────────────────────────────────────────

#[component]
pub fn RaysOfLightSinglePlayer() -> impl IntoView {
    use crate::app::{Footer, NavBar};
    view! {
        <NavBar />
        <div class="container mx-auto px-4 py-6">
            <GameGate>
                <rays_of_light::SinglePlayerInner />
            </GameGate>
        </div>
        <Footer />
    }
}

// ── Game room page ────────────────────────────────────────────────────────────

#[derive(Params, PartialEq, Clone)]
struct RoomParams {
    code: Option<String>,
}

#[component]
pub fn RaysOfLightRoom() -> impl IntoView {
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

    // Read passcode from query string (set when navigating from lobby "Join" button)
    #[cfg(target_arch = "wasm32")]
    let passcode_from_url = {
        web_sys::window()
            .and_then(|w| w.location().search().ok())
            .and_then(|s| {
                s.strip_prefix('?').and_then(|q| {
                    q.split('&').find_map(|part| {
                        let (k, v) = part.split_once('=')?;
                        (k == "passcode").then_some(v.to_string())
                    })
                })
            })
            .unwrap_or_default()
    };
    #[cfg(not(target_arch = "wasm32"))]
    let passcode_from_url = String::new();

    let (passcode, set_passcode) = signal(passcode_from_url.clone());
    let (passcode_submitted, set_passcode_submitted) = signal(!passcode_from_url.is_empty());

    // Confirm the room actually exists before prompting for a passcode, so a bad
    // link tells the visitor "no such game" instead of taking a passcode and
    // dropping them into a doomed connection.
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
            {move || {
                let c = code();
                if c.is_empty() {
                    return view! { <p class="text-red-500">"Missing room code."</p> }.into_any();
                }
                view! {
                    <Suspense fallback=|| {
                        view! {
                            <p class="text-sm text-stone-400 pt-16 text-center">
                                "Checking room..."
                            </p>
                        }
                    }>
                        {move || {
                            let c = code();
                            match exists.get() {
                                Some(Ok(true)) => {
                                    view! { <RoomGate code=c.clone() passcode passcode_submitted set_passcode set_passcode_submitted /> }
                                        .into_any()
                                }
                                Some(Ok(false)) => {
                                    view! {
                                        <div class="max-w-xs mx-auto space-y-4 pt-16 text-center">
                                            <h2 class="text-xl font-bold dark:text-white">
                                                "No game found"
                                            </h2>
                                            <p class="text-sm text-stone-500 dark:text-stone-400">
                                                "There's no room with the code " <span class="font-mono">{c.clone()}</span> "."
                                            </p>
                                            <A
                                                href="/games/rays-of-light"
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
                                        <p class="text-sm text-red-500 pt-16 text-center">
                                            "Couldn't check the room. Try again."
                                        </p>
                                    }
                                        .into_any()
                                }
                            }
                        }}
                    </Suspense>
                }
                    .into_any()
            }}
            </GameGate>
        </div>
        <Footer />
    }
}

// Renders the passcode prompt (or the live game once the passcode is entered)
// for a room that's already been confirmed to exist.
#[component]
fn RoomGate(
    code: String,
    passcode: ReadSignal<String>,
    passcode_submitted: ReadSignal<bool>,
    set_passcode: WriteSignal<String>,
    set_passcode_submitted: WriteSignal<bool>,
) -> impl IntoView {
    view! {
        {move || {
                let c = code.clone();
                if !passcode_submitted.get() {
                    return view! {
                        <div class="max-w-xs mx-auto space-y-4 pt-16">
                            <h2 class="text-xl font-bold dark:text-white">
                                "Enter Passcode for " {c.clone()}
                            </h2>
                            <form
                                on:submit=move |ev| {
                                    ev.prevent_default();
                                    if !passcode.get_untracked().is_empty() {
                                        set_passcode_submitted.set(true);
                                    }
                                }
                                class="space-y-3"
                            >
                                <input
                                    type="text"
                                    placeholder="Passcode"
                                    class="w-full rounded border border-stone-300 dark:border-stone-600 bg-white dark:bg-stone-700 px-3 py-2 dark:text-white"
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
                        </div>
                    }
                        .into_any();
                }
                let pc = passcode.get();
                view! {
                    <rays_of_light::GameRoomInner room_id=c room_passcode=pc />
                }
                    .into_any()
            }}
    }
}
