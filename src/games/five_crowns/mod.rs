// Jackson Coxson
// Five Crowns: the rummy-style card game where the wild card changes every
// round. Rules: https://www.playmonster.com/wp-content/uploads/2019/09/5Crowns_RULES.pdf
// The lobby, rooms, hand and card motion come from the shared card table.

pub mod logic;
#[cfg(feature = "ssr")]
pub mod server;

use leptos::prelude::*;

use crate::games::cards::{
    self, final_standings, hand_view, seat_strip, table_talk, GameKind, HandOrder, SeatChip,
    TAG_DARK, TAG_GOLD,
};
use crate::games::rules::{RulesButton, RulesSection};
use cards::Standing;
use logic::{
    best_meld, hand_size, rank_label, sort_for_display, Card, ClientMsg, GameView, Meld,
    RoundResult, Stage, Suit, ROUNDS,
};

fn send_msg(msg: &ClientMsg) {
    cards::send(msg);
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
            "card-face {ring} {} relative select-none overflow-hidden rounded-lg bg-white font-bold shadow-md",
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
        <div class="card-face flex select-none flex-col items-center justify-center rounded-lg bg-gradient-to-br from-indigo-700 to-purple-800 text-amber-300 shadow-md ring-1 ring-indigo-400">
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
        <div class="card-table grid gap-4 lg:grid-cols-[1fr_16rem]">
            <div class="min-w-0 space-y-3">
                <div class="flex flex-wrap items-center gap-2 text-sm">
                    <span class="rounded bg-stone-700 px-3 py-1 font-semibold text-white">
                        {move || format!("Round {}/{ROUNDS}", game.with(|g| g.round))}
                    </span>
                    <span class="rounded bg-amber-400 px-3 py-1 font-semibold text-stone-900">
                        {move || format!("{}s are wild", rank_label(wild.get()))}
                    </span>
                </div>

                {seat_strip(move || {
                    game.with(|g| {
                        let playing = matches!(g.stage, Stage::Playing { .. });
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
                            if can_draw() && game.with(|g| g.discard_top.is_some()) {
                                "card-glow shrink-0 touch-manipulation"
                            } else {
                                "shrink-0 cursor-default"
                            }
                        }
                        disabled=move || !(can_draw() && game.with(|g| g.discard_top.is_some()))
                        on:click=|_| send_msg(
                            &ClientMsg::Draw {
                                from_discard: true,
                            },
                        )
                        title="Take the top discard"
                    >
                        {move || match game.with(|g| g.discard_top) {
                            Some(c) => {
                                view! {
                                    <div data-card=c.id>{card_face(c, wild.get_untracked())}</div>
                                }
                                    .into_any()
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
                        view! { {results_view} }.into_any()
                    }
                    StageKind::GameOver => {
                        let results_view = move || {
                            let g = game.get();
                            let Stage::GameOver { results, standings } = g.stage.clone() else {
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

fn send_order(order: Vec<u8>) {
    send_msg(&ClientMsg::Arrange { order });
}

#[component]
fn HandView(game: Memo<GameView>, my_turn: Memo<bool>, drawn: Memo<bool>) -> impl IntoView {
    let selected = RwSignal::new(None::<u8>);
    let order = HandOrder::new(send_order);
    let wild = move || game.with(|g| g.wild);
    let hand = Memo::new(move |_| game.with(|g| g.hand.clone()));

    let sort = move |by_rank: bool| {
        let w = wild();
        let mut cards = order.apply(hand.get_untracked());
        if by_rank {
            cards.sort_by_key(|c| (c.is_wild(w), c.rank, c.suit, c.id));
        } else {
            sort_for_display(&mut cards, w);
        }
        order.arrange(cards.iter().map(|c| c.id).collect());
    };

    let preview = move || {
        let mut hand = hand.get();
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
            {hand_view(hand, selected, order, move |card: Card| card_face(card, wild()), |_| false)}
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
            }} <div class="flex gap-2">
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
                            {r
                                .went_out
                                .then(|| {
                                    view! {
                                        <span class="ml-2 text-xs text-amber-500">"went out"</span>
                                    }
                                })}
                        </span>
                        <span class=if r.meld.points == 0 {
                            "text-green-500"
                        } else {
                            "text-red-400"
                        }>{format!("+{}", r.meld.points)}</span>
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
        .map(|p| (p.id, p.nickname.clone()))
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
        <RulesButton title="How to play Five Crowns">
            <FiveCrownsRules />
        </RulesButton>
    }
    .into_any()
}

#[component]
fn FiveCrownsRules() -> impl IntoView {
    let b = "font-medium text-white";
    view! {
        <RulesSection heading="Goal">
            <p>
                "Turn your whole hand into books and runs before anyone else, and keep the \
                points left in your hand low. Lowest total after eleven rounds wins."
            </p>
        </RulesSection>
        <RulesSection heading="The cards">
            <p>
                "Two decks of five suits (★ ♥ ♣ ♠ ♦), ranks 3 to King, plus jokers. Round \
                one deals three cards, round two four, up to thirteen in the last round."
            </p>
            <p>
                "The "<span class=b>"wild card"</span>" changes every round: it's the rank \
                matching the number of cards dealt, so 3s are wild in round one and Kings \
                in the last. Jokers are always wild."
            </p>
        </RulesSection>
        <RulesSection heading="Your turn">
            <p>"Draw the top card of the deck or the discard pile, then discard one card."</p>
            <ul class="list-disc space-y-1 pl-5">
                <li>
                    <span class=b>"Books"</span>
                    ": three or more cards of the same rank, like 7♥ 7♣ 7♠."
                </li>
                <li>
                    <span class=b>"Runs"</span>
                    ": three or more cards in a row of one suit, like 9♦ 10♦ J♦."
                </li>
                <li>"Wild cards stand in for anything in a book or run."</li>
            </ul>
            <p>
                "When everything but your discard fits into books and runs, you can "
                <span class=b>"go out"</span>
                ". Everyone else then gets one last turn, and their hands are laid down as \
                well as they can be. The table works out the best lay-down for you."
            </p>
        </RulesSection>
        <RulesSection heading="Scoring">
            <p>
                "Cards left over count against you: face value for 3 to 10, 11 for Jacks, 12 \
                for Queens, 13 for Kings, 20 for the round's wild cards and 50 for jokers."
            </p>
        </RulesSection>
    }
}

#[component]
pub fn FiveCrownsLobby() -> impl IntoView {
    cards::lobby_page(GameKind::FiveCrowns, ().into_any(), rules_button)
}

#[component]
pub fn FiveCrownsRoom() -> impl IntoView {
    cards::room_page::<GameView>(
        GameKind::FiveCrowns,
        |game| view! { <TableView game /> }.into_any(),
        rules_button,
    )
}
