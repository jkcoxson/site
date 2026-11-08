// Jackson Coxson
// Pirate's Wager: a pirate trick-taking game where every round you bid
// exactly how many tricks you'll take. Built on the shared card table, which
// supplies the lobby, rooms, hand and card motion.

pub mod logic;
#[cfg(feature = "ssr")]
pub mod server;

use leptos::prelude::*;

use crate::games::cards::{
    self, final_standings, hand_view, seat_strip, table_talk, GameKind, HandOrder, Motion,
    SeatChip, Spot, TAG_DARK, TAG_GREEN, TAG_RED, TAG_SOFT,
};
use crate::games::rules::{RulesButton, RulesSection};
use logic::{
    can_play, Card, ClientMsg, Crew, Face, GameView, Length, Options, Played, RoundResult, Stage,
    Suit, TrickView, GHOST,
};

fn send_msg(msg: &ClientMsg) {
    cards::send(msg);
}

// ── Cards ─────────────────────────────────────────────────────────────────────

/// Colors, corner label, centre picture and caption for a card.
fn look(card: &Card) -> (&'static str, String, &'static str, &'static str) {
    match card.face {
        Face::Number { suit, rank } => {
            let colors = match suit {
                Suit::Palms => "bg-white text-green-700 ring-1 ring-stone-300",
                Suit::Pearls => "bg-white text-purple-700 ring-1 ring-stone-300",
                Suit::Coins => "bg-white text-amber-600 ring-1 ring-stone-300",
                Suit::Cutlasses => "bg-stone-900 text-white ring-1 ring-stone-500",
            };
            (colors, rank.to_string(), suit.symbol(), "")
        }
        Face::Flag => (
            "bg-sky-50 text-sky-700 ring-1 ring-sky-300",
            "🏳️".into(),
            "🏳️",
            "FLAG",
        ),
        Face::Pirate(crew) => (
            "bg-red-700 text-white ring-1 ring-red-900",
            "☠️".into(),
            "☠️",
            match crew {
                Crew::Navigator => "NAVIGATOR",
                Crew::Smuggler => "SMUGGLER",
                Crew::Gambler => "GAMBLER",
                Crew::Seer => "SEER",
                Crew::Brute => "BRUTE",
            },
        ),
        Face::Turncoat => (
            "bg-gradient-to-br from-red-700 to-sky-300 text-white ring-1 ring-red-900",
            "🎭".into(),
            "🎭",
            "TURNCOAT",
        ),
        Face::Captain => (
            "bg-stone-900 text-amber-400 ring-2 ring-amber-400",
            "👑".into(),
            "👑",
            "CAPTAIN",
        ),
        Face::Mermaid => (
            "bg-teal-500 text-white ring-1 ring-teal-700",
            "🧜".into(),
            "🧜",
            "MERMAID",
        ),
        Face::Kraken => (
            "bg-indigo-950 text-pink-300 ring-1 ring-pink-400",
            "🐙".into(),
            "🐙",
            "KRAKEN",
        ),
        Face::Whale => (
            "bg-slate-100 text-slate-600 ring-1 ring-slate-400",
            "🐋".into(),
            "🐋",
            "WHALE",
        ),
        Face::Loot => (
            "bg-amber-200 text-amber-900 ring-1 ring-amber-500",
            "💰".into(),
            "💰",
            "LOOT",
        ),
    }
}

/// A full-size card. The corner shows when cards overlap in the hand.
fn card_face(card: Card) -> impl IntoView {
    let (colors, corner, picture, caption) = look(&card);
    view! {
        <div class=format!(
            "card-face {colors} relative select-none overflow-hidden rounded-lg font-bold shadow-md",
        )>
            <div class="absolute left-1 top-1 flex flex-col items-center leading-none">
                <span class="text-[0.95rem] tracking-tighter sm:text-lg">{corner}</span>
            </div>
            <div class="absolute inset-0 flex items-center justify-center pl-2 pt-2 text-2xl sm:text-3xl">
                {picture}
            </div>
            {(!caption.is_empty())
                .then(|| {
                    view! {
                        <span class="absolute inset-x-0 bottom-0.5 text-center text-[0.45rem] tracking-tight sm:text-[0.55rem]">
                            {caption}
                        </span>
                    }
                })}
        </div>
    }
}

fn card_back(label: String) -> impl IntoView {
    view! {
        <div class="card-face flex select-none flex-col items-center justify-center rounded-lg bg-gradient-to-br from-slate-800 to-stone-900 text-2xl shadow-md ring-1 ring-amber-700">
            <span class="leading-none">"🏴‍☠️"</span>
            <span class="mt-1 text-[0.6rem] font-semibold text-amber-200">{label}</span>
        </div>
    }
}

/// A row of small cards (the Seer's peek, the Smuggler's pick).
fn mini_cards(cards: Vec<Card>) -> impl IntoView {
    view! {
        <div class="flex flex-wrap gap-1" style="--cw: 3rem">
            {cards.into_iter().map(card_face).collect_view()}
        </div>
    }
}

// ── Lobby ─────────────────────────────────────────────────────────────────────

fn rules_button() -> AnyView {
    view! {
        <RulesButton title="How to play Pirate's Wager">
            <PiratesWagerRules />
        </RulesButton>
    }
    .into_any()
}

#[component]
fn PiratesWagerRules() -> impl IntoView {
    let b = "font-medium text-white";
    view! {
        <RulesSection heading="Goal">
            <p>
                "Each round, bid exactly how many tricks you'll win. Hit your bid and you \
                score; miss and you lose points. Most points after the last round wins."
            </p>
        </RulesSection>
        <RulesSection heading="A round">
            <p>
                "Round one deals one card each, round two two cards, and so on up to ten. \
                After looking at your hand, everyone bids at once; bids stay hidden until \
                everyone is in."
            </p>
            <p>
                "The leader plays any card. If it's a numbered card, everyone who plays a \
                numbered card must follow that suit if they can. Special cards can always \
                be played. The highest card of the suit led wins, but "
                <span class=b>"⚔️ cutlasses"</span>
                " are trumps and beat the other suits. The winner leads the next trick."
            </p>
        </RulesSection>
        <RulesSection heading="Special cards">
            <ul class="list-disc space-y-1 pl-5">
                <li>
                    <span class=b>"☠️ Pirates"</span>
                    " beat every number. Between pirates, the first one played wins."
                </li>
                <li>
                    <span class=b>"👑 The Captain"</span>
                    " beats every pirate."
                </li>
                <li>
                    <span class=b>"🧜 Mermaids"</span>
                    " beat numbers and the Captain, but lose to pirates."
                </li>
                <li>
                    <span class=b>"🏳️ Flags"</span>
                    " always lose. If everyone plays one, the first wins."
                </li>
                <li>
                    <span class=b>"🎭 The Turncoat"</span>
                    " plays as a pirate or a flag, your choice."
                </li>
                <li>
                    "A flag lead lets the next card set the suit. A character lead means \
                    there's no suit to follow."
                </li>
            </ul>
        </RulesSection>
        <RulesSection heading="Scoring">
            <ul class="list-disc space-y-1 pl-5">
                <li>
                    "Bid one or more and make it: 20 points per trick. Miss: −10 per trick you were off."
                </li>
                <li>
                    "Bid zero and take nothing: 10 × cards dealt. Take any: −10 × cards dealt."
                </li>
                <li>
                    "Bonuses, only when you make your bid: 10 per 14 you capture (20 for the \
                    cutlass 14), 20 per mermaid taken by a pirate, 30 per pirate taken by the \
                    Captain, 40 for a mermaid taking the Captain."
                </li>
            </ul>
        </RulesSection>
        <RulesSection heading="Optional cards">
            <ul class="list-disc space-y-1 pl-5">
                <li>
                    <span class=b>"🐙 Kraken"</span>
                    ": nobody wins the trick. Whoever would have won leads next."
                </li>
                <li>
                    <span class=b>"🐋 White Whale"</span>
                    ": special cards are wiped out and the highest number wins, ignoring \
                    suits. If both beasts are played, the later one counts."
                </li>
                <li>
                    <span class=b>"💰 Loot"</span>
                    ": plays as a flag and allies you with the trick's winner. If you both \
                    make your bids, you each get 20."
                </li>
                <li>
                    <span class=b>"Crew abilities"</span>
                    ": win a trick with a pirate to use its power. Navigator: choose who \
                    leads. Smuggler: draw two, discard two. Gambler: bet 10 or 20 on your \
                    bid. Seer: peek at the undealt cards. Brute: change your bid by one (even \
                    after the last trick)."
                </li>
            </ul>
        </RulesSection>
        <RulesSection heading="Two players and ties">
            <p>
                "With two players, a ghost hand plays second in every trick, flipping its \
                top card. A tie for the lead after the last round plays one more round."
            </p>
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
        <div class="space-y-2 rounded border border-stone-200 p-3 dark:border-stone-600">
            <select
                class="w-full rounded border border-stone-300 bg-white px-2 py-2 text-sm dark:border-stone-600 dark:bg-stone-700 dark:text-white"
                on:change=move |ev| {
                    let key = event_target_value(&ev);
                    if let Some(length) = Length::ALL.into_iter().find(|l| l.key() == key) {
                        options.update(|o| o.length = length);
                    }
                }
            >
                {Length::ALL
                    .into_iter()
                    .map(|l| view! { <option value=l.key()>{l.label()}</option> })
                    .collect_view()}
            </select>
            <div class="grid grid-cols-2 gap-1">
                {toggle("🐙 Kraken", |o| o.kraken, |o, v| o.kraken = v)}
                {toggle("🐋 White Whale", |o| o.whale, |o, v| o.whale = v)}
                {toggle("💰 Loot (3+ players)", |o| o.loot, |o, v| o.loot = v)}
                {toggle("☠️ Crew abilities", |o| o.crew, |o, v| o.crew = v)}
            </div>
            <input
                type="hidden"
                name="options"
                prop:value=move || serde_json::to_string(&options.get()).unwrap_or_default()
            />
        </div>
    }
}

// ── Table ─────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum StageKind {
    Bidding,
    Playing,
    Ability,
    RoundOver,
    GameOver,
}

fn name_of(g: &GameView, id: u8) -> String {
    if id == GHOST {
        return "The ghost".into();
    }
    g.players
        .iter()
        .find(|p| p.id == id)
        .map(|p| p.nickname.clone())
        .unwrap_or_default()
}

/// How long a finished trick stays on the table before it's swept away.
const TRICK_HOLD_MS: i32 = 1800;

#[component]
fn TableView(game: Memo<GameView>) -> impl IntoView {
    let stage = Memo::new(move |_| {
        game.with(|g| match g.stage {
            Stage::Bidding => StageKind::Bidding,
            Stage::Playing => StageKind::Playing,
            Stage::Ability { .. } => StageKind::Ability,
            Stage::RoundOver { .. } => StageKind::RoundOver,
            Stage::GameOver { .. } => StageKind::GameOver,
        })
    });
    let my_turn = Memo::new(move |_| game.with(|g| g.current == Some(g.you)));

    // A finished trick stays up for a moment, then flies to its winner.
    let last_trick = Memo::new(move |_| game.with(|g| g.last_trick.clone()));
    let swept = RwSignal::new(None::<Vec<u8>>);
    let ids = |t: &TrickView| t.cards.iter().map(|p| p.card.id).collect::<Vec<u8>>();
    Effect::new(move |_| {
        let Some(t) = last_trick.get() else {
            return;
        };
        let these = ids(&t);
        if swept.get_untracked().as_ref() == Some(&these) {
            return;
        }
        cards::after(TRICK_HOLD_MS, move || {
            if last_trick.get_untracked().as_ref().map(ids).as_ref() != Some(&these) {
                return;
            }
            let motion = Motion {
                leave: these.iter().map(|&id| (Spot::Card(id), t.winner)).collect(),
                ..Motion::default()
            };
            cards::animate_change(motion, move || swept.set(Some(these)));
        });
    });
    // The cards on the table: the trick in play, or the one just finished.
    let shown = Memo::new(move |_| {
        game.with(|g| {
            if !g.trick.is_empty() {
                return (g.trick.clone(), None);
            }
            match &g.last_trick {
                Some(t) if swept.get().as_ref() != Some(&ids(t)) => {
                    (t.cards.clone(), Some(t.clone()))
                }
                _ => (Vec::new(), None),
            }
        })
    });

    let status = move || {
        game.with(|g| match &g.stage {
            Stage::Bidding => {
                let me = g.players.iter().find(|p| p.id == g.you);
                match me.and_then(|p| p.bid) {
                    None => "How many tricks will you take? Bid below.".to_string(),
                    Some(b) => {
                        let waiting: Vec<String> = g
                            .players
                            .iter()
                            .filter(|p| !p.has_bid)
                            .map(|p| p.nickname.clone())
                            .collect();
                        format!("You bid {b}. Waiting on {}.", waiting.join(", "))
                    }
                }
            }
            Stage::Playing => match g.current {
                Some(c) if c == g.you && g.trick.is_empty() => "Your lead. Play any card.".into(),
                Some(c) if c == g.you => "Your turn.".into(),
                Some(c) => format!("{} to play.", name_of(g, c)),
                None => String::new(),
            },
            Stage::Ability { player, crew, .. } => {
                if *player == g.you {
                    format!("You won with {}! {}", crew.name(), crew.ability())
                } else {
                    format!("{} is using {}...", name_of(g, *player), crew.name())
                }
            }
            Stage::RoundOver { .. } => "Round over.".into(),
            Stage::GameOver { .. } => "Game over.".into(),
        })
    };

    let seats = move || {
        game.with(|g| {
            let bidding = matches!(g.stage, Stage::Bidding);
            let mut chips: Vec<SeatChip> = g
                .players
                .iter()
                .map(|p| {
                    let mut tags = Vec::new();
                    match p.bid {
                        Some(bid) if !bidding => {
                            let class = if p.won == bid {
                                TAG_GREEN
                            } else if p.won > bid {
                                TAG_RED
                            } else {
                                TAG_SOFT
                            };
                            tags.push((format!("{}/{bid}", p.won), class));
                        }
                        _ if bidding => tags.push((
                            if p.has_bid { "bid in" } else { "bidding…" }.to_string(),
                            if p.has_bid { TAG_GREEN } else { TAG_SOFT },
                        )),
                        _ => {}
                    }
                    if p.bet > 0 {
                        tags.push((format!("bet {}", p.bet), TAG_DARK));
                    }
                    let active = match &g.stage {
                        Stage::Bidding => !p.has_bid,
                        Stage::Playing => g.current == Some(p.id),
                        Stage::Ability { player, .. } => *player == p.id,
                        _ => false,
                    };
                    SeatChip {
                        id: p.id,
                        name: p.nickname.clone(),
                        you: p.id == g.you,
                        connected: p.connected,
                        active,
                        score: Some(p.total),
                        tags,
                    }
                })
                .collect();
            if g.ghost {
                chips.push(SeatChip {
                    id: GHOST,
                    name: "👻 Ghost".into(),
                    you: false,
                    connected: true,
                    active: g.current == Some(GHOST),
                    score: None,
                    tags: vec![("plays 2nd".into(), TAG_SOFT)],
                });
            }
            chips
        })
    };

    view! {
        <div class="card-table grid gap-4 lg:grid-cols-[1fr_16rem]">
            <div class="min-w-0 space-y-3">
                <div class="flex flex-wrap items-center gap-2 text-sm">
                    <span class="rounded bg-stone-700 px-3 py-1 font-semibold text-white">
                        {move || game.with(|g| format!("Round {}/{}", g.round, g.rounds))}
                    </span>
                    <span class="rounded bg-amber-400 px-3 py-1 font-semibold text-stone-900">
                        {move || {
                            game.with(|g| {
                                if g.dealt == 1 {
                                    "1 card".to_string()
                                } else {
                                    format!("{} cards", g.dealt)
                                }
                            })
                        }}
                    </span>
                </div>

                {seat_strip(seats)}

                <div class="flex items-center gap-3 rounded-xl bg-sky-900/70 p-3 ring-1 ring-sky-800 sm:gap-5 sm:p-5">
                    <div data-pile="deck" class="shrink-0">
                        {card_back("deck".into())}
                    </div>
                    <div class="flex min-h-[calc(var(--cw)*1.4+1.25rem)] min-w-0 flex-1 flex-wrap items-start gap-2">
                        {move || {
                            let (cards, result) = shown.get();
                            if cards.is_empty() {
                                return view! {
                                    <p class=move || {
                                        if my_turn.get() {
                                            "self-center text-sm font-semibold text-amber-300"
                                        } else {
                                            "self-center text-sm text-sky-100"
                                        }
                                    }>{status}</p>
                                }
                                    .into_any();
                            }
                            let winner_card = result
                                .as_ref()
                                .and_then(|r| {
                                    let w = r.winner?;
                                    let won_with = logic::resolve(&r.cards).winner?;
                                    Some((w, r.cards[won_with].card.id))
                                });
                            game.with_untracked(|g| {
                                    cards
                                        .into_iter()
                                        .map(|p| trick_card(g, p, winner_card.map(|w| w.1)))
                                        .collect_view()
                                })
                                .into_any()
                        }}
                    </div>
                </div>
                <p class=move || {
                    if my_turn.get() {
                        "min-h-5 text-sm font-semibold text-amber-600 dark:text-amber-300"
                    } else {
                        "min-h-5 text-sm text-stone-600 dark:text-stone-300"
                    }
                }>
                    {move || {
                        let (_, result) = shown.get();
                        match result {
                            Some(t) => {
                                game.with(|g| {
                                    let mut s = match t.winner {
                                        Some(w) if w == g.you => "You take the trick!".to_string(),
                                        Some(w) => format!("{} takes the trick.", name_of(g, w)),
                                        None => {
                                            format!("Nobody wins. {} leads.", name_of(g, t.next))
                                        }
                                    };
                                    if !t.why.is_empty() {
                                        s = format!("{} {s}", t.why);
                                    }
                                    if t.bonus > 0 && t.winner.is_some_and(|w| w != GHOST) {
                                        s.push_str(&format!(" +{} bonus.", t.bonus));
                                    }
                                    s
                                })
                            }
                            None if !game.with(|g| g.trick.is_empty()) => status(),
                            None => String::new(),
                        }
                    }}
                </p>

                {move || match stage.get() {
                    StageKind::Bidding => {
                        view! {
                            <BidPanel game />
                            <HandPanel game my_turn />
                        }
                            .into_any()
                    }
                    StageKind::Playing => view! { <HandPanel game my_turn /> }.into_any(),
                    StageKind::Ability => {
                        view! {
                            <AbilityPanel game />
                            <HandPanel game my_turn />
                        }
                            .into_any()
                    }
                    StageKind::RoundOver => {
                        let results = move || {
                            let g = game.get();
                            let Stage::RoundOver { results } = g.stage.clone() else {
                                return None;
                            };
                            Some(round_over(g, results))
                        };
                        view! { {results} }.into_any()
                    }
                    StageKind::GameOver => {
                        let results = move || {
                            let g = game.get();
                            let Stage::GameOver { results, standings } = g.stage.clone() else {
                                return None;
                            };
                            let names = g
                                .players
                                .iter()
                                .map(|p| (p.id, p.nickname.clone()))
                                .collect();
                            Some(
                                view! {
                                    <div class="space-y-4">
                                        {final_standings(
                                            g.you,
                                            standings,
                                            names,
                                            || send_msg(&ClientMsg::PlayAgain),
                                        )}
                                        <h3 class="pt-2 text-sm font-semibold text-stone-500 uppercase">
                                            "Final round"
                                        </h3> {results_table(&g, results)}
                                    </div>
                                },
                            )
                        };
                        view! { {results} }.into_any()
                    }
                }}
            </div>

            <div class="space-y-3 lg:self-start">
                {table_talk(move || game.with(|g| g.log.clone()))}
            </div>
        </div>
    }
}

/// A card on the table with who played it underneath.
fn trick_card(g: &GameView, p: Played, winner: Option<u8>) -> impl IntoView {
    let won = winner == Some(p.card.id);
    let lost = winner.is_some() && !won;
    let label = if p.player == g.you {
        "You".to_string()
    } else {
        name_of(g, p.player)
    };
    let note =
        (p.card.face == Face::Turncoat).then(|| if p.pirate { "as pirate" } else { "as flag" });
    view! {
        <div class=if lost {
            "flex w-[var(--cw)] flex-col items-center gap-0.5 opacity-60 transition-opacity"
        } else {
            "flex w-[var(--cw)] flex-col items-center gap-0.5 transition-opacity"
        }>
            <div data-card=p.card.id class=if won { "card-won rounded-lg" } else { "rounded-lg" }>
                {card_face(p.card)}
            </div>
            <span class="w-full truncate text-center text-[0.65rem] text-sky-100">{label}</span>
            {note.map(|n| view! { <span class="text-[0.6rem] text-amber-200">{n}</span> })}
        </div>
    }
}

#[component]
fn BidPanel(game: Memo<GameView>) -> impl IntoView {
    let mine = Memo::new(move |_| {
        game.with(|g| g.players.iter().find(|p| p.id == g.you).and_then(|p| p.bid))
    });
    view! {
        <div class="rounded-lg border border-stone-200 bg-white p-3 dark:border-stone-700 dark:bg-stone-800">
            <p class="mb-2 text-sm font-semibold dark:text-stone-100">
                {move || match mine.get() {
                    None => "Your bid".to_string(),
                    Some(_) => "Your bid (you can change it until everyone's in)".to_string(),
                }}
            </p>
            <div class="flex flex-wrap gap-2">
                {move || {
                    let dealt = game.with(|g| g.dealt);
                    (0..=dealt)
                        .map(|n| {
                            view! {
                                <button
                                    class=move || {
                                        if mine.get() == Some(n) {
                                            "h-11 w-11 touch-manipulation rounded-lg bg-amber-500 text-lg font-bold text-stone-900 ring-2 ring-amber-300"
                                        } else {
                                            "h-11 w-11 touch-manipulation rounded-lg bg-blue-500 text-lg font-bold text-white hover:bg-blue-600"
                                        }
                                    }
                                    on:click=move |_| send_msg(&ClientMsg::Bid { bid: n })
                                >
                                    {n}
                                </button>
                            }
                        })
                        .collect_view()
                }}
            </div>
        </div>
    }
}

fn send_order(order: Vec<u8>) {
    send_msg(&ClientMsg::Arrange { order });
}

#[component]
fn HandPanel(game: Memo<GameView>, my_turn: Memo<bool>) -> impl IntoView {
    let selected = RwSignal::new(None::<u8>);
    let order = HandOrder::new(send_order);
    let hand = Memo::new(move |_| game.with(|g| g.hand.clone()));
    let playable = move |id: u8| {
        game.with(|g| {
            g.hand
                .iter()
                .find(|c| c.id == id)
                .is_some_and(|c| can_play(&g.hand, &g.trick, c))
        })
    };
    let dimmed = move |id: u8| my_turn.get() && !playable(id);
    let chosen = move || {
        let id = selected.get()?;
        game.with(|g| g.hand.iter().find(|c| c.id == id).copied())
    };
    let play = move |pirate: bool| {
        if let Some(card) = selected.get_untracked() {
            send_msg(&ClientMsg::Play { card, pirate });
        }
    };
    let sort = move |_| {
        let mut cards = hand.get_untracked();
        cards.sort_by_key(|c| c.sort_key());
        order.arrange(cards.iter().map(|c| c.id).collect());
    };

    view! {
        <div class="space-y-3">
            {hand_view(hand, selected, order, card_face, dimmed)}
            {move || {
                if !my_turn.get() {
                    return view! {
                        <p class="text-xs text-stone-500 dark:text-stone-400">
                            "Tap a card to pick it. Drag cards to rearrange your hand."
                        </p>
                    }
                        .into_any();
                }
                match chosen() {
                    Some(c) if c.face == Face::Turncoat => {
                        view! {
                            <div class="grid grid-cols-2 gap-2">
                                <button
                                    class="touch-manipulation rounded-lg bg-red-700 py-3 font-semibold text-white hover:bg-red-600"
                                    on:click=move |_| play(true)
                                >
                                    "Play as ☠️ pirate"
                                </button>
                                <button
                                    class="touch-manipulation rounded-lg bg-sky-600 py-3 font-semibold text-white hover:bg-sky-500"
                                    on:click=move |_| play(false)
                                >
                                    "Play as 🏳️ flag"
                                </button>
                            </div>
                        }
                            .into_any()
                    }
                    Some(c) => {
                        let ok = playable(c.id);
                        view! {
                            <button
                                class="w-full touch-manipulation rounded-lg bg-blue-500 py-3 font-semibold text-white hover:bg-blue-600 disabled:cursor-not-allowed disabled:opacity-40"
                                disabled=!ok
                                on:click=move |_| play(false)
                            >
                                {if ok {
                                    format!("Play {}", c.name())
                                } else {
                                    "You must follow suit".to_string()
                                }}
                            </button>
                        }
                            .into_any()
                    }
                    None => {
                        view! {
                            <p class="text-sm font-semibold text-amber-600 dark:text-amber-300">
                                "Tap the card you want to play."
                            </p>
                        }
                            .into_any()
                    }
                }
            }}
            <button
                class="touch-manipulation rounded bg-stone-600 px-3 py-2 text-sm text-white hover:bg-stone-500"
                on:click=sort
            >
                "Sort hand"
            </button>
        </div>
    }
}

/// A pirate's ability, for the player who unlocked it.
#[component]
fn AbilityPanel(game: Memo<GameView>) -> impl IntoView {
    let picks = RwSignal::new(Vec::<u8>::new());
    let button = "touch-manipulation rounded-lg bg-blue-500 px-4 py-2 font-semibold text-white hover:bg-blue-600 disabled:cursor-not-allowed disabled:opacity-40";
    move || {
        let g = game.get();
        let Stage::Ability {
            player,
            crew,
            cards,
        } = g.stage.clone()
        else {
            return None;
        };
        if player != g.you {
            return None;
        }
        let body = match crew {
            Crew::Navigator => g
                .players
                .iter()
                .map(|p| {
                    let id = p.id;
                    let label = if id == g.you {
                        "Me".to_string()
                    } else {
                        p.nickname.clone()
                    };
                    view! {
                        <button
                            class=button
                            on:click=move |_| send_msg(
                                &ClientMsg::ChooseLeader {
                                    player: id,
                                },
                            )
                        >
                            {label}
                        </button>
                    }
                })
                .collect_view()
                .into_any(),
            Crew::Smuggler => {
                let extra = g.hand.len().saturating_sub(g.tricks_left as usize);
                let hand = g.hand.clone();
                view! {
                    <div class="w-full space-y-2">
                        <p class="text-xs text-stone-500 dark:text-stone-400">
                            {format!("Pick {extra} cards to throw away.")}
                        </p>
                        <div class="flex flex-wrap gap-1" style="--cw: 3rem">
                            {hand
                                .into_iter()
                                .map(|c| {
                                    let id = c.id;
                                    view! {
                                        <button
                                            class=move || {
                                                if picks.with(|p| p.contains(&id)) {
                                                    "rounded-lg opacity-40 ring-2 ring-red-500"
                                                } else {
                                                    "rounded-lg"
                                                }
                                            }
                                            on:click=move |_| {
                                                picks
                                                    .update(|p| {
                                                        if let Some(i) = p.iter().position(|&x| x == id) {
                                                            p.remove(i);
                                                        } else {
                                                            p.push(id);
                                                        }
                                                    })
                                            }
                                        >
                                            {card_face(c)}
                                        </button>
                                    }
                                })
                                .collect_view()}
                        </div>
                        <button
                            class=button
                            disabled=move || picks.with(|p| p.len()) != extra
                            on:click=move |_| {
                                send_msg(
                                    &ClientMsg::Discard {
                                        cards: picks.get_untracked(),
                                    },
                                );
                                picks.set(Vec::new());
                            }
                        >
                            "Discard these"
                        </button>
                    </div>
                }
                    .into_any()
            }
            Crew::Gambler => [0u8, 10, 20]
                .into_iter()
                .map(|points| {
                    let label = if points == 0 {
                        "No bet".to_string()
                    } else {
                        format!("Bet {points}")
                    };
                    view! {
                        <button class=button on:click=move |_| send_msg(&ClientMsg::Bet { points })>
                            {label}
                        </button>
                    }
                })
                .collect_view()
                .into_any(),
            Crew::Seer => view! {
                <div class="w-full space-y-2">
                    <p class="text-xs text-stone-500 dark:text-stone-400">
                        {if cards.is_empty() {
                            "Every card was dealt; there's nothing to see.".to_string()
                        } else {
                            format!("The {} cards nobody holds:", cards.len())
                        }}
                    </p>
                    {mini_cards(cards)}
                    <button class=button on:click=|_| send_msg(&ClientMsg::Done)>
                        "Done"
                    </button>
                </div>
            }
            .into_any(),
            Crew::Brute => {
                let bid = g
                    .players
                    .iter()
                    .find(|p| p.id == g.you)
                    .and_then(|p| p.bid)
                    .unwrap_or_default();
                let at_min = bid == 0;
                let at_max = bid >= g.dealt;
                view! {
                    <button
                        class=button
                        disabled=at_min
                        on:click=|_| send_msg(&ClientMsg::ChangeBid { by: -1 })
                    >
                        {format!("Bid {}", bid.saturating_sub(1))}
                    </button>
                    <button class=button on:click=|_| send_msg(&ClientMsg::ChangeBid { by: 0 })>
                        {format!("Keep {bid}")}
                    </button>
                    <button
                        class=button
                        disabled=at_max
                        on:click=|_| send_msg(&ClientMsg::ChangeBid { by: 1 })
                    >
                        {format!("Bid {}", bid + 1)}
                    </button>
                }
                .into_any()
            }
        };
        Some(view! {
            <div class="rounded-lg border-2 border-amber-400 bg-amber-50 p-3 dark:bg-stone-800">
                <p class="mb-2 text-sm font-semibold text-stone-800 dark:text-stone-100">
                    {format!("☠️ {}: {}", capitalized(crew.name()), crew.ability())}
                </p>
                <div class="flex flex-wrap gap-2">{body}</div>
            </div>
        })
    }
}

fn capitalized(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

fn signed(n: i32) -> String {
    if n > 0 {
        format!("+{n}")
    } else {
        n.to_string()
    }
}

fn results_table(g: &GameView, results: Vec<RoundResult>) -> impl IntoView {
    let show_loot = results.iter().any(|r| r.loot != 0);
    let show_bet = results.iter().any(|r| r.bet != 0);
    let rows = results
        .into_iter()
        .map(|r| {
            let total = g
                .players
                .iter()
                .find(|p| p.id == r.player)
                .map_or(0, |p| p.total);
            let class = if r.made {
                "font-semibold text-green-600 dark:text-green-400"
            } else {
                "font-semibold text-red-500"
            };
            view! {
                <tr class="border-t border-stone-200 dark:border-stone-700">
                    <td class="py-1.5 pr-2 font-semibold">{name_of(g, r.player)}</td>
                    <td class="px-1 text-center">{format!("{}/{}", r.won, r.bid)}</td>
                    <td class="px-1 text-right">{signed(r.base)}</td>
                    <td class="px-1 text-right">{(r.bonus != 0).then(|| signed(r.bonus))}</td>
                    {show_loot
                        .then(|| {
                            view! {
                                <td class="px-1 text-right">
                                    {(r.loot != 0).then(|| signed(r.loot))}
                                </td>
                            }
                        })}
                    {show_bet
                        .then(|| {
                            view! {
                                <td class="px-1 text-right">
                                    {(r.bet != 0).then(|| signed(r.bet))}
                                </td>
                            }
                        })}
                    <td class=format!("px-1 text-right {class}")>{signed(r.total)}</td>
                    <td class="pl-2 text-right font-bold">{total}</td>
                </tr>
            }
        })
        .collect_view();
    view! {
        <div class="overflow-x-auto rounded-lg border border-stone-200 bg-white p-3 dark:border-stone-700 dark:bg-stone-800">
            <table class="game-table w-full text-sm dark:text-stone-100">
                <thead>
                    <tr class="text-xs text-stone-500 uppercase">
                        <th class="pb-1 text-left">"Player"</th>
                        <th class="px-1 text-center">"Won/bid"</th>
                        <th class="px-1 text-right">"Bid"</th>
                        <th class="px-1 text-right">"Bonus"</th>
                        {show_loot.then(|| view! { <th class="px-1 text-right">"Loot"</th> })}
                        {show_bet.then(|| view! { <th class="px-1 text-right">"Bet"</th> })}
                        <th class="px-1 text-right">"Round"</th>
                        <th class="pl-2 text-right">"Total"</th>
                    </tr>
                </thead>
                <tbody>{rows}</tbody>
            </table>
        </div>
    }
}

fn round_over(g: GameView, results: Vec<RoundResult>) -> impl IntoView {
    let host = g.you == g.host;
    let next = g.round + 1;
    view! {
        <div class="space-y-3">
            <h2 class="text-lg font-semibold dark:text-white">
                {format!("Round {} results", g.round)}
            </h2>
            {results_table(&g, results)}
            {if host {
                view! {
                    <button
                        class="w-full rounded bg-blue-500 py-3 font-semibold text-white hover:bg-blue-600"
                        on:click=|_| send_msg(&ClientMsg::NextRound)
                    >
                        {format!("Deal round {next}")}
                    </button>
                }
                    .into_any()
            } else {
                view! {
                    <p class="text-center text-sm text-stone-500">
                        {format!("Waiting for {} to deal...", name_of(&g, g.host))}
                    </p>
                }
                    .into_any()
            }}
        </div>
    }
}

// ── Pages ─────────────────────────────────────────────────────────────────────

#[component]
pub fn PiratesWagerLobby() -> impl IntoView {
    cards::lobby_page(
        GameKind::PiratesWager,
        options_form().into_any(),
        rules_button,
    )
}

#[component]
pub fn PiratesWagerRoom() -> impl IntoView {
    cards::room_page::<GameView>(
        GameKind::PiratesWager,
        |game| view! { <TableView game /> }.into_any(),
        rules_button,
    )
}
