// Jackson Coxson
// 9 Holes of Golf: a family card game. Everyone has a face-down 3×3 grid and
// swaps and turns up cards to end each hole with the lowest score. Built on
// the shared card table, with the numbered deck from Stack Attack.

pub mod logic;
#[cfg(feature = "ssr")]
pub mod server;

use leptos::prelude::*;

use crate::games::cards::{
    self, final_standings, seat_strip, table_talk, GameKind, SeatChip, Standing, TAG_DARK,
    TAG_GOLD, TAG_SOFT,
};
use crate::games::number_cards::{card_back, card_face, card_slot};
use crate::games::rules::{RulesButton, RulesSection};
use logic::{
    column, column_cleared, showing_score, slot_pile, Card, ClientMsg, GameView, HoleResult,
    PlayerView, Stage, OPENING_FLIPS,
};

fn send_msg(msg: &ClientMsg) {
    cards::send(msg);
}

fn name_of(g: &GameView, id: u8) -> String {
    g.players
        .iter()
        .find(|p| p.id == id)
        .map(|p| p.nickname.clone())
        .unwrap_or_default()
}

fn signed(n: i32) -> String {
    if n > 0 {
        format!("+{n}")
    } else {
        n.to_string()
    }
}

/// What tapping one of your own slots would do right now.
#[derive(Clone, Copy, PartialEq)]
enum SlotAction {
    None,
    Flip,
    Swap,
}

fn slot_action(g: &GameView, slot: usize) -> SlotAction {
    let Some(me) = g.players.iter().find(|p| p.id == g.you) else {
        return SlotAction::None;
    };
    let face_down = me.grid.get(slot).is_some_and(Option::is_none);
    match g.stage {
        Stage::Opening => {
            let up = me.grid.iter().filter(|c| c.is_some()).count();
            if face_down && up < OPENING_FLIPS {
                SlotAction::Flip
            } else {
                SlotAction::None
            }
        }
        Stage::Playing if g.current == g.you => match g.held {
            Some(_) => SlotAction::Swap,
            None if face_down => SlotAction::Flip,
            None => SlotAction::None,
        },
        _ => SlotAction::None,
    }
}

/// One player's 3×3 grid. Face-up cards turn over as they appear; face-down
/// slots are named piles so a card swapped out of one can fly from it.
fn grid_view(game: Memo<GameView>, player: u8, mine: bool) -> impl IntoView {
    let slots = move || {
        game.with(|g| {
            g.players
                .iter()
                .find(|p| p.id == player)
                .map(|p| p.grid.clone())
                .unwrap_or_default()
                .into_iter()
                .enumerate()
                .collect::<Vec<(usize, Option<Card>)>>()
        })
    };
    // Columns that score 0, so they can be marked.
    let cleared = move |slot: usize| {
        game.with(|g| {
            let Some(p) = g.players.iter().find(|p| p.id == player) else {
                return false;
            };
            let col = column(slot % 3).map(|i| p.grid.get(i).copied().flatten());
            match col {
                [Some(a), Some(b), Some(c)] => column_cleared([a, b, c]),
                _ => false,
            }
        })
    };
    view! {
        <div class="grid w-max grid-cols-3 gap-1.5 sm:gap-2">
            <For
                each=slots
                key=|(slot, card)| (*slot, card.map(|c| c.id))
                children=move |(slot, card)| {
                    let action = move || {
                        if mine { game.with(|g| slot_action(g, slot)) } else { SlotAction::None }
                    };
                    let face = match card {
                        Some(c) => {
                            view! {
                                <div data-card=c.id>
                                    <div class="card-flip">
                                        {move || {
                                            if cleared(slot) {
                                                card_face(c, "opacity-60").into_any()
                                            } else {
                                                card_face(c, "").into_any()
                                            }
                                        }}
                                    </div>
                                </div>
                            }
                                .into_any()
                        }
                        None => {
                            view! {
                                <div data-pile=slot_pile(
                                    player,
                                    slot as u8,
                                )>{card_back(String::new())}</div>
                            }
                                .into_any()
                        }
                    };
                    view! {
                        <button
                            class=move || {
                                if action() == SlotAction::None {
                                    "rounded-lg cursor-default"
                                } else {
                                    "card-glow rounded-lg touch-manipulation"
                                }
                            }
                            on:click=move |_| {
                                let msg = match action() {
                                    SlotAction::None => return,
                                    SlotAction::Flip => {
                                        ClientMsg::Flip {
                                            slot: slot as u8,
                                        }
                                    }
                                    SlotAction::Swap => {
                                        ClientMsg::Swap {
                                            slot: slot as u8,
                                        }
                                    }
                                };
                                send_msg(&msg);
                            }
                        >
                            {face}
                        </button>
                    }
                }
            />
        </div>
    }
}

// ── Table ─────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum StageKind {
    Opening,
    Playing,
    HoleOver,
    GameOver,
}

#[component]
fn TableView(game: Memo<GameView>) -> impl IntoView {
    let stage = Memo::new(move |_| {
        game.with(|g| match g.stage {
            Stage::Opening => StageKind::Opening,
            Stage::Playing => StageKind::Playing,
            Stage::HoleOver { .. } => StageKind::HoleOver,
            Stage::GameOver { .. } => StageKind::GameOver,
        })
    });
    let my_turn =
        Memo::new(move |_| stage.get() == StageKind::Playing && game.with(|g| g.current == g.you));
    let can_draw = move || my_turn.get() && game.with(|g| g.held.is_none());
    let can_take = move || can_draw() && game.with(|g| g.discard_top.is_some());
    let can_toss = move || my_turn.get() && game.with(|g| matches!(g.held, Some((_, false))));
    let you = Memo::new(move |_| game.with(|g| g.you));

    let turn_text = move || {
        game.with(|g| {
            let out = g.out.map(|o| name_of(g, o));
            let last = |s: String| match &out {
                Some(o) if g.out != Some(g.you) => format!("{o} is out, so this is your last turn. {s}"),
                _ => s,
            };
            match &g.stage {
                Stage::Opening => {
                    let up = g
                        .players
                        .iter()
                        .find(|p| p.id == g.you)
                        .map_or(0, |p| p.grid.iter().filter(|c| c.is_some()).count());
                    if up < OPENING_FLIPS {
                        format!("Pick {} card{} to turn up.", OPENING_FLIPS - up, if up == 1 { "" } else { "s" })
                    } else {
                        "Waiting for everyone to turn up two cards...".into()
                    }
                }
                Stage::Playing if g.current == g.you => match g.held {
                    None => last(
                        "Draw from the deck, take the discard, or tap a face-down card to turn it up."
                            .into(),
                    ),
                    Some((_, true)) => "Tap one of your cards to swap it out.".into(),
                    Some((_, false)) => "Tap one of your cards to swap it out, or discard what you drew.".into(),
                },
                Stage::Playing => format!("{}'s turn.", name_of(g, g.current)),
                Stage::HoleOver { .. } => format!("Hole {} is done.", g.hole),
                Stage::GameOver { .. } => "Game over.".into(),
            }
        })
    };

    view! {
        <div class="card-table grid gap-4 lg:grid-cols-[1fr_16rem]">
            <div class="min-w-0 space-y-3">
                <div class="flex flex-wrap items-center gap-2 text-sm">
                    <span class="rounded bg-stone-700 px-3 py-1 font-semibold text-white">
                        {move || format!("Hole {}", game.with(|g| g.hole))}
                    </span>
                </div>

                {seat_strip(move || {
                    game.with(|g| {
                        let playing = matches!(g.stage, Stage::Playing);
                        g.players
                            .iter()
                            .map(|p| {
                                let mut tags = Vec::new();
                                if p.id == g.dealer {
                                    tags.push(("dealer".to_string(), TAG_DARK));
                                }
                                if g.out == Some(p.id) {
                                    tags.push(("out".to_string(), TAG_GOLD));
                                }
                                if matches!(g.stage, Stage::Opening | Stage::Playing) {
                                    tags.push((
                                        format!("showing {}", showing_score(&p.grid)),
                                        TAG_SOFT,
                                    ));
                                }
                                SeatChip {
                                    id: p.id,
                                    name: p.nickname.clone(),
                                    you: p.id == g.you,
                                    connected: p.connected,
                                    active: playing && g.current == p.id,
                                    score: Some(p.total),
                                    tags,
                                }
                            })
                            .collect()
                    })
                })}

                <OtherGrids game />

                <div class="flex flex-wrap items-center gap-3 rounded-xl bg-green-900/60 p-3 ring-1 ring-green-800 sm:gap-5 sm:p-5">
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
                            None => card_slot("discard").into_any(),
                        }}
                    </button>
                    {move || {
                        game.with(|g| g.held)
                            .map(|(c, _)| {
                                let holder = game.with(|g| name_of(g, g.current));
                                view! {
                                    <div class="flex flex-col items-center gap-1">
                                        <div data-card=c
                                            .id>{card_face(c, "ring-4 ring-blue-500")}</div>
                                        <span class="text-[0.65rem] text-green-100">
                                            {if my_turn.get() {
                                                "You drew".to_string()
                                            } else {
                                                format!("{holder} drew")
                                            }}
                                        </span>
                                    </div>
                                }
                            })
                    }}
                    <div class="min-w-[10rem] flex-1 space-y-2">
                        <p class=move || {
                            if my_turn.get() {
                                "text-sm font-semibold text-amber-300"
                            } else {
                                "text-sm text-green-100"
                            }
                        }>{turn_text}</p>
                        {move || {
                            can_toss()
                                .then(|| {
                                    view! {
                                        <button
                                            class="touch-manipulation rounded-lg bg-stone-600 px-4 py-2 text-sm font-semibold text-white hover:bg-stone-500"
                                            on:click=|_| send_msg(&ClientMsg::Toss)
                                        >
                                            "Discard it"
                                        </button>
                                    }
                                })
                        }}
                    </div>
                </div>

                {move || match stage.get() {
                    StageKind::HoleOver => {
                        let g = game.get();
                        let Stage::HoleOver { results } = g.stage.clone() else {
                            return None;
                        };
                        Some(hole_over(g, results).into_any())
                    }
                    StageKind::GameOver => {
                        let g = game.get();
                        let Stage::GameOver { results, standings } = g.stage.clone() else {
                            return None;
                        };
                        Some(game_over(g, results, standings).into_any())
                    }
                    _ => None,
                }}

                <div class="flex flex-col items-center gap-2">
                    {move || grid_view(game, you.get(), true)}
                    <p class="text-xs text-stone-500 dark:text-stone-400">
                        "Matching columns score 0. 2s are -2 and Wilds are 0."
                    </p>
                </div>
            </div>

            {table_talk(move || game.with(|g| g.log.clone()))}
        </div>
    }
}

/// Everyone else's grids, small.
#[component]
fn OtherGrids(game: Memo<GameView>) -> impl IntoView {
    let others = Memo::new(move |_| {
        game.with(|g| {
            g.players
                .iter()
                .filter(|p| p.id != g.you)
                .map(|p| (p.id, p.nickname.clone()))
                .collect::<Vec<_>>()
        })
    });
    view! {
        <div class="card-mini flex flex-wrap justify-center gap-3">
            <For
                each=move || others.get()
                key=|p| p.clone()
                children=move |(id, nickname)| {
                    let active = move || {
                        game.with(|g| matches!(g.stage, Stage::Playing) && g.current == id)
                    };
                    view! {
                        <div class=move || {
                            if active() {
                                "space-y-1 rounded-lg bg-amber-400/20 p-2 ring-2 ring-amber-400"
                            } else {
                                "space-y-1 rounded-lg bg-green-900/30 p-2 ring-1 ring-green-800"
                            }
                        }>
                            <p class="max-w-[9rem] truncate text-center text-xs font-semibold dark:text-stone-100">
                                {nickname}
                            </p>
                            {grid_view(game, id, false)}
                        </div>
                    }
                }
            />
        </div>
    }
}

fn mini_grid(grid: Vec<Card>) -> impl IntoView {
    let cleared: Vec<bool> = (0..3)
        .map(|col| column_cleared(column(col).map(|i| grid[i])))
        .collect();
    view! {
        <div class="card-mini grid w-max grid-cols-3 gap-1">
            {grid
                .into_iter()
                .enumerate()
                .map(|(i, c)| {
                    if cleared[i % 3] {
                        card_face(c, "opacity-60").into_any()
                    } else {
                        card_face(c, "").into_any()
                    }
                })
                .collect_view()}
        </div>
    }
}

fn results_list(g: &GameView, results: Vec<HoleResult>) -> impl IntoView {
    let players: Vec<PlayerView> = g.players.clone();
    view! {
        <div class="grid gap-2 sm:grid-cols-2">
            {results
                .into_iter()
                .map(|r| {
                    let p = players.iter().find(|p| p.id == r.player);
                    let name = p.map(|p| p.nickname.clone()).unwrap_or_default();
                    let total = p.map_or(0, |p| p.total);
                    view! {
                        <div class="flex items-center gap-3 rounded-lg border border-stone-200 p-2 dark:border-stone-700">
                            {mini_grid(r.grid)}
                            <div class="space-y-0.5 text-sm dark:text-stone-100">
                                <p class="font-semibold">
                                    {name}
                                    {r
                                        .went_out
                                        .then(|| {
                                            view! {
                                                <span class="ml-1 text-xs text-amber-500">"went out"</span>
                                            }
                                        })}
                                </p>
                                <p>{format!("This hole: {}", signed(r.score))}</p>
                                <p class="text-xs text-stone-500">{format!("Total: {total}")}</p>
                            </div>
                        </div>
                    }
                })
                .collect_view()}
        </div>
    }
}

fn hole_over(g: GameView, results: Vec<HoleResult>) -> impl IntoView {
    let host = g.you == g.host;
    let next = g.hole + 1;
    view! {
        <div class="space-y-3 rounded-xl border border-stone-200 bg-white p-4 dark:border-stone-700 dark:bg-stone-800">
            <h2 class="text-lg font-semibold dark:text-white">
                {format!("Hole {} results", g.hole)}
            </h2>
            {results_list(&g, results)}
            {if host {
                view! {
                    <div class="grid grid-cols-2 gap-2">
                        <button
                            class="touch-manipulation rounded bg-blue-500 py-3 font-semibold text-white hover:bg-blue-600"
                            on:click=|_| send_msg(&ClientMsg::NextHole)
                        >
                            {format!("Play hole {next}")}
                        </button>
                        <button
                            class="touch-manipulation rounded bg-stone-600 py-3 font-semibold text-white hover:bg-stone-500"
                            on:click=|_| send_msg(&ClientMsg::EndGame)
                        >
                            "End the game"
                        </button>
                    </div>
                }
                    .into_any()
            } else {
                view! {
                    <p class="text-center text-sm text-stone-500">
                        "Waiting for the host to play another hole or end the game..."
                    </p>
                }
                    .into_any()
            }}
        </div>
    }
}

fn game_over(g: GameView, results: Vec<HoleResult>, standings: Vec<Standing>) -> impl IntoView {
    let names = g
        .players
        .iter()
        .map(|p| (p.id, p.nickname.clone()))
        .collect();
    view! {
        <div class="space-y-4 rounded-xl border border-stone-200 bg-white p-4 dark:border-stone-700 dark:bg-stone-800">
            {final_standings(g.you, standings, names, || send_msg(&ClientMsg::PlayAgain))}
            <h3 class="pt-2 text-sm font-semibold text-stone-500 uppercase">
                {format!("Hole {}", g.hole)}
            </h3> {results_list(&g, results)}
        </div>
    }
}

// ── Pages ─────────────────────────────────────────────────────────────────────

fn rules_button() -> AnyView {
    view! {
        <RulesButton title="How to play 9 Holes of Golf">
            <GolfRules />
        </RulesButton>
    }
    .into_any()
}

#[component]
fn GolfRules() -> impl IntoView {
    let b = "font-medium text-white";
    view! {
        <RulesSection heading="Goal">
            <p>
                "Have the lowest score. Each hand is a hole, and scores add up from hole to hole."
            </p>
        </RulesSection>
        <RulesSection heading="Setup">
            <p>
                "Everyone is dealt nine cards face down in a 3×3 grid, and one card starts the \
                discard pile. Before play begins, everyone turns up any two of their cards."
            </p>
            <p>"The deck is twelve each of 1 to 12, plus Wild cards (★)."</p>
        </RulesSection>
        <RulesSection heading="Your turn">
            <p>"Do one of these:"</p>
            <ul class="list-disc space-y-1 pl-5">
                <li>
                    <span class=b>"Draw from the deck."</span>
                    " Swap it for any card in your grid (face up or face down), or discard it."
                </li>
                <li>
                    <span class=b>"Take the top discard."</span>
                    " You have to swap it for one of your cards."
                </li>
                <li>
                    <span class=b>"Turn up"</span>
                    " one of your face-down cards."
                </li>
            </ul>
            <p>"A card you swap out goes on the discard pile, and the new card goes in face up."</p>
        </RulesSection>
        <RulesSection heading="Ending a hole">
            <p>
                "When someone has all nine cards face up, everyone else gets one more turn. \
                Then every card is turned up and counted."
            </p>
        </RulesSection>
        <RulesSection heading="Scoring">
            <ul class="list-disc space-y-1 pl-5">
                <li>
                    "Cards are worth their number. " <span class=b>"2s are -2"</span> " and "
                    <span class=b>"Wilds are 0"</span> "."
                </li>
                <li>
                    "A " <span class=b>"column"</span>
                    " of three matching numbers scores 0. Wilds don't match anything, and a column \
                    of 2s still counts -6."
                </li>
            </ul>
            <p>"After each hole the host plays another or ends the game. Lowest total wins."</p>
        </RulesSection>
    }
}

#[component]
pub fn GolfLobby() -> impl IntoView {
    cards::lobby_page(GameKind::Golf, ().into_any(), rules_button)
}

#[component]
pub fn GolfRoom() -> impl IntoView {
    cards::room_page::<GameView>(
        GameKind::Golf,
        |game| view! { <TableView game /> }.into_any(),
        rules_button,
    )
}
