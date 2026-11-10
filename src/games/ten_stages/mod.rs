// Jackson Coxson
// Ten Stages: a rummy-style race through ten stages of sets, runs and
// colors, one stage per round. Rules: https://service.mattel.com/instruction_sheets/W4729-Eng.pdf
// The lobby, rooms, hand and card motion come from the shared card table.

pub mod logic;
#[cfg(feature = "ssr")]
pub mod server;

use leptos::prelude::*;

use crate::games::cards::{
    self, final_standings, hand_view, seat_strip, table_talk, GameKind, HandOrder, SeatChip,
    Standing, TAG_DARK, TAG_GREEN, TAG_RED, TAG_SOFT,
};
use crate::games::number_cards::{card_back, numbered_face};
use crate::games::rules::{RulesButton, RulesSection};
use logic::{
    describe, lay_downs, plan_hits, Card, ClientMsg, Color, Face, GameView, Group, LayDown,
    Options, RoundResult, Status, STAGES,
};

fn send_msg(msg: &ClientMsg) {
    cards::send(msg);
}

// ── Cards ─────────────────────────────────────────────────────────────────────

fn colors(card: Card) -> &'static str {
    match card.face {
        Face::Number { color, .. } => match color {
            Color::Red => "bg-red-600 text-white ring-1 ring-red-800",
            Color::Blue => "bg-blue-600 text-white ring-1 ring-blue-800",
            Color::Green => "bg-green-600 text-white ring-1 ring-green-800",
            Color::Yellow => "bg-yellow-400 text-stone-900 ring-1 ring-yellow-600",
        },
        Face::Wild => {
            "bg-gradient-to-br from-fuchsia-500 via-amber-400 to-sky-500 text-white ring-1 ring-stone-700"
        }
        Face::Skip => "bg-stone-900 text-amber-300 ring-1 ring-amber-500",
    }
}

/// The strip along the bottom names the color, so red and green read apart.
fn banner(card: Card) -> (&'static str, &'static str) {
    match card.face {
        Face::Number { color, .. } => match color {
            Color::Red => ("RED", "bg-red-800 text-red-100"),
            Color::Blue => ("BLUE", "bg-blue-800 text-blue-100"),
            Color::Green => ("GREEN", "bg-green-800 text-green-100"),
            Color::Yellow => ("YELLOW", "bg-yellow-600 text-yellow-50"),
        },
        Face::Wild => ("WILD", "bg-stone-900/70 text-white"),
        Face::Skip => ("SKIP", "bg-amber-500 text-stone-900"),
    }
}

fn card_face(card: Card, extra: &'static str) -> impl IntoView {
    let label = match card.face {
        Face::Number { rank, .. } => rank.to_string(),
        Face::Wild => "W".into(),
        Face::Skip => "⊘".into(),
    };
    numbered_face(label, colors(card), extra, Some(banner(card)))
}

/// Small cards for previews and results. `moving` marks them with
/// `data-card` so they slide; only cards that exist once on the page may.
fn mini_row(cards: Vec<Card>, moving: bool) -> impl IntoView {
    view! {
        <div class="card-mini flex flex-wrap gap-0.5">
            {cards
                .into_iter()
                .map(|c| {
                    if moving {
                        view! { <div data-card=c.id>{card_face(c, "")}</div> }.into_any()
                    } else {
                        card_face(c, "").into_any()
                    }
                })
                .collect_view()}
        </div>
    }
}

fn groups_preview(groups: Vec<Group>) -> impl IntoView {
    view! {
        <div class="flex flex-wrap gap-2">
            {groups
                .into_iter()
                .map(|g| {
                    view! {
                        <div class="rounded-lg bg-green-900/40 p-1 ring-1 ring-green-700">
                            <p class="mb-0.5 text-[0.65rem] font-semibold text-green-100">
                                {g.label()}
                            </p>
                            {mini_row(g.cards, false)}
                        </div>
                    }
                })
                .collect_view()}
        </div>
    }
}

// ── Table ─────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum StatusKind {
    Playing,
    RoundOver,
    GameOver,
}

fn name_of(g: &GameView, id: u8) -> String {
    g.players
        .iter()
        .find(|p| p.id == id)
        .map(|p| p.nickname.clone())
        .unwrap_or_default()
}

#[component]
fn TableView(game: Memo<GameView>) -> impl IntoView {
    let my_turn = Memo::new(move |_| {
        game.with(|g| g.current == g.you && matches!(g.status, Status::Playing { .. }))
    });
    let drawn =
        Memo::new(move |_| game.with(|g| matches!(g.status, Status::Playing { drawn: true })));
    let can_draw = move || my_turn.get() && !drawn.get();
    let can_take = move || can_draw() && game.with(|g| g.discard_top.is_some_and(|c| !c.is_skip()));
    let status = Memo::new(move |_| {
        game.with(|g| match g.status {
            Status::Playing { .. } => StatusKind::Playing,
            Status::RoundOver { .. } => StatusKind::RoundOver,
            Status::GameOver { .. } => StatusKind::GameOver,
        })
    });
    // The hand card picked for a discard or a hit.
    let selected = RwSignal::new(None::<u8>);

    let turn_text = move || {
        game.with(|g| {
            let laid = g.me().is_some_and(|m| !m.laid.is_empty());
            match &g.status {
                Status::Playing { drawn: false } if my_turn.get() => {
                    "Your turn: draw from the deck or take the discard.".into()
                }
                Status::Playing { .. } if my_turn.get() && laid => {
                    "Hit cards onto the table, then discard to end your turn.".into()
                }
                Status::Playing { .. } if my_turn.get() => {
                    "Lay down your stage if you can, then discard.".into()
                }
                Status::Playing { .. } => format!("{}'s turn.", name_of(g, g.current)),
                Status::RoundOver { .. } => "Round over.".into(),
                Status::GameOver { .. } => "Game over.".into(),
            }
        })
    };

    view! {
        <div class="card-table grid gap-4 lg:grid-cols-[1fr_16rem]">
            <div class="min-w-0 space-y-3">
                <div class="flex flex-wrap items-center gap-2 text-sm">
                    <span class="rounded bg-stone-700 px-3 py-1 font-semibold text-white">
                        {move || format!("Round {}", game.with(|g| g.round))}
                    </span>
                    {move || {
                        game.with(|g| {
                            g.me()
                                .map(|m| {
                                    view! {
                                        <span class="rounded bg-amber-400 px-3 py-1 font-semibold text-stone-900">
                                            {format!("Stage {}: {}", m.stage, describe(m.stage))}
                                        </span>
                                    }
                                })
                        })
                    }}
                </div>

                {seat_strip(move || {
                    game.with(|g| {
                        let playing = matches!(g.status, Status::Playing { .. });
                        g.players
                            .iter()
                            .map(|p| {
                                let mut tags = Vec::new();
                                if p.id == g.dealer {
                                    tags.push(("dealer".to_string(), TAG_DARK));
                                }
                                let laid = !p.laid.is_empty();
                                tags.push((
                                    format!("stage {}{}", p.stage, if laid { " ✓" } else { "" }),
                                    if laid { TAG_GREEN } else { TAG_SOFT },
                                ));
                                if playing {
                                    tags.push((format!("{} cards", p.cards), TAG_SOFT));
                                }
                                if p.skipped {
                                    tags.push(("skipped".to_string(), TAG_RED));
                                }
                                SeatChip {
                                    id: p.id,
                                    name: p.nickname.clone(),
                                    you: p.id == g.you,
                                    connected: p.connected,
                                    active: playing && g.current == p.id,
                                    score: Some(p.total as i32),
                                    tags,
                                }
                            })
                            .collect()
                    })
                })}

                <div class="flex items-center gap-4 rounded-xl bg-green-900/60 p-3 ring-1 ring-green-800 sm:gap-6 sm:p-5">
                    <button
                        data-pile="deck"
                        class=move || {
                            if can_draw() {
                                "card-glow shrink-0 touch-manipulation"
                            } else {
                                "shrink-0 cursor-default"
                            }
                        }
                        disabled=move || !can_draw()
                        on:click=|_| send_msg(
                            &ClientMsg::Draw {
                                from_discard: false,
                            },
                        )
                        title="Draw from the deck"
                    >
                        {move || card_back(format!("{} left", game.with(|g| g.deck_size)))}
                    </button>
                    <button
                        class=move || {
                            if can_take() {
                                "card-glow shrink-0 touch-manipulation"
                            } else {
                                "shrink-0 cursor-default"
                            }
                        }
                        disabled=move || !can_take()
                        on:click=|_| send_msg(
                            &ClientMsg::Draw {
                                from_discard: true,
                            },
                        )
                        title="Take the top discard"
                    >
                        {move || match game.with(|g| g.discard_top) {
                            Some(c) => {
                                view! { <div data-card=c.id>{card_face(c, "")}</div> }.into_any()
                            }
                            None => {
                                view! {
                                    <div class="card-face rounded-lg border-2 border-dashed border-green-700"></div>
                                }
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

                {move || match status.get() {
                    StatusKind::Playing => {
                        view! {
                            <LaidDown game my_turn drawn selected />
                            <HandView game my_turn drawn selected />
                        }
                            .into_any()
                    }
                    StatusKind::RoundOver => {
                        let results_view = move || {
                            let g = game.get();
                            let Status::RoundOver { results } = g.status.clone() else {
                                return None;
                            };
                            Some(view! { <RoundResults view=g results /> })
                        };
                        view! { {results_view} }.into_any()
                    }
                    StatusKind::GameOver => {
                        let results_view = move || {
                            let g = game.get();
                            let Status::GameOver { results, standings } = g.status.clone() else {
                                return None;
                            };
                            Some(view! { <FinalResults view=g results standings /> })
                        };
                        view! { {results_view} }.into_any()
                    }
                }}
            </div>

            {table_talk(move || game.with(|g| g.log.clone()))}
        </div>
    }
}

/// A "+" spot at the end of a group, where the picked card can be hit.
fn hit_slot(card: u8, owner: u8, group: u8, low: bool) -> impl IntoView {
    view! {
        <button
            class="card-glow card-face flex shrink-0 touch-manipulation items-center justify-center rounded-lg border-2 border-dashed border-amber-300 text-xl font-bold text-amber-200"
            title="Hit here"
            on:click=move |_| send_msg(
                &ClientMsg::Hit {
                    card,
                    owner,
                    group,
                    low,
                },
            )
        >
            "+"
        </button>
    }
}

/// Everything laid down this round, by player. Once you've laid down, pick
/// a card and "+" spots show where it fits.
#[component]
fn LaidDown(
    game: Memo<GameView>,
    my_turn: Memo<bool>,
    drawn: Memo<bool>,
    selected: RwSignal<Option<u8>>,
) -> impl IntoView {
    move || {
        let g = game.get();
        let laid = g.me().is_some_and(|m| !m.laid.is_empty());
        let picked = selected
            .get()
            .filter(|_| my_turn.get() && drawn.get() && laid)
            .and_then(|id| g.hand.iter().find(|c| c.id == id).copied());
        let players: Vec<_> = g
            .players
            .iter()
            .filter(|p| !p.laid.is_empty())
            .cloned()
            .collect();
        if players.is_empty() {
            return view! {
                <p class="rounded-lg border border-dashed border-stone-300 p-3 text-center text-xs text-stone-500 dark:border-stone-700 dark:text-stone-400">
                    "Nobody has laid down a stage yet."
                </p>
            }
                .into_any();
        }
        view! {
            <div class="space-y-2 rounded-xl bg-green-950/40 p-2 ring-1 ring-green-900">
                {players
                    .into_iter()
                    .map(|p| {
                        let owner = p.id;
                        let who = if p.id == g.you {
                            "You".to_string()
                        } else {
                            p.nickname.clone()
                        };
                        view! {
                            <div>
                                <p class="mb-1 text-xs font-semibold text-green-100">
                                    {format!("{who} · stage {}", p.stage)}
                                </p>
                                <div class="flex flex-wrap gap-2">
                                    {p
                                        .laid
                                        .into_iter()
                                        .enumerate()
                                        .map(|(i, group)| {
                                            let [low, high] = picked
                                                .map(|c| group.fits(&c))
                                                .unwrap_or_default();
                                            let card = picked.map(|c| c.id).unwrap_or_default();
                                            let label = group.label();
                                            view! {
                                                <div class="rounded-lg bg-green-900/50 p-1 ring-1 ring-green-700">
                                                    <p class="mb-0.5 text-[0.65rem] font-semibold text-green-100">
                                                        {label}
                                                    </p>
                                                    <div class="card-mini flex flex-wrap items-center gap-0.5">
                                                        {low.then(|| hit_slot(card, owner, i as u8, true))}
                                                        {group
                                                            .cards
                                                            .into_iter()
                                                            .map(|c| {
                                                                view! { <div data-card=c.id>{card_face(c, "")}</div> }
                                                            })
                                                            .collect_view()}
                                                        {high.then(|| hit_slot(card, owner, i as u8, false))}
                                                    </div>
                                                </div>
                                            }
                                        })
                                        .collect_view()}
                                </div>
                            </div>
                        }
                    })
                    .collect_view()}
            </div>
        }
            .into_any()
    }
}

fn send_order(order: Vec<u8>) {
    send_msg(&ClientMsg::Arrange { order });
}

const BUTTON_BLUE: &str = "touch-manipulation rounded-lg bg-blue-500 py-3 font-semibold text-white hover:bg-blue-600 disabled:cursor-not-allowed disabled:opacity-40";
const BUTTON_AMBER: &str = "touch-manipulation rounded-lg bg-amber-500 py-3 font-semibold text-stone-900 hover:bg-amber-400 disabled:cursor-not-allowed disabled:opacity-40";
const BUTTON_SMALL: &str = "flex-1 touch-manipulation rounded bg-stone-600 px-3 py-2 text-sm text-white hover:bg-stone-500";

#[component]
fn HandView(
    game: Memo<GameView>,
    my_turn: Memo<bool>,
    drawn: Memo<bool>,
    selected: RwSignal<Option<u8>>,
) -> impl IntoView {
    let order = HandOrder::new(send_order);
    let hand = Memo::new(move |_| game.with(|g| g.hand.clone()));
    let laid = Memo::new(move |_| game.with(|g| g.me().is_some_and(|m| !m.laid.is_empty())));
    let stage = Memo::new(move |_| game.with(|g| g.me().map_or(1, |m| m.stage)));
    let table = Memo::new(move |_| {
        game.with(|g| {
            g.table()
                .into_iter()
                .map(|(_, _, gr)| gr)
                .collect::<Vec<Group>>()
        })
    });
    let options = Memo::new(move |_| {
        if laid.get() {
            return Vec::new();
        }
        hand.with(|h| table.with(|t| lay_downs(stage.get(), h, t)))
    });
    let option_at = RwSignal::new(0usize);
    let option = move || {
        options.with(|o| {
            let at = option_at.get().min(o.len().saturating_sub(1));
            o.get(at).cloned().map(|l| (at, o.len(), l))
        })
    };
    let hits = Memo::new(move |_| {
        if !laid.get() {
            return 0;
        }
        let mut groups = table.get();
        hand.with(|h| plan_hits(&mut groups, h).len())
    });
    let picked = move || {
        let id = selected.get()?;
        hand.with(|h| h.iter().find(|c| c.id == id).copied())
    };
    // Who a Skip could go to: everyone else who hasn't just been skipped.
    let targets = Memo::new(move |_| {
        game.with(|g| {
            g.players
                .iter()
                .filter(|p| p.id != g.you && !p.skip_proof)
                .map(|p| (p.id, p.nickname.clone()))
                .collect::<Vec<_>>()
        })
    });
    let choosing_target =
        move || picked().is_some_and(|c| c.is_skip()) && targets.with(|t| t.len() > 1);

    let sort = move |by_number: bool| {
        let mut cards = order.apply(hand.get_untracked());
        if by_number {
            cards.sort_by_key(Card::by_number);
        } else {
            cards.sort_by_key(Card::by_color);
        }
        order.arrange(cards.iter().map(|c| c.id).collect());
    };
    let discard = move |skip: Option<u8>| {
        if let Some(card) = selected.get_untracked() {
            send_msg(&ClientMsg::Discard { card, skip });
        }
    };
    let discard_label = move || match picked() {
        Some(c) if c.is_skip() => match targets.get().as_slice() {
            [] => "Discard Skip".to_string(),
            [(_, name)] => format!("Skip {name}"),
            _ => "Skip who?".to_string(),
        },
        _ => "Discard".to_string(),
    };
    let lay_down = move |l: LayDown| {
        let groups = l
            .groups
            .iter()
            .map(|g| g.cards.iter().map(|c| c.id).collect())
            .collect();
        send_msg(&ClientMsg::LayDown { groups });
    };

    view! {
        <div class="space-y-3">
            {hand_view(hand, selected, order, |card: Card| card_face(card, ""), |_| false)}
            <p class="text-xs text-stone-500 dark:text-stone-400">
                {move || {
                    if laid.get() {
                        "Tap a card to pick it, then a + on the table to hit it there. Drag to rearrange."
                    } else {
                        "Tap a card to pick it. Drag cards to rearrange your hand."
                    }
                }}
            </p>
            {move || {
                my_turn
                    .get()
                    .then(|| {
                        view! {
                            <div class="grid grid-cols-2 gap-2">
                                <button
                                    class=BUTTON_BLUE
                                    disabled=move || {
                                        !drawn.get() || selected.get().is_none()
                                            || choosing_target()
                                    }
                                    on:click=move |_| {
                                        let only = targets.get_untracked().first().map(|t| t.0);
                                        discard(picked().filter(|c| c.is_skip()).and(only))
                                    }
                                >
                                    {discard_label}
                                </button>
                                <button
                                    class=BUTTON_AMBER
                                    disabled=move || !drawn.get() || hits.get() == 0
                                    on:click=|_| send_msg(&ClientMsg::HitAll)
                                >
                                    {move || {
                                        let n = hits.get();
                                        if laid.get() && n > 0 {
                                            format!("Hit all ({n})")
                                        } else {
                                            "Hit all".to_string()
                                        }
                                    }}
                                </button>
                            </div>
                            {move || {
                                (drawn.get() && choosing_target())
                                    .then(|| {
                                        view! {
                                            <div class="flex flex-wrap items-center gap-2 rounded-lg bg-stone-800 p-2 text-sm text-stone-100">
                                                <span>"Skip:"</span>
                                                {targets
                                                    .get()
                                                    .into_iter()
                                                    .map(|(id, name)| {
                                                        view! {
                                                            <button
                                                                class="touch-manipulation rounded bg-red-600 px-3 py-1.5 font-semibold text-white hover:bg-red-500"
                                                                on:click=move |_| discard(Some(id))
                                                            >
                                                                {name}
                                                            </button>
                                                        }
                                                    })
                                                    .collect_view()}
                                            </div>
                                        }
                                    })
                            }}
                        }
                    })
            }} <div class="flex gap-2">
                <button class=BUTTON_SMALL on:click=move |_| sort(false)>
                    "Sort by color"
                </button>
                <button class=BUTTON_SMALL on:click=move |_| sort(true)>
                    "Sort by number"
                </button>
            </div>
            <div class="rounded-lg border border-stone-200 bg-white p-3 dark:border-stone-700 dark:bg-stone-800">
                {move || {
                    let s = stage.get();
                    if laid.get() {
                        let n = hits.get();
                        let text = if n == 0 {
                            format!(
                                "Stage {s} is down. Nothing in your hand fits the table right now.",
                            )
                        } else {
                            format!(
                                "Stage {s} is down. {n} of your cards can be hit onto the table.",
                            )
                        };
                        return view! {
                            <p class="text-xs text-stone-500 dark:text-stone-400">{text}</p>
                        }
                            .into_any();
                    }
                    match option() {
                        None => {
                            view! {
                                <p class="text-xs text-stone-500 dark:text-stone-400">
                                    {format!("Not yet: stage {s} needs {}.", describe(s))}
                                </p>
                            }
                                .into_any()
                        }
                        Some((at, count, l)) => {
                            let ready = my_turn.get() && drawn.get();
                            let multiple = count > 1;
                            let stuck = l.stuck;
                            let note = if !drawn.get() || !my_turn.get() {
                                format!("You can make stage {s}.")
                            } else if stuck <= 1 {
                                "Lay this down, hit the rest and you're out!".to_string()
                            } else {
                                format!(
                                    "After this and every hit, {stuck} cards stay in your hand.",
                                )
                            };
                            let chosen = l.clone();
                            view! {
                                <div class="space-y-2">
                                    <div class="flex items-center justify-between gap-2">
                                        <p class="text-xs text-stone-500 dark:text-stone-400">
                                            {note}
                                        </p>
                                        {multiple
                                            .then(|| {
                                                view! {
                                                    <div class="flex shrink-0 items-center gap-1 text-xs dark:text-stone-200">
                                                        <button
                                                            class="h-7 w-7 touch-manipulation rounded bg-stone-200 dark:bg-stone-700"
                                                            on:click=move |_| {
                                                                option_at.set((at + count - 1) % count)
                                                            }
                                                        >
                                                            "◂"
                                                        </button>
                                                        <span>{format!("{} of {count}", at + 1)}</span>
                                                        <button
                                                            class="h-7 w-7 touch-manipulation rounded bg-stone-200 dark:bg-stone-700"
                                                            on:click=move |_| option_at.set((at + 1) % count)
                                                        >
                                                            "▸"
                                                        </button>
                                                    </div>
                                                }
                                            })}
                                    </div>
                                    {groups_preview(l.groups)}
                                    <button
                                        class=format!("w-full {BUTTON_AMBER}")
                                        disabled=!ready
                                        on:click=move |_| lay_down(chosen.clone())
                                    >
                                        {format!("Lay down stage {s}")}
                                    </button>
                                </div>
                            }
                                .into_any()
                        }
                    }
                }}
            </div>
        </div>
    }
}

fn results_list(view: &GameView, results: Vec<RoundResult>) -> impl IntoView {
    let players = view.players.clone();
    results
        .into_iter()
        .map(|r| {
            let name = players
                .iter()
                .find(|p| p.id == r.player)
                .map(|p| p.nickname.clone())
                .unwrap_or_default();
            let (stage_text, stage_class) = if r.advanced {
                (format!("stage {} ✓", r.stage), TAG_GREEN)
            } else {
                (format!("stage {} ✗", r.stage), TAG_SOFT)
            };
            view! {
                <div class="space-y-2 rounded-lg border border-stone-200 p-3 dark:border-stone-700">
                    <div class="flex items-center justify-between text-sm dark:text-stone-100">
                        <span class="flex items-center gap-2 font-semibold">
                            {name}
                            <span class=format!(
                                "rounded px-1 text-[0.65rem] {stage_class}",
                            )>{stage_text}</span>
                            {r
                                .went_out
                                .then(|| {
                                    view! { <span class="text-xs text-amber-500">"went out"</span> }
                                })}
                        </span>
                        <span class=if r.points == 0 {
                            "text-green-500"
                        } else {
                            "text-red-400"
                        }>{format!("+{}", r.points)}</span>
                    </div>
                    {(!r.hand.is_empty()).then(|| mini_row(r.hand, false))}
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
                        {format!("Deal round {}", view.round + 1)}
                    </button>
                }
                    .into_any()
            } else {
                view! {
                    <p class="text-center text-sm text-stone-500">
                        "Waiting for the host to deal..."
                    </p>
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
    let names = view
        .players
        .iter()
        .map(|p| (p.id, format!("{} ({} stages)", p.nickname, p.done)))
        .collect();
    view! {
        <div class="space-y-4">
            {final_standings(view.you, standings, names, || send_msg(&ClientMsg::PlayAgain))}
            <h3 class="pt-2 text-sm font-semibold text-stone-500 uppercase">"Final round"</h3>
            {results_list(&view, results)}
        </div>
    }
}

// ── Pages ─────────────────────────────────────────────────────────────────────

fn rules_button() -> AnyView {
    view! {
        <RulesButton title="How to play Ten Stages">
            <TenStagesRules />
        </RulesButton>
    }
    .into_any()
}

#[component]
fn TenStagesRules() -> impl IntoView {
    let b = "font-medium text-white";
    view! {
        <RulesSection heading="Goal">
            <p>
                "Be the first to complete all ten stages. You work on one stage per round, and \
                you only move on once you've laid it down."
            </p>
        </RulesSection>
        <RulesSection heading="The stages">
            <ol class="list-decimal space-y-0.5 pl-5">
                {(1..=STAGES).map(|s| view! { <li>{describe(s)}</li> }).collect_view()}
            </ol>
            <p>
                <span class=b>"Sets"</span>
                " are cards of one number in any colors. "
                <span class=b>"Runs"</span>
                " are numbers in a row, in any colors, from 1 up to 12. "
                <span class=b>"One color"</span>
                " means any numbers, all the same color."
            </p>
        </RulesSection>
        <RulesSection heading="The cards">
            <p>
                "Two of each number 1 to 12 in four colors, plus eight Wilds and four Skips. \
                Everyone gets ten cards."
            </p>
            <p>
                <span class=b>"Wilds"</span>
                " stand in for any number or color. Each group you lay down needs at least one \
                numbered card."
            </p>
            <p>
                <span class=b>"Skips"</span>
                " are discarded to make another player lose their next turn. Nobody can be \
                skipped twice before they've had a normal turn, Skips can't be taken from the \
                discard pile, and they never go in a stage."
            </p>
        </RulesSection>
        <RulesSection heading="Your turn">
            <ol class="list-decimal space-y-1 pl-5">
                <li>"Draw from the deck or take the top discard."</li>
                <li>
                    "Lay down your stage if your hand makes it. The table finds the ways you \
                    can, best first."
                </li>
                <li>
                    "Once your stage is down, "<span class=b>"hit"</span>
                    ": put cards onto any group on the table, yours or anyone's, as long as they fit."
                </li>
                <li>"Discard one card to end your turn."</li>
            </ol>
            <p>
                "Get rid of every card, by hitting and with your last discard, to "
                <span class=b>"go out"</span>
                ". That ends the round. Everyone who laid down their stage moves on to the next one."
            </p>
        </RulesSection>
        <RulesSection heading="Scoring">
            <p>
                "Cards left in your hand count against you: 5 for 1–9, 10 for 10–12, 15 for a \
                Skip and 25 for a Wild. If more than one player finishes the last stage in the \
                same round, the lowest score wins."
            </p>
        </RulesSection>
    }
}

fn options_form() -> impl IntoView {
    let options = RwSignal::new(Options::default());
    view! {
        <div class="space-y-2 rounded border border-stone-200 p-3 dark:border-stone-600">
            <select
                class="w-full rounded border border-stone-300 bg-white px-2 py-2 text-sm dark:border-stone-600 dark:bg-stone-700 dark:text-white"
                on:change=move |ev| {
                    if let Ok(goal) = event_target_value(&ev).parse::<u8>() {
                        options.update(|o| o.goal = goal);
                    }
                }
            >
                {[10u8, 7, 5, 3]
                    .into_iter()
                    .map(|n| {
                        let label = if n == STAGES {
                            "All ten stages".to_string()
                        } else {
                            format!("Short game: stages 1 to {n}")
                        };
                        view! { <option value=n.to_string()>{label}</option> }
                    })
                    .collect_view()}
            </select>
            <label class="flex items-center gap-2 text-sm dark:text-stone-200">
                <input
                    type="checkbox"
                    prop:checked=move || options.with(|o| o.fixed)
                    on:change=move |ev| {
                        let on = event_target_checked(&ev);
                        options.update(|o| o.fixed = on);
                    }
                />
                "Everyone moves on each round (lowest score wins)"
            </label>
            <input
                type="hidden"
                name="options"
                prop:value=move || serde_json::to_string(&options.get()).unwrap_or_default()
            />
        </div>
    }
}

#[component]
pub fn TenStagesLobby() -> impl IntoView {
    cards::lobby_page(GameKind::TenStages, options_form().into_any(), rules_button)
}

#[component]
pub fn TenStagesRoom() -> impl IntoView {
    cards::room_page::<GameView>(
        GameKind::TenStages,
        |game| view! { <TableView game /> }.into_any(),
        rules_button,
    )
}
