// Jackson Coxson
// Chicken Scratch: a dominoes game with double-12 dominoes. Each round
// starts on the next double down, from 12 to 0-0. Play off the middle
// double, and every double you play grows a chicken foot that has to be
// filled before anything else. Lowest score after 13 rounds wins.
//
// The board is the shared pan-and-zoom board (`games::board`); the lobby,
// seats, websocket and hand motion come from the card table.

pub mod layout;
pub mod logic;
#[cfg(feature = "ssr")]
pub mod server;

use leptos::prelude::*;

use crate::games::board::{self, board_view, piece_style, Bounds, Camera, ScreenPose};
use crate::games::cards::{
    self, final_standings, seat_strip, table_talk, GameKind, SeatChip, Standing, TAG_GOLD, TAG_SOFT,
};
use crate::games::dominoes::{back_svg, tile_svg, Tile};
use crate::games::rules::{RulesButton, RulesSection};
use layout::{Layout, Pose};
use logic::{
    round_double, tile_points, ActionKind, Board, ClientMsg, GameView, Options, RoundResult, Stage,
    BONEYARD, ROUNDS,
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

fn layout_of(b: &Board) -> Layout {
    let placed: Vec<(u8, bool)> = b
        .placed
        .iter()
        .map(|p| (p.line, p.tile.is_double()))
        .collect();
    Layout::new(b.sides, &placed)
}

/// Where something on the page is: center, width, height.
fn page_rect(selector: &str) -> Option<(f64, f64, f64, f64)> {
    #[cfg(target_arch = "wasm32")]
    {
        let el = cards::fx::one(selector)?;
        let r = el.get_bounding_client_rect();
        Some((
            r.left() + r.width() / 2.0,
            r.top() + r.height() / 2.0,
            r.width(),
            r.height(),
        ))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = selector;
        None
    }
}

/// A tile tossed from a player's seat: small, and turned a little.
fn from_seat(player: u8, to: ScreenPose) -> ScreenPose {
    match page_rect(&format!("[data-seat=\"{player}\"]")) {
        Some((x, y, _, _)) => ScreenPose {
            x,
            y,
            len: 28.0,
            angle: to.angle - 35.0,
        },
        // No seat to come from: grow into place.
        None => ScreenPose {
            len: to.len * 0.3,
            ..to
        },
    }
}

/// What a lobby room is set to, in a line.
fn rules_line(o: &Options) -> String {
    let mut s = format!("{} sides", o.sides);
    if o.blank_fifty {
        s.push_str(" · 0-0 is 50");
    }
    s
}

// ── Table ─────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum StageKind {
    Playing,
    RoundOver,
    GameOver,
}

/// A spot a tile can go: the line, where it would sit, and whether the
/// selected tile can go there now.
#[derive(Clone, PartialEq)]
struct Slot {
    line: u8,
    pose: Pose,
    live: bool,
}

#[component]
fn TableView(game: Memo<GameView>) -> impl IntoView {
    let camera = Camera::new();
    let selected = RwSignal::new(None::<u8>);
    // How much of the board is drawn; a tile in flight isn't, yet.
    let shown = RwSignal::new(0usize);
    let center_shown = RwSignal::new(false);
    // The piece that just landed and glows: -1 for the middle double.
    let landed = RwSignal::new(None::<i32>);
    // Where on the page your last played tile left from.
    let launch = StoredValue::new(None::<(u8, ScreenPose)>);
    let seen = StoredValue::new(None::<(u8, usize)>);

    let stage = Memo::new(move |_| {
        game.with(|g| match g.stage {
            Stage::Playing => StageKind::Playing,
            Stage::RoundOver { .. } => StageKind::RoundOver,
            Stage::GameOver { .. } => StageKind::GameOver,
        })
    });
    let my_turn = Memo::new(move |_| game.with(|g| g.my_turn()));
    let layout = Memo::new(move |_| game.with(|g| g.board.as_ref().map(layout_of)));
    let round = Memo::new(move |_| game.with(|g| g.round));

    // Animate the board as tiles arrive.
    let play_landing = move |i: usize| {
        let Some(pose) =
            layout.with_untracked(|l| l.as_ref().and_then(|l| l.tiles.get(i).copied()))
        else {
            shown.set(i + 1);
            return;
        };
        let Some(p) =
            game.with_untracked(|g| g.board.as_ref().and_then(|b| b.placed.get(i).copied()))
        else {
            return;
        };
        let you = game.with_untracked(|g| g.you);
        let moved = camera.reveal(Layout::tile_bounds(pose).grow(1.5));
        let to = camera.pose_on_screen(pose.x, pose.y, 2.0, pose.angle);
        let from = match launch.get_value() {
            Some((id, from)) if p.by == you && id == p.tile.id => from,
            _ => from_seat(p.by, to),
        };
        launch.set_value(None);
        board::fly(
            tile_svg(p.near, p.far(), false),
            from,
            to,
            if moved { board::GLIDE_MS - 150 } else { 0 },
            move || {
                shown.update(|s| *s = (*s).max(i + 1));
                landed.set(Some(i as i32));
            },
        );
    };

    Effect::new(move |_| {
        let Some((round, count, deal, opener, center)) = game.with(|g| {
            g.board.as_ref().map(|b| {
                (
                    g.round,
                    b.placed.len(),
                    matches!(g.last.map(|a| a.kind), Some(ActionKind::Deal { .. })),
                    g.opener,
                    b.center,
                )
            })
        }) else {
            return;
        };
        let prev = seen.get_value();
        seen.set_value(Some((round, count)));
        match prev {
            Some((r, c)) if r == round => {
                if count == c + 1 {
                    play_landing(count - 1);
                } else if count != c {
                    shown.set(count);
                }
            }
            _ if deal && count == 0 => {
                // A new round: the double flies from whoever had it to the
                // middle.
                shown.set(0);
                center_shown.set(false);
                landed.set(None);
                selected.set(None);
                cards::after(0, move || {
                    camera.fit(Bounds::around(0.0, 0.0, 4.5), 45.0, false);
                    let to = camera.pose_on_screen(0.0, 0.0, 2.0, 0.0);
                    board::fly(
                        tile_svg(center.a, center.b, false),
                        from_seat(opener, to),
                        to,
                        250,
                        move || {
                            center_shown.set(true);
                            landed.set(Some(-1));
                        },
                    );
                });
            }
            _ => {
                // Joining partway: show it all.
                shown.set(count);
                center_shown.set(true);
                cards::after(0, move || camera.fit_all(false));
            }
        }
    });

    // Step back to see the whole board when a round ends.
    Effect::new(move |prev: Option<StageKind>| {
        let now = stage.get();
        if prev == Some(StageKind::Playing) && now != StageKind::Playing {
            cards::after(board::FLY_MS + board::GLIDE_MS, move || {
                camera.fit_all(true)
            });
        }
        now
    });

    // Drew a tile that fits: pick it up for the player.
    Effect::new(move |_| {
        let drawn = game.with(|g| {
            (g.my_turn() && g.drew)
                .then(|| {
                    g.hand
                        .iter()
                        .find(|t| !g.targets(t).is_empty())
                        .map(|t| t.id)
                })
                .flatten()
        });
        if let Some(id) = drawn {
            selected.set(Some(id));
        }
    });

    let selected_tile = Memo::new(move |_| {
        let id = selected.get()?;
        game.with(|g| g.hand.iter().find(|t| t.id == id).copied())
    });

    // Where tiles can go: the selected tile's spots glow, and empty lines
    // that have to be filled show faintly.
    let slots = Memo::new(move |_| {
        let tile = selected_tile.get();
        game.with(|g| {
            let Some(b) = g.board.as_ref() else {
                return Vec::new();
            };
            layout.with(|l| {
                let Some(l) = l.as_ref() else {
                    return Vec::new();
                };
                let live: Vec<u8> = tile.map(|t| g.targets(&t)).unwrap_or_default();
                let double = tile.is_some_and(|t| t.is_double());
                b.lines
                    .iter()
                    .enumerate()
                    .filter_map(|(i, line)| {
                        let i = i as u8;
                        if live.contains(&i) {
                            Some(Slot {
                                line: i,
                                pose: l.preview(i, double),
                                live: true,
                            })
                        } else if matches!(g.stage, Stage::Playing) && line.len == 0 && !line.closed
                        {
                            Some(Slot {
                                line: i,
                                pose: l.preview(i, false),
                                live: false,
                            })
                        } else {
                            None
                        }
                    })
                    .collect()
            })
        })
    });

    // The board's extent, counting spots waiting to be filled.
    Effect::new(move |_| {
        let Some(tiles) = layout.with(|l| l.as_ref().map(Layout::bounds)) else {
            return;
        };
        let b = slots.with(|s| {
            s.iter()
                .map(|s| Layout::tile_bounds(s.pose))
                .fold(tiles, Bounds::union)
        });
        camera.set_world(b);
    });

    // Picking a tile brings the places it can go into view.
    Effect::new(move |_| {
        let live: Vec<Pose> = slots.with(|s| s.iter().filter(|s| s.live).map(|s| s.pose).collect());
        if let Some(b) = live
            .into_iter()
            .map(Layout::tile_bounds)
            .reduce(Bounds::union)
        {
            camera.reveal(b.grow(0.6));
        }
    });

    let play = move |tile: Tile, line: u8| {
        if camera.was_drag() {
            return;
        }
        let near = game.with_untracked(|g| {
            g.board
                .as_ref()
                .and_then(|b| b.lines.get(line as usize))
                .map(|l| l.open)
        });
        if let (Some(near), Some((x, y, _, h))) =
            (near, page_rect(&format!("[data-card=\"{}\"]", tile.id)))
        {
            // The hand shows `a` on top; the flying tile is drawn with the
            // matching end on the left.
            let angle = if tile.a == near { 90.0 } else { -90.0 };
            launch.set_value(Some((
                tile.id,
                ScreenPose {
                    x,
                    y,
                    len: h,
                    angle,
                },
            )));
        }
        selected.set(None);
        send_msg(&ClientMsg::Play {
            tile: tile.id,
            line,
        });
    };

    let tap_tile = move |tile: Tile| {
        let targets = game.with_untracked(|g| g.targets(&tile));
        if targets.is_empty() {
            return;
        }
        if selected.get_untracked() == Some(tile.id) {
            // A second tap plays it when there's only one place it fits.
            if let [line] = targets[..] {
                play(tile, line);
            } else {
                selected.set(None);
            }
        } else {
            selected.set(Some(tile.id));
        }
    };

    let world = view! {
        {move || {
            center_shown
                .get()
                .then(|| {
                    game.with(|g| g.board.as_ref().map(|b| b.center))
                        .map(|c| {
                            view! {
                                <div
                                    class=move || {
                                        if landed.get() == Some(-1) {
                                            "board-piece tile-land"
                                        } else {
                                            "board-piece"
                                        }
                                    }
                                    style=piece_style(0.0, 0.0, 2.0, 1.0, 0.0)
                                    inner_html=tile_svg(c.a, c.b, false)
                                ></div>
                            }
                        })
                })
        }}
        <For
            each=move || {
                let n = game.with(|g| g.board.as_ref().map_or(0, |b| b.placed.len()));
                let r = round.get();
                (0..shown.get().min(n)).map(|i| (r, i)).collect::<Vec<_>>()
            }
            key=|k| *k
            children=move |(_, i)| {
                let pose = layout
                    .with_untracked(|l| l.as_ref().and_then(|l| l.tiles.get(i).copied()));
                let p = game
                    .with_untracked(|g| g.board.as_ref().and_then(|b| b.placed.get(i).copied()));
                let (Some(pose), Some(p)) = (pose, p) else {
                    return ().into_any();
                };
                view! {
                    <div
                        class=move || {
                            if landed.get() == Some(i as i32) {
                                "board-piece tile-land"
                            } else {
                                "board-piece"
                            }
                        }
                        style=piece_style(pose.x, pose.y, 2.0, 1.0, pose.angle)
                        inner_html=tile_svg(p.near, p.far(), false)
                    ></div>
                }
                    .into_any()
            }
        />
        {move || {
            let tile = selected_tile.get();
            slots
                .get()
                .into_iter()
                .map(|s| {
                    let line = s.line;
                    view! {
                        <button
                            class=if s.live { "board-slot board-slot-live" } else { "board-slot" }
                            style=piece_style(s.pose.x, s.pose.y, 2.0, 1.0, s.pose.angle)
                            disabled=!s.live
                            aria-label="Play here"
                            on:click=move |_| {
                                if let Some(t) = tile {
                                    play(t, line);
                                }
                            }
                        ></button>
                    }
                })
                .collect_view()
        }}
    };

    let can_draw =
        move || game.with(|g| g.my_turn() && !g.drew && g.boneyard > 0 && !g.can_play_any());

    let turn_text = move || {
        game.with(|g| {
            let d = round_double(g.round);
            let filling = g.board.as_ref().map(|b| {
                let middle = b.lines.iter().take(b.sides as usize).any(|l| l.len == 0);
                (b.filling(), middle)
            });
            match &g.stage {
                Stage::Playing if g.my_turn() => {
                    if g.drew {
                        "Play the tile you drew.".to_string()
                    } else if g.can_play_any() {
                        match filling {
                            Some((true, true)) => format!("Play off the double {d} in the middle."),
                            Some((true, false)) => "Fill the chicken foot.".to_string(),
                            _ => "Your turn. Tap a tile, then where it goes.".to_string(),
                        }
                    } else {
                        "Nothing fits. Draw from the boneyard.".to_string()
                    }
                }
                Stage::Playing => {
                    let who = name_of(g, g.current);
                    match filling {
                        Some((true, false)) => {
                            format!("{who}'s turn. The chicken foot needs filling.")
                        }
                        _ => format!("{who}'s turn."),
                    }
                }
                Stage::RoundOver { .. } => format!("Round {} is done.", g.round),
                Stage::GameOver { .. } => "Game over.".to_string(),
            }
        })
    };

    view! {
        <div class="domino-table grid gap-3 lg:grid-cols-[1fr_16rem]">
            <div class="min-w-0 space-y-3">
                <div class="flex flex-wrap items-center gap-2 text-sm">
                    <span class="rounded bg-stone-700 px-3 py-1 font-semibold text-white">
                        {move || {
                            let r = round.get();
                            format!("Round {r} of {ROUNDS} · double {}", round_double(r))
                        }}
                    </span>
                    <span class="text-xs text-stone-500 dark:text-stone-400">
                        {move || game.with(|g| rules_line(&g.options))}
                    </span>
                </div>

                {seat_strip(move || {
                    game.with(|g| {
                        let playing = matches!(g.stage, Stage::Playing);
                        g.players
                            .iter()
                            .map(|p| {
                                let mut tags = vec![
                                    (
                                        format!(
                                            "{} tile{}",
                                            p.tiles,
                                            if p.tiles == 1 { "" } else { "s" },
                                        ),
                                        TAG_SOFT,
                                    ),
                                ];
                                if playing && p.tiles == 1 {
                                    tags.push(("last one!".to_string(), TAG_GOLD));
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

                {board_view(camera, world)}

                <div class="flex items-center gap-3">
                    <button
                        data-pile=BONEYARD
                        class=move || {
                            if can_draw() {
                                "domino-pile card-glow shrink-0 touch-manipulation"
                            } else {
                                "domino-pile shrink-0 cursor-default"
                            }
                        }
                        disabled=move || !can_draw()
                        title="Draw from the boneyard"
                        on:click=|_| send_msg(&ClientMsg::Draw)
                    >
                        <div class="domino-small" inner_html=back_svg(true)></div>
                    </button>
                    <div class="min-w-0 flex-1">
                        <p class="text-xs text-stone-500 dark:text-stone-400">
                            {move || {
                                let n = game.with(|g| g.boneyard);
                                format!("{n} in the boneyard")
                            }}
                        </p>
                        <p class=move || {
                            if my_turn.get() {
                                "text-sm font-semibold text-amber-600 dark:text-amber-300"
                            } else {
                                "text-sm text-stone-700 dark:text-stone-200"
                            }
                        }>{turn_text}</p>
                    </div>
                </div>

                {move || match stage.get() {
                    StageKind::RoundOver => {
                        let g = game.get();
                        let Stage::RoundOver { results, blocked } = g.stage.clone() else {
                            return None;
                        };
                        Some(round_over(g, results, blocked).into_any())
                    }
                    StageKind::GameOver => {
                        let g = game.get();
                        let Stage::GameOver { results, standings } = g.stage.clone() else {
                            return None;
                        };
                        Some(game_over(g, results, standings).into_any())
                    }
                    StageKind::Playing => None,
                }}

                <div class="domino-hand">
                    <For
                        each=move || game.with(|g| g.hand.clone())
                        key=|t| t.id
                        children=move |tile| {
                            let id = tile.id;
                            let fits = move || game.with(|g| !g.targets(&tile).is_empty());
                            view! {
                                <button
                                    data-card=id
                                    class=move || {
                                        let mut c = String::from(
                                            "domino-in-hand touch-manipulation",
                                        );
                                        if selected.get() == Some(id) {
                                            c.push_str(" domino-lift");
                                        }
                                        if my_turn.get() && !fits() {
                                            c.push_str(" domino-dim");
                                        }
                                        c
                                    }
                                    on:click=move |_| tap_tile(tile)
                                    on:contextmenu=|ev| ev.prevent_default()
                                    aria-label=tile.label()
                                >
                                    <div
                                        class="domino-face"
                                        inner_html=tile_svg(tile.a, tile.b, true)
                                    ></div>
                                </button>
                            }
                        }
                    />
                </div>
            </div>

            {table_talk(move || game.with(|g| g.log.clone()))}
        </div>
    }
}

fn left_tiles(tiles: Vec<Tile>) -> impl IntoView {
    view! {
        <div class="flex flex-wrap gap-1">
            {tiles
                .into_iter()
                .map(|t| {
                    view! { <div class="domino-mini" inner_html=tile_svg(t.a, t.b, false)></div> }
                })
                .collect_view()}
        </div>
    }
}

fn results_list(g: &GameView, results: Vec<RoundResult>) -> impl IntoView {
    let players = g.players.clone();
    view! {
        <div class="grid gap-2 sm:grid-cols-2">
            {results
                .into_iter()
                .map(|r| {
                    let p = players.iter().find(|p| p.id == r.player);
                    let name = p.map(|p| p.nickname.clone()).unwrap_or_default();
                    let total = p.map_or(0, |p| p.total);
                    view! {
                        <div class="space-y-1 rounded-lg border border-stone-200 p-2 dark:border-stone-700">
                            <div class="flex items-baseline justify-between text-sm dark:text-stone-100">
                                <span class="font-semibold">
                                    {name}
                                    {r
                                        .went_out
                                        .then(|| {
                                            view! {
                                                <span class="ml-1 text-xs text-amber-500">"went out"</span>
                                            }
                                        })}
                                </span>
                                <span>
                                    {format!("+{}", r.points)}
                                    <span class="ml-2 text-xs text-stone-500">
                                        {format!("total {total}")}
                                    </span>
                                </span>
                            </div>
                            {left_tiles(r.left)}
                        </div>
                    }
                })
                .collect_view()}
        </div>
    }
}

fn round_over(g: GameView, results: Vec<RoundResult>, blocked: bool) -> impl IntoView {
    let host = g.you == g.host;
    let next = g.round + 1;
    view! {
        <div class="row-appear space-y-3 rounded-xl border border-stone-200 bg-white p-4 dark:border-stone-700 dark:bg-stone-800">
            <h2 class="text-lg font-semibold dark:text-white">
                {format!("Round {} results", g.round)}
            </h2>
            {blocked
                .then(|| {
                    view! {
                        <p class="text-sm text-stone-500 dark:text-stone-400">
                            "Nobody could play and the boneyard ran out."
                        </p>
                    }
                })}
            {results_list(&g, results)}
            {if host {
                view! {
                    <button
                        class="w-full touch-manipulation rounded bg-blue-500 py-3 font-semibold text-white hover:bg-blue-600"
                        on:click=|_| send_msg(&ClientMsg::NextRound)
                    >
                        {format!("Play round {next} (double {})", round_double(next))}
                    </button>
                }
                    .into_any()
            } else {
                view! {
                    <p class="text-center text-sm text-stone-500">
                        "Waiting for the host to start the next round..."
                    </p>
                }
                    .into_any()
            }}
        </div>
    }
}

fn game_over(g: GameView, results: Vec<RoundResult>, standings: Vec<Standing>) -> impl IntoView {
    let names = g
        .players
        .iter()
        .map(|p| (p.id, p.nickname.clone()))
        .collect();
    view! {
        <div class="row-appear space-y-4 rounded-xl border border-stone-200 bg-white p-4 dark:border-stone-700 dark:bg-stone-800">
            {final_standings(g.you, standings, names, || send_msg(&ClientMsg::PlayAgain))}
            <h3 class="pt-2 text-sm font-semibold text-stone-500 uppercase">"Last round"</h3>
            {results_list(&g, results)}
        </div>
    }
}

// ── Pages ─────────────────────────────────────────────────────────────────────

fn rules_button() -> AnyView {
    view! {
        <RulesButton title="How to play Chicken Scratch">
            <ChickenScratchRules />
        </RulesButton>
    }
    .into_any()
}

#[component]
fn ChickenScratchRules() -> impl IntoView {
    let b = "font-medium text-white";
    let blank = tile_points(&Tile { id: 0, a: 0, b: 0 }, &Options::default());
    view! {
        <RulesSection heading="Goal">
            <p>
                "Get rid of your dominoes. Whatever's left in your hand at the end of a round \
                counts against you, and the lowest total after 13 rounds wins."
            </p>
        </RulesSection>
        <RulesSection heading="Setup">
            <p>
                "Double-12 dominoes. Everyone draws a hand (15 tiles for up to four players, 12 \
                for five or six, 10 for seven or eight). The rest are the boneyard."
            </p>
            <p>
                "Round 1 starts with the double 12, round 2 with the double 11, and so on down \
                to the double blank. Whoever holds the round's double puts it in the middle. If \
                nobody has it, players take turns drawing until someone does."
            </p>
        </RulesSection>
        <RulesSection heading="Playing">
            <ul class="list-disc space-y-1 pl-5">
                <li>
                    "Play starts with the player after whoever set down the double. First, "
                    <span class=b>"every side of the middle double"</span>
                    " (four, unless the room says otherwise) gets a tile with a matching number."
                </li>
                <li>"After that, play a tile on the open end of any line, matching its number."</li>
                <li>
                    "Play a double and it lies crosswise. It starts a "
                    <span class=b>"chicken foot"</span>
                    ": the next three tiles must go on that double, before anything else."
                </li>
                <li>
                    "If you can play, you have to. If nothing fits, draw one from the boneyard \
                    and play it if it fits. Otherwise your turn passes."
                </li>
            </ul>
        </RulesSection>
        <RulesSection heading="Ending a round">
            <p>
                "The round ends when someone plays their last tile, or when nobody can play and \
                the boneyard is empty. Everyone counts the pips left in their hand."
            </p>
            <p>{format!("The double blank counts {blank} when the room has that rule on.")}</p>
        </RulesSection>
        <RulesSection heading="On a phone">
            <p>
                "Drag the board to look around, pinch to zoom, or use the buttons in its corner. \
                Tap a tile in your hand and the places it fits glow; tap one to play there. Tap \
                the tile again to play it when it fits only one place."
            </p>
        </RulesSection>
    }
}

fn options_form() -> impl IntoView {
    let options = RwSignal::new(Options::default());
    view! {
        <div class="space-y-2 rounded border border-stone-200 p-3 text-sm dark:border-stone-600 dark:text-stone-200">
            <label class="flex items-center justify-between gap-2">
                "Sides on the middle double"
                <select
                    class="rounded border border-stone-300 bg-white px-2 py-1 dark:border-stone-600 dark:bg-stone-700"
                    on:change=move |ev| {
                        let sides = event_target_value(&ev).parse().unwrap_or(4);
                        options.update(|o| o.sides = sides);
                    }
                >
                    {Options::SIDES
                        .into_iter()
                        .map(|n| {
                            view! {
                                <option value=n selected=n == Options::default().sides>
                                    {n.to_string()}
                                </option>
                            }
                        })
                        .collect_view()}
                </select>
            </label>
            <label class="flex items-center gap-2">
                <input
                    type="checkbox"
                    prop:checked=move || options.with(|o| o.blank_fifty)
                    on:change=move |ev| {
                        let on = event_target_checked(&ev);
                        options.update(|o| o.blank_fifty = on);
                    }
                />
                "A leftover double blank counts 50"
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
pub fn ChickenScratchLobby() -> impl IntoView {
    cards::lobby_page(
        GameKind::ChickenScratch,
        options_form().into_any(),
        rules_button,
    )
}

#[component]
pub fn ChickenScratchRoom() -> impl IntoView {
    cards::room_page::<GameView>(
        GameKind::ChickenScratch,
        |game| view! { <TableView game /> }.into_any(),
        rules_button,
    )
}
