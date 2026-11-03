// Jackson Coxson
// 9 Holes of Golf game server, running on the shared room engine.

use crate::games::cards::server::{broadcast_lobby, error, name, shuffle, standings, Log};
use crate::games::cards::Standing;
use crate::games::number_cards::full_deck;
use crate::games::rooms::{RoomGame, Seats};

use super::logic::{
    grid_score, Action, ActionKind, Card, ClientMsg, GameView, HoleResult, PlayerView, ServerMsg,
    Stage, GRID, MAX_PLAYERS, MIN_PLAYERS, OPENING_FLIPS,
};

struct Hole {
    deck: Vec<Card>,
    discard: Vec<Card>,
    grids: Vec<Vec<Card>>,
    face_up: Vec<[bool; GRID]>,
    current: usize,
    /// The card the current player drew, and whether it came from the
    /// discard pile.
    held: Option<(Card, bool)>,
    /// Who turned up their last card first.
    out: Option<usize>,
}

impl Hole {
    fn all_up(&self, player: usize) -> bool {
        self.face_up[player].iter().all(|&u| u)
    }

    fn up_count(&self, player: usize) -> usize {
        self.face_up[player].iter().filter(|&&u| u).count()
    }

    #[cfg(test)]
    fn card_count(&self) -> usize {
        self.deck.len()
            + self.discard.len()
            + self.grids.iter().map(Vec::len).sum::<usize>()
            + self.held.is_some() as usize
    }
}

enum Phase {
    Lobby,
    Opening,
    Playing,
    HoleOver(Vec<HoleResult>),
    GameOver(Vec<HoleResult>, Vec<Standing>),
}

pub struct Golf {
    phase: Phase,
    /// 1-based.
    hole: u8,
    dealer: usize,
    scores: Vec<Vec<i32>>,
    table: Option<Hole>,
    log: Log,
    last: Option<Action>,
}

impl Default for Golf {
    fn default() -> Self {
        Self::new()
    }
}

impl Golf {
    pub fn new() -> Self {
        Self {
            phase: Phase::Lobby,
            hole: 0,
            dealer: 0,
            scores: Vec::new(),
            table: None,
            log: Log::default(),
            last: None,
        }
    }

    fn act(&mut self, player: usize, kind: ActionKind) {
        let seq = self.last.map_or(1, |a| a.seq + 1);
        self.last = Some(Action {
            seq,
            player: player as u8,
            kind,
        });
    }

    fn broadcast_lobby(&self, seats: &Seats) {
        broadcast_lobby::<GameView>(
            seats,
            "Play as many holes as you like; the host decides when to stop.",
        );
    }

    fn view(&self, seats: &Seats, you: usize) -> GameView {
        let t = self.table.as_ref();
        let stage = match &self.phase {
            Phase::Lobby | Phase::Opening => Stage::Opening,
            Phase::Playing => Stage::Playing,
            Phase::HoleOver(results) => Stage::HoleOver {
                results: results.clone(),
            },
            Phase::GameOver(results, standings) => Stage::GameOver {
                results: results.clone(),
                standings: standings.clone(),
            },
        };
        GameView {
            you: you as u8,
            host: 0,
            hole: self.hole,
            dealer: self.dealer as u8,
            current: t.map_or(0, |t| t.current as u8),
            players: seats
                .iter()
                .enumerate()
                .map(|(i, s)| PlayerView {
                    id: i as u8,
                    nickname: s.nickname.clone(),
                    connected: s.connected,
                    grid: t
                        .map(|t| {
                            (0..GRID)
                                .map(|j| t.face_up[i][j].then_some(t.grids[i][j]))
                                .collect()
                        })
                        .unwrap_or_default(),
                    holes: self.scores.get(i).cloned().unwrap_or_default(),
                    total: self.scores.get(i).map_or(0, |s| s.iter().sum()),
                })
                .collect(),
            discard_top: t.and_then(|t| t.discard.last().copied()),
            deck_size: t.map_or(0, |t| t.deck.len() as u8),
            held: t.and_then(|t| t.held),
            out: t.and_then(|t| t.out.map(|o| o as u8)),
            stage,
            log: self.log.lines(),
            last: self.last,
        }
    }

    fn sync(&self, seats: &Seats) {
        for p in 0..seats.len() {
            seats.send(p, &ServerMsg::Game(Box::new(self.view(seats, p))));
        }
    }

    fn deal(&mut self, seats: &Seats) {
        let n = seats.len();
        let mut deck = full_deck();
        shuffle(&mut deck);
        let grids = (0..n).map(|_| deck.split_off(deck.len() - GRID)).collect();
        let discard = vec![deck.pop().expect("the deck holds every grid")];
        self.table = Some(Hole {
            deck,
            discard,
            grids,
            face_up: vec![[false; GRID]; n],
            current: (self.dealer + 1) % n,
            held: None,
            out: None,
        });
        self.phase = Phase::Opening;
        self.act(self.dealer, ActionKind::Deal);
        self.log
            .note(format!("Hole {}: everyone turns up two cards.", self.hole));
        self.sync(seats);
    }

    fn on_start(&mut self, seats: &Seats, player: usize) {
        if player != 0 || !matches!(self.phase, Phase::Lobby) {
            return;
        }
        if seats.len() < MIN_PLAYERS {
            error(seats, player, "Need at least two players to start.");
            return;
        }
        if seats.len() > MAX_PLAYERS {
            error(seats, player, "Too many players for this game.");
            return;
        }
        self.hole = 1;
        self.dealer = 0;
        self.scores = vec![Vec::new(); seats.len()];
        self.log.clear();
        self.deal(seats);
    }

    fn on_flip(&mut self, seats: &Seats, player: usize, slot: u8) {
        let slot = slot as usize;
        if slot >= GRID {
            return;
        }
        match self.phase {
            Phase::Opening => {
                let Some(t) = self.table.as_mut() else {
                    return;
                };
                if t.up_count(player) >= OPENING_FLIPS || t.face_up[player][slot] {
                    return;
                }
                t.face_up[player][slot] = true;
                let n = t.grids.len();
                let ready = (0..n).all(|p| t.up_count(p) >= OPENING_FLIPS);
                let first = t.current;
                self.act(player, ActionKind::Flipped { slot: slot as u8 });
                if ready {
                    self.phase = Phase::Playing;
                    self.log.note(format!("{} tees off.", name(seats, first)));
                }
                self.sync(seats);
            }
            Phase::Playing => {
                let Some(t) = self.turn(seats, player) else {
                    return;
                };
                if t.held.is_some() {
                    error(seats, player, "Place or discard the card you drew first.");
                    return;
                }
                if t.face_up[player][slot] {
                    error(seats, player, "That card is already face up.");
                    return;
                }
                t.face_up[player][slot] = true;
                let card = t.grids[player][slot];
                self.act(player, ActionKind::Flipped { slot: slot as u8 });
                self.log.note(format!(
                    "{} turned up {}.",
                    name(seats, player),
                    card.label()
                ));
                self.end_turn(seats, player);
            }
            _ => {}
        }
    }

    /// The hole, if it's `player`'s turn.
    fn turn(&mut self, seats: &Seats, player: usize) -> Option<&mut Hole> {
        if !matches!(self.phase, Phase::Playing) {
            return None;
        }
        let t = self.table.as_mut()?;
        if t.current != player {
            error(seats, player, "It's not your turn.");
            return None;
        }
        Some(t)
    }

    fn on_draw(&mut self, seats: &Seats, player: usize, from_discard: bool) {
        let Some(t) = self.turn(seats, player) else {
            return;
        };
        if t.held.is_some() {
            error(seats, player, "You already drew this turn.");
            return;
        }
        let card = if from_discard {
            let Some(card) = t.discard.pop() else {
                error(seats, player, "The discard pile is empty.");
                return;
            };
            card
        } else {
            if t.deck.is_empty() {
                // Shuffle everything but the top discard back into the deck.
                let top = t.discard.pop();
                t.deck.append(&mut t.discard);
                shuffle(&mut t.deck);
                t.discard.extend(top);
            }
            let Some(card) = t.deck.pop() else {
                error(seats, player, "There are no cards left to draw.");
                return;
            };
            card
        };
        t.held = Some((card, from_discard));
        let line = if from_discard {
            format!(
                "{} took {} from the discard pile.",
                name(seats, player),
                card.label()
            )
        } else {
            format!("{} drew from the deck.", name(seats, player))
        };
        self.act(
            player,
            if from_discard {
                ActionKind::TookDiscard
            } else {
                ActionKind::DrewDeck
            },
        );
        self.log.note(line);
        self.sync(seats);
    }

    fn on_swap(&mut self, seats: &Seats, player: usize, slot: u8) {
        let Some(t) = self.turn(seats, player) else {
            return;
        };
        let slot = slot as usize;
        if slot >= GRID {
            return;
        }
        let Some((card, _)) = t.held.take() else {
            error(seats, player, "Draw a card first.");
            return;
        };
        let out = std::mem::replace(&mut t.grids[player][slot], card);
        t.face_up[player][slot] = true;
        t.discard.push(out);
        self.act(
            player,
            ActionKind::Swapped {
                slot: slot as u8,
                out,
            },
        );
        self.log.note(format!(
            "{} put {} in and discarded {}.",
            name(seats, player),
            card.label(),
            out.label()
        ));
        self.end_turn(seats, player);
    }

    fn on_toss(&mut self, seats: &Seats, player: usize) {
        let Some(t) = self.turn(seats, player) else {
            return;
        };
        match t.held {
            Some((_, true)) => {
                error(
                    seats,
                    player,
                    "A card from the discard pile has to be used.",
                );
            }
            Some((card, false)) => {
                t.held = None;
                t.discard.push(card);
                self.act(player, ActionKind::Tossed);
                self.log.note(format!(
                    "{} discarded {}.",
                    name(seats, player),
                    card.label()
                ));
                self.end_turn(seats, player);
            }
            None => error(seats, player, "Draw a card first."),
        }
    }

    fn end_turn(&mut self, seats: &Seats, player: usize) {
        let Some(t) = self.table.as_mut() else {
            return;
        };
        if t.out.is_none() && t.all_up(player) {
            t.out = Some(player);
            self.log.note(format!(
                "{} turned up their last card! Everyone else gets one more turn.",
                name(seats, player)
            ));
        }
        let next = (player + 1) % t.grids.len();
        if t.out == Some(next) {
            self.end_hole(seats);
        } else {
            t.current = next;
        }
        self.sync(seats);
    }

    fn end_hole(&mut self, seats: &Seats) {
        let Some(t) = self.table.as_mut() else {
            return;
        };
        for up in &mut t.face_up {
            *up = [true; GRID];
        }
        let results: Vec<HoleResult> = t
            .grids
            .iter()
            .enumerate()
            .map(|(i, grid)| HoleResult {
                player: i as u8,
                grid: grid.clone(),
                score: grid_score(grid),
                went_out: t.out == Some(i),
            })
            .collect();
        for r in &results {
            self.scores[r.player as usize].push(r.score);
        }
        let best = results.iter().map(|r| r.score).min().unwrap_or_default();
        let leaders: Vec<String> = results
            .iter()
            .filter(|r| r.score == best)
            .map(|r| name(seats, r.player as usize))
            .collect();
        self.log.note(format!(
            "Hole {} done. Best: {} with {best}.",
            self.hole,
            leaders.join(" and ")
        ));
        self.act(self.dealer, ActionKind::Revealed);
        self.phase = Phase::HoleOver(results);
    }

    fn on_next_hole(&mut self, seats: &Seats, player: usize) {
        if player != 0 || !matches!(self.phase, Phase::HoleOver(_)) {
            return;
        }
        self.hole = self.hole.saturating_add(1);
        self.dealer = (self.dealer + 1) % seats.len();
        self.deal(seats);
    }

    fn on_end_game(&mut self, seats: &Seats, player: usize) {
        if player != 0 {
            return;
        }
        let Phase::HoleOver(results) = &self.phase else {
            return;
        };
        let results = results.clone();
        let totals: Vec<i32> = self.scores.iter().map(|s| s.iter().sum()).collect();
        let best = totals.iter().min().copied().unwrap_or_default();
        let winners: Vec<String> = (0..totals.len())
            .filter(|&i| totals[i] == best)
            .map(|i| name(seats, i))
            .collect();
        self.log.note(format!(
            "Game over after {} holes! {} won with {best}.",
            self.hole,
            winners.join(" and ")
        ));
        self.phase = Phase::GameOver(results, standings(&totals, true));
        self.sync(seats);
    }

    fn on_play_again(&mut self, seats: &mut Seats) {
        if !matches!(self.phase, Phase::GameOver(..)) {
            return;
        }
        seats.drop_disconnected();
        self.phase = Phase::Lobby;
        self.table = None;
        self.broadcast_lobby(seats);
    }
}

impl RoomGame for Golf {
    type ClientMsg = ClientMsg;

    fn capacity(&self) -> usize {
        MAX_PLAYERS
    }

    fn in_lobby(&self) -> bool {
        matches!(self.phase, Phase::Lobby)
    }

    fn is_over(&self) -> bool {
        matches!(self.phase, Phase::GameOver(..))
    }

    fn mode(&self) -> String {
        match self.phase {
            Phase::Lobby => "9 Holes of Golf".into(),
            _ => format!("Hole {}", self.hole),
        }
    }

    fn joined(&mut self, seats: &Seats, _player: usize) {
        self.broadcast_lobby(seats);
    }

    fn rejoined(&mut self, seats: &Seats, _player: usize) {
        if self.in_lobby() {
            self.broadcast_lobby(seats);
        } else {
            self.sync(seats);
        }
    }

    fn handle(&mut self, seats: &mut Seats, player: usize, msg: ClientMsg) {
        match msg {
            ClientMsg::SetNickname { name } => {
                if self.in_lobby() && seats.set_nickname(player, &name) {
                    self.broadcast_lobby(seats);
                }
            }
            ClientMsg::StartGame => self.on_start(seats, player),
            ClientMsg::Flip { slot } => self.on_flip(seats, player, slot),
            ClientMsg::Draw { from_discard } => self.on_draw(seats, player, from_discard),
            ClientMsg::Swap { slot } => self.on_swap(seats, player, slot),
            ClientMsg::Toss => self.on_toss(seats, player),
            ClientMsg::NextHole => self.on_next_hole(seats, player),
            ClientMsg::EndGame => self.on_end_game(seats, player),
            ClientMsg::PlayAgain => self.on_play_again(seats),
        }
    }

    fn left(&mut self, seats: &Seats, _player: usize) {
        if self.in_lobby() {
            self.broadcast_lobby(seats);
        } else {
            self.sync(seats);
        }
    }

    fn pruned(&mut self, seats: &Seats) {
        self.broadcast_lobby(seats);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::number_cards::DECK_SIZE;
    use rand::RngExt;

    fn seated(players: usize) -> Seats {
        let mut seats = Seats::default();
        for _ in 0..players {
            seats.add_for_test();
        }
        seats
    }

    fn started(players: usize) -> (Seats, Golf) {
        let mut seats = seated(players);
        let mut g = Golf::new();
        g.handle(&mut seats, 0, ClientMsg::StartGame);
        (seats, g)
    }

    fn open_all(seats: &mut Seats, g: &mut Golf, players: usize) {
        for p in 0..players {
            g.handle(seats, p, ClientMsg::Flip { slot: 0 });
            g.handle(seats, p, ClientMsg::Flip { slot: 4 });
        }
    }

    #[test]
    fn opening_needs_two_flips_each() {
        let (mut seats, mut g) = started(3);
        assert!(matches!(g.phase, Phase::Opening));
        g.handle(&mut seats, 1, ClientMsg::Flip { slot: 3 });
        g.handle(&mut seats, 1, ClientMsg::Flip { slot: 3 });
        g.handle(&mut seats, 1, ClientMsg::Flip { slot: 5 });
        g.handle(&mut seats, 1, ClientMsg::Flip { slot: 6 });
        assert_eq!(
            g.table.as_ref().unwrap().up_count(1),
            2,
            "only two to start"
        );
        open_all(&mut seats, &mut g, 3);
        assert!(matches!(g.phase, Phase::Playing));
        // Player after the dealer goes first.
        assert_eq!(g.table.as_ref().unwrap().current, 1);
    }

    #[test]
    fn discard_must_be_used_but_a_deck_card_can_be_tossed() {
        let (mut seats, mut g) = started(2);
        open_all(&mut seats, &mut g, 2);
        g.handle(&mut seats, 1, ClientMsg::Draw { from_discard: true });
        g.handle(&mut seats, 1, ClientMsg::Toss);
        assert!(
            g.table.as_ref().unwrap().held.is_some(),
            "can't toss a discard"
        );
        g.handle(&mut seats, 1, ClientMsg::Swap { slot: 8 });
        let t = g.table.as_ref().unwrap();
        assert!(t.face_up[1][8]);
        assert_eq!(t.current, 0);

        g.handle(
            &mut seats,
            0,
            ClientMsg::Draw {
                from_discard: false,
            },
        );
        g.handle(&mut seats, 0, ClientMsg::Toss);
        let t = g.table.as_ref().unwrap();
        assert!(t.held.is_none());
        assert_eq!(t.current, 1);
        assert_eq!(t.up_count(0), 2);
    }

    #[test]
    fn last_card_gives_everyone_one_more_turn() {
        let (mut seats, mut g) = started(3);
        open_all(&mut seats, &mut g, 3);
        // Turn up all but one of player 1's cards by hand.
        let t = g.table.as_mut().unwrap();
        t.face_up[1] = [true; GRID];
        t.face_up[1][7] = false;
        g.handle(&mut seats, 1, ClientMsg::Flip { slot: 7 });
        assert_eq!(g.table.as_ref().unwrap().out, Some(1));
        g.handle(&mut seats, 2, ClientMsg::Flip { slot: 1 });
        assert!(matches!(g.phase, Phase::Playing));
        g.handle(&mut seats, 0, ClientMsg::Flip { slot: 1 });
        assert!(matches!(g.phase, Phase::HoleOver(_)));
        assert_eq!(
            g.scores.iter().map(Vec::len).collect::<Vec<_>>(),
            vec![1, 1, 1]
        );
        // Everything is turned up and counted.
        let t = g.table.as_ref().unwrap();
        assert!((0..3).all(|p| t.all_up(p)));
        assert_eq!(g.scores[0][0], grid_score(&t.grids[0]));
    }

    #[test]
    fn host_chooses_when_to_stop() {
        let (mut seats, mut g) = started(2);
        open_all(&mut seats, &mut g, 2);
        let t = g.table.as_mut().unwrap();
        t.face_up[1] = [true; GRID];
        t.face_up[1][2] = false;
        g.handle(&mut seats, 1, ClientMsg::Flip { slot: 2 });
        g.handle(&mut seats, 0, ClientMsg::Flip { slot: 2 });
        assert!(matches!(g.phase, Phase::HoleOver(_)));
        g.handle(&mut seats, 1, ClientMsg::NextHole);
        assert!(matches!(g.phase, Phase::HoleOver(_)), "only the host deals");
        g.handle(&mut seats, 0, ClientMsg::NextHole);
        assert!(matches!(g.phase, Phase::Opening));
        assert_eq!(g.hole, 2);
        // Dealer moves on, so player 0 starts hole two.
        assert_eq!(g.table.as_ref().unwrap().current, 0);
    }

    /// Play many holes with random legal moves and check nothing panics,
    /// stalls or loses cards.
    fn simulate(players: usize, holes: u8) {
        let mut rng = rand::rng();
        let (mut seats, mut g) = started(players);
        for _ in 0..100_000 {
            match &g.phase {
                Phase::Lobby => panic!("game didn't start"),
                Phase::Opening => {
                    let t = g.table.as_ref().unwrap();
                    let p = (0..players).find(|&p| t.up_count(p) < 2).unwrap();
                    let slot = rng.random_range(0..GRID) as u8;
                    g.handle(&mut seats, p, ClientMsg::Flip { slot });
                }
                Phase::Playing => {
                    let t = g.table.as_ref().unwrap();
                    assert_eq!(t.card_count(), DECK_SIZE, "cards went missing");
                    let p = t.current;
                    let down: Vec<u8> = (0..GRID)
                        .filter(|&i| !t.face_up[p][i])
                        .map(|i| i as u8)
                        .collect();
                    let msg = match t.held {
                        Some((_, false)) if rng.random_bool(0.4) => ClientMsg::Toss,
                        Some(_) => ClientMsg::Swap {
                            slot: rng.random_range(0..GRID) as u8,
                        },
                        None if !down.is_empty() && rng.random_bool(0.3) => ClientMsg::Flip {
                            slot: down[rng.random_range(0..down.len())],
                        },
                        None => ClientMsg::Draw {
                            from_discard: rng.random_bool(0.3),
                        },
                    };
                    g.handle(&mut seats, p, msg);
                }
                Phase::HoleOver(results) => {
                    assert_eq!(results.len(), players);
                    if g.hole >= holes {
                        g.handle(&mut seats, 0, ClientMsg::EndGame);
                    } else {
                        g.handle(&mut seats, 0, ClientMsg::NextHole);
                    }
                }
                Phase::GameOver(_, standings) => {
                    assert_eq!(standings.len(), players);
                    assert!(g.scores.iter().all(|s| s.len() == holes as usize));
                    return;
                }
            }
        }
        panic!("game never finished");
    }

    #[test]
    fn random_games_finish() {
        for players in MIN_PLAYERS..=MAX_PLAYERS {
            for _ in 0..10 {
                simulate(players, 9);
            }
        }
    }
}
