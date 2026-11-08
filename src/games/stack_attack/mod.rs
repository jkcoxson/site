// Jackson Coxson
// Stack Attack: race to empty your stock pile by building shared piles from 1
// up to 12. Plays by the rules of the classic stock-pile card game
// (https://service.mattel.com/instruction_sheets/42050-Skip-Bo.pdf) under its
// own names. Built on the shared card table.

pub mod logic;
#[cfg(feature = "ssr")]
pub mod server;

use leptos::prelude::*;

use crate::games::cards::{
    self, final_standings, hand_view, seat_strip, table_talk, GameKind, HandOrder, SeatChip,
    TAG_DARK, TAG_GOLD, TAG_SOFT,
};
use crate::games::number_cards::{card_back, card_face, card_slot};
use crate::games::rules::{RulesButton, RulesSection};
use logic::{fits, Card, ClientMsg, GameView, Options, PlayerView, Stage, MATCH_POINTS};

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

/// What the player has picked to play: a card from their hand, or the top of
/// a stock or discard pile.
#[derive(Clone, Copy)]
struct Pick {
    hand: RwSignal<Option<u8>>,
    table: RwSignal<Option<u8>>,
}

impl Pick {
    fn card(&self) -> Option<u8> {
        self.hand.get().or(self.table.get())
    }

    fn clear(&self) {
        self.hand.set(None);
        self.table.set(None);
    }

    fn toggle_table(&self, id: u8) {
        self.hand.set(None);
        self.table
            .update(|t| *t = if *t == Some(id) { None } else { Some(id) });
    }
}

/// The picked card, wherever it is.
fn find_card(g: &GameView, id: u8) -> Option<Card> {
    g.hand
        .iter()
        .copied()
        .chain(g.players.iter().flat_map(|p| {
            p.stock_top
                .into_iter()
                .chain(p.discards.iter().filter_map(|d| d.last().copied()))
        }))
        .find(|c| c.id == id)
}

// ── Table ─────────────────────────────────────────────────────────────────────

#[component]
fn TableView(game: Memo<GameView>) -> impl IntoView {
    let pick = Pick {
        hand: RwSignal::new(None),
        table: RwSignal::new(None),
    };
    let playing = Memo::new(move |_| game.with(|g| matches!(g.stage, Stage::Playing)));
    let my_turn = Memo::new(move |_| playing.get() && game.with(|g| g.current == g.you));
    // Picking from the hand drops a pick from the table, and a new turn
    // starts with nothing picked.
    Effect::new(move |_| {
        if pick.hand.get().is_some() {
            pick.table.set(None);
        }
    });
    Effect::new(move |_| {
        if !my_turn.get() {
            pick.clear();
        }
    });
    // A picked table card that has since moved is no longer picked.
    Effect::new(move |_| {
        let still_there = game.with(|g| {
            pick.table.get_untracked().is_none_or(|id| {
                find_card(g, id).is_some_and(|_| !g.hand.iter().any(|c| c.id == id))
            })
        });
        if !still_there {
            pick.table.set(None);
        }
    });

    let turn_text = move || {
        game.with(|g| match &g.stage {
            Stage::Playing if g.current == g.you => match pick.card() {
                Some(_) if pick.hand.get().is_some() => {
                    "Tap a building pile to play it, or one of your discard piles to end your turn."
                        .to_string()
                }
                Some(_) => "Tap a building pile to play it.".into(),
                None if g.hand.is_empty() => {
                    "No cards left to draw. Play what you can, then end your turn.".into()
                }
                None => "Your turn: pick a card from your stock, hand or discards.".into(),
            },
            Stage::Playing => format!("{}'s turn.", name_of(g, g.current)),
            Stage::GameOver { .. } => "Game over.".into(),
        })
    };

    view! {
        <div class="card-table grid gap-4 lg:grid-cols-[1fr_16rem]">
            <div class="min-w-0 space-y-3">
                {move || {
                    game.with(|g| g.options.scored)
                        .then(|| {
                            view! {
                                <div class="flex flex-wrap items-center gap-2 text-sm">
                                    <span class="rounded bg-stone-700 px-3 py-1 font-semibold text-white">
                                        {move || format!("Game {}", game.with(|g| g.game))}
                                    </span>
                                    <span class="rounded bg-amber-400 px-3 py-1 font-semibold text-stone-900">
                                        {format!("First to {MATCH_POINTS}")}
                                    </span>
                                </div>
                            }
                        })
                }}
                {seat_strip(move || {
                    game.with(|g| {
                        let playing = matches!(g.stage, Stage::Playing);
                        g.players
                            .iter()
                            .map(|p| {
                                let mut tags = vec![(format!("stock {}", p.stock_left), TAG_GOLD)];
                                if playing {
                                    tags.push((format!("hand {}", p.hand_size), TAG_SOFT));
                                }
                                if g.partner == Some(p.id) {
                                    tags.push(("partner".to_string(), TAG_DARK));
                                }
                                SeatChip {
                                    id: p.id,
                                    name: p.nickname.clone(),
                                    you: p.id == g.you,
                                    connected: p.connected,
                                    active: playing && g.current == p.id,
                                    score: g.options.scored.then_some(p.score),
                                    tags,
                                }
                            })
                            .collect()
                    })
                })} <Opponents game pick my_turn />
                <div class="space-y-2 rounded-xl bg-green-900/60 p-3 ring-1 ring-green-800 sm:p-5">
                    <div class="flex items-end gap-3 sm:gap-5">
                        <div data-pile="deck" class="shrink-0">
                            {move || card_back(format!("{} left", game.with(|g| g.draw_count)))}
                        </div>
                        <div class="h-16 w-px self-center bg-green-700"></div>
                        <BuildingPiles game pick my_turn />
                    </div>
                    <p class=move || {
                        if my_turn.get() {
                            "text-sm font-semibold text-amber-300"
                        } else {
                            "text-sm text-green-100"
                        }
                    }>{turn_text}</p>
                </div>
                {move || {
                    let g = game.get();
                    match g.stage.clone() {
                        Stage::Playing => None,
                        Stage::GameOver { winners, points, match_over, standings } => {
                            Some(game_over(g, winners, points, match_over, standings))
                        }
                    }
                }} <MyArea game pick my_turn />
            </div>

            {table_talk(move || game.with(|g| g.log.clone()))}
        </div>
    }
}

#[component]
fn BuildingPiles(game: Memo<GameView>, pick: Pick, my_turn: Memo<bool>) -> impl IntoView {
    let fits_pile = move |i: usize| {
        let Some(id) = pick.card() else {
            return false;
        };
        game.with(|g| {
            let height = g.building[i].map_or(0, |b| b.value as usize);
            find_card(g, id).is_some_and(|c| fits(&c, height))
        })
    };
    view! {
        <div class="grid grid-cols-4 gap-2 sm:gap-4">
            {(0..4)
                .map(|i| {
                    view! {
                        <button
                            class=move || {
                                if my_turn.get() && fits_pile(i) {
                                    "card-glow relative touch-manipulation"
                                } else {
                                    "relative cursor-default"
                                }
                            }
                            on:click=move |_| {
                                if !my_turn.get_untracked() {
                                    return;
                                }
                                if let Some(card) = pick.card() {
                                    send_msg(
                                        &ClientMsg::Play {
                                            card,
                                            pile: i as u8,
                                        },
                                    );
                                    pick.clear();
                                }
                            }
                            title="Building pile"
                        >
                            {move || match game.with(|g| g.building[i]) {
                                Some(top) => {
                                    view! {
                                        <div data-card=top.card.id>{card_face(top.card, "")}</div>
                                        {top
                                            .card
                                            .is_wild()
                                            .then(|| {
                                                view! {
                                                    <span class="absolute -right-1.5 -top-1.5 rounded-full bg-stone-900 px-1.5 text-xs font-bold text-amber-300 ring-1 ring-amber-400">
                                                        {top.value}
                                                    </span>
                                                }
                                            })}
                                    }
                                        .into_any()
                                }
                                None => card_slot("1 or ★").into_any(),
                            }}
                        </button>
                    }
                })
                .collect_view()}
        </div>
    }
}

/// Everyone else's stock and discard piles, small. A partner's piles can be
/// picked from.
#[component]
fn Opponents(game: Memo<GameView>, pick: Pick, my_turn: Memo<bool>) -> impl IntoView {
    let others = Memo::new(move |_| {
        game.with(|g| {
            g.players
                .iter()
                .filter(|p| p.id != g.you)
                .cloned()
                .collect::<Vec<PlayerView>>()
        })
    });
    view! {
        <div class="grid gap-2 sm:grid-cols-2">
            {move || {
                others
                    .get()
                    .into_iter()
                    .map(|p| {
                        let usable = game.with(|g| g.partner == Some(p.id));
                        view! {
                            <div class=if usable {
                                "card-mini flex items-end gap-3 rounded-lg bg-green-900/40 p-2 ring-1 ring-amber-500/60"
                            } else {
                                "card-mini flex items-end gap-3 rounded-lg bg-green-900/30 p-2 ring-1 ring-green-800"
                            }>
                                <div class="flex flex-col items-center gap-0.5">
                                    {stock_view(p.stock_top, p.stock_left, usable, pick, my_turn)}
                                    <span class="max-w-[4rem] truncate text-[0.6rem] text-green-100">
                                        {p.nickname.clone()}
                                    </span>
                                </div>
                                <div class="flex gap-1">
                                    {p
                                        .discards
                                        .into_iter()
                                        .map(|pile| discard_pile(
                                            pile,
                                            3,
                                            usable,
                                            None,
                                            pick,
                                            my_turn,
                                        ))
                                        .collect_view()}
                                </div>
                            </div>
                        }
                    })
                    .collect_view()
            }}
        </div>
    }
}

/// A stock pile: the face-up top card over the rest.
fn stock_view(
    top: Option<Card>,
    left: u8,
    usable: bool,
    pick: Pick,
    my_turn: Memo<bool>,
) -> impl IntoView {
    match top {
        Some(card) => {
            let id = card.id;
            view! {
                <button
                    class=move || {
                        if usable && pick.table.get() == Some(id) {
                            "relative rounded-lg ring-4 ring-blue-500 touch-manipulation"
                        } else if usable && my_turn.get() {
                            "relative rounded-lg touch-manipulation"
                        } else {
                            "relative cursor-default"
                        }
                    }
                    on:click=move |_| {
                        if usable && my_turn.get_untracked() {
                            pick.toggle_table(id);
                        }
                    }
                    title="Stock pile"
                >
                    <div data-card=id>{card_face(card, "ring-2 ring-amber-400")}</div>
                    <span class="absolute -bottom-1.5 -right-1.5 rounded-full bg-amber-400 px-1.5 text-xs font-bold text-stone-900">
                        {left}
                    </span>
                </button>
            }
            .into_any()
        }
        None => card_slot("empty").into_any(),
    }
}

/// One discard pile, fanned downward so its cards can be read. Only the top
/// card can be played. `shown` caps how many cards are drawn. With
/// `discard_to` set, tapping it while a hand card is picked discards there.
fn discard_pile(
    pile: Vec<Card>,
    shown: usize,
    usable: bool,
    discard_to: Option<u8>,
    pick: Pick,
    my_turn: Memo<bool>,
) -> impl IntoView {
    let top = pile.last().map(|c| c.id);
    let hidden = pile.len().saturating_sub(shown);
    let visible: Vec<Card> = pile.into_iter().skip(hidden).collect();
    let on_click = move |_| {
        if !my_turn.get_untracked() {
            return;
        }
        if let (Some(card), Some(to)) = (pick.hand.get_untracked(), discard_to) {
            send_msg(&ClientMsg::Discard { card, pile: to });
            pick.clear();
        } else if let (Some(id), true) = (top, usable) {
            pick.toggle_table(id);
        }
    };
    let glow = move || my_turn.get() && discard_to.is_some() && pick.hand.get().is_some();
    view! {
        <button
            class=move || {
                if glow() { "card-glow discard-fan touch-manipulation" } else { "discard-fan" }
            }
            on:click=on_click
            title="Discard pile"
        >
            {(hidden > 0)
                .then(|| {
                    view! {
                        <span class="block text-center text-[0.6rem] text-green-200">
                            {format!("+{hidden}")}
                        </span>
                    }
                })}
            {if visible.is_empty() {
                card_slot(if discard_to.is_some() { "discard" } else { "" }).into_any()
            } else {
                visible
                    .into_iter()
                    .map(|c| {
                        let id = c.id;
                        let is_top = Some(id) == top;
                        view! {
                            <div
                                data-card=id
                                class=move || {
                                    if is_top && pick.table.get() == Some(id) {
                                        "rounded-lg ring-4 ring-blue-500"
                                    } else {
                                        ""
                                    }
                                }
                            >
                                {card_face(c, "")}
                            </div>
                        }
                    })
                    .collect_view()
                    .into_any()
            }}
        </button>
    }
}

fn send_order(order: Vec<u8>) {
    send_msg(&ClientMsg::Arrange { order });
}

/// Your stock, discard piles and hand.
#[component]
fn MyArea(game: Memo<GameView>, pick: Pick, my_turn: Memo<bool>) -> impl IntoView {
    let order = HandOrder::new(send_order);
    let hand = Memo::new(move |_| game.with(|g| g.hand.clone()));
    let me = Memo::new(move |_| game.with(|g| g.players.iter().find(|p| p.id == g.you).cloned()));
    let can_pass = move || my_turn.get() && hand.with(|h| h.is_empty());
    view! {
        <div class="space-y-3">
            <div class="flex items-start gap-3 rounded-xl bg-green-900/40 p-3 ring-1 ring-green-800 sm:gap-5">
                {move || {
                    me.get()
                        .map(|p| {
                            view! {
                                <div class="flex flex-col items-center gap-1">
                                    {stock_view(p.stock_top, p.stock_left, true, pick, my_turn)}
                                    <span class="text-[0.65rem] text-green-100">"Your stock"</span>
                                </div>
                                <div class="h-16 w-px self-center bg-green-700"></div>
                                <div class="grid grid-cols-4 gap-2 sm:gap-3">
                                    {p
                                        .discards
                                        .into_iter()
                                        .enumerate()
                                        .map(|(i, pile)| {
                                            discard_pile(pile, 5, true, Some(i as u8), pick, my_turn)
                                        })
                                        .collect_view()}
                                </div>
                            }
                        })
                }}
            </div>
            {hand_view(hand, pick.hand, order, |card: Card| card_face(card, ""), |_| false)}
            <p class="text-xs text-stone-500 dark:text-stone-400">
                "Tap a card to pick it. Drag cards to rearrange your hand."
            </p>
            {move || {
                can_pass()
                    .then(|| {
                        view! {
                            <button
                                class="w-full touch-manipulation rounded-lg bg-blue-500 py-3 font-semibold text-white hover:bg-blue-600"
                                on:click=|_| send_msg(&ClientMsg::Pass)
                            >
                                "End turn"
                            </button>
                        }
                    })
            }}
        </div>
    }
}

fn game_over(
    g: GameView,
    winners: Vec<u8>,
    points: Option<i32>,
    match_over: bool,
    standings: Vec<cards::Standing>,
) -> impl IntoView {
    let names = winners
        .iter()
        .map(|&w| name_of(&g, w))
        .collect::<Vec<_>>()
        .join(" and ");
    let headline = match points {
        Some(points) => format!("{names} emptied the stock and scored {points}."),
        None => format!("{names} emptied the stock first!"),
    };
    let names: Vec<(u8, String)> = g
        .players
        .iter()
        .map(|p| (p.id, p.nickname.clone()))
        .collect();
    let host = g.you == g.host;
    view! {
        <div class="space-y-3 rounded-xl border border-stone-200 bg-white p-4 dark:border-stone-700 dark:bg-stone-800">
            <p class="text-center font-semibold dark:text-white">{headline}</p>
            {if match_over {
                view! {
                    <p class="text-center text-xs text-stone-500">
                        {if points.is_some() {
                            "Match points"
                        } else {
                            "Cards left in each stock pile"
                        }}
                    </p>
                    {final_standings(g.you, standings, names, || send_msg(&ClientMsg::PlayAgain))}
                }
                    .into_any()
            } else if host {
                view! {
                    <button
                        class="w-full rounded bg-blue-500 py-3 font-semibold text-white hover:bg-blue-600"
                        on:click=|_| send_msg(&ClientMsg::NextGame)
                    >
                        {format!("Deal game {}", g.game + 1)}
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

// ── Pages ─────────────────────────────────────────────────────────────────────

fn rules_button() -> AnyView {
    view! {
        <RulesButton title="How to play Stack Attack">
            <StackAttackRules />
        </RulesButton>
    }
    .into_any()
}

#[component]
fn StackAttackRules() -> impl IntoView {
    let b = "font-medium text-white";
    view! {
        <RulesSection heading="Goal">
            <p>"Be the first to play every card in your " <span class=b>"stock pile"</span> "."</p>
        </RulesSection>
        <RulesSection heading="The cards">
            <p>
                "162 cards: twelve each of 1 to 12, plus 18 " <span class=b>"Wild"</span>
                " cards (★) that can be any number you need."
            </p>
            <p>
                "Everyone gets a face-down stock pile of 30 cards (20 with five or more \
                players) with its top card turned up. The rest is the draw pile."
            </p>
        </RulesSection>
        <RulesSection heading="Your turn">
            <ol class="list-decimal space-y-1 pl-5">
                <li>"Draw until you hold 5 cards. This happens for you."</li>
                <li>
                    "Play cards onto the four shared " <span class=b>"building piles"</span>
                    " from your hand, the top of your stock, or the tops of your discard piles."
                </li>
                <li>
                    "End your turn by putting one card from your hand on any of your four "
                    <span class=b>"discard piles"</span>". They don't need to be in order."
                </li>
            </ol>
            <p>
                "If you play all five cards from your hand, you draw five more right away \
                and keep going. Discarding your last card doesn't count."
            </p>
        </RulesSection>
        <RulesSection heading="Building piles">
            <p>
                "A building pile starts with a 1 or a Wild, and cards go on in order up to 12. \
                There can only be four piles at once. When a pile reaches 12 it's cleared \
                away, and finished piles are shuffled back in when the draw pile runs out."
            </p>
            <p>
                "Play from your stock whenever you can, since that's what you're trying to empty."
            </p>
        </RulesSection>
        <RulesSection heading="Options">
            <ul class="list-disc space-y-1 pl-5">
                <li>
                    <span class=b>"Short game"</span>
                    ": stock piles of only 10 cards."
                </li>
                <li>
                    <span class=b>"Partners"</span>
                    " (four or six players): partners sit across from each other and may play \
                    from each other's stock and discard piles. The team wins when both stocks \
                    are empty."
                </li>
                <li>
                    <span class=b>"Scored match"</span>
                    ": the winner of each game scores 25 points plus 5 for every card left in \
                    the other players' stock piles. First to 500 wins."
                </li>
            </ul>
        </RulesSection>
    }
}

fn options_form() -> impl IntoView {
    let options = RwSignal::new(Options::default());
    let toggle =
        move |label: &'static str, get: fn(&Options) -> bool, set: fn(&mut Options, bool)| {
            view! {
                <label class="flex items-center gap-2 text-sm dark:text-stone-200">
                    <input
                        type="checkbox"
                        prop:checked=move || options.with(get)
                        on:change=move |ev| {
                            let on = event_target_checked(&ev);
                            options.update(|o| set(o, on));
                        }
                    />
                    {label}
                </label>
            }
        };
    view! {
        <div class="space-y-1 rounded border border-stone-200 p-3 dark:border-stone-600">
            {toggle("Short game (stock of 10)", |o| o.short, |o, v| o.short = v)}
            {toggle("Partners (4 or 6 players)", |o| o.partners, |o, v| o.partners = v)}
            {toggle("Scored match to 500", |o| o.scored, |o, v| o.scored = v)}
            <input
                type="hidden"
                name="options"
                prop:value=move || serde_json::to_string(&options.get()).unwrap_or_default()
            />
        </div>
    }
}

#[component]
pub fn StackAttackLobby() -> impl IntoView {
    cards::lobby_page(
        GameKind::StackAttack,
        options_form().into_any(),
        rules_button,
    )
}

#[component]
pub fn StackAttackRoom() -> impl IntoView {
    cards::room_page::<GameView>(
        GameKind::StackAttack,
        |game| view! { <TableView game /> }.into_any(),
        rules_button,
    )
}
