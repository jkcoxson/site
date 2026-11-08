// Jackson Coxson
// Chicken Scratch game server, running on the shared room engine.

use crate::games::cards::server::{broadcast_lobby, error, name, shuffle, standings, Log};
use crate::games::cards::Standing;
use crate::games::dominoes::full_set;
use crate::games::rooms::{RoomGame, Seats};

use super::logic::{
    hand_size, round_double, tile_points, Action, ActionKind, Board, ClientMsg, GameView, Options,
    PlayerView, RoundResult, ServerMsg, Stage, Tile, MAX_PLAYERS, MIN_PLAYERS, ROUNDS, TOP,
};

struct Round {
    boneyard: Vec<Tile>,
    hands: Vec<Vec<Tile>>,
    board: Board,
    current: usize,
    drew: bool,
    opener: usize,
}

impl Round {
    fn can_play(&self, player: usize) -> bool {
        self.hands[player].iter().any(|t| self.board.can_play(t))
    }

    /// Nobody can play and there's nothing left to draw.
    fn blocked(&self) -> bool {
        self.boneyard.is_empty() && (0..self.hands.len()).all(|p| !self.can_play(p))
    }

    #[cfg(test)]
    fn tile_count(&self) -> usize {
        self.boneyard.len()
            + self.hands.iter().map(Vec::len).sum::<usize>()
            + self.board.placed.len()
            + 1
    }
}

enum Phase {
    Lobby,
    Playing,
    RoundOver(Vec<RoundResult>, bool),
    GameOver(Vec<RoundResult>, Vec<Standing>),
}

pub struct ChickenScratch {
    options: Options,
    phase: Phase,
    /// 1-based.
    round: u8,
    scores: Vec<Vec<i32>>,
    table: Option<Round>,
    log: Log,
    last: Option<Action>,
}

impl ChickenScratch {
    pub fn new(options: Options) -> Self {
        Self {
            options: options.checked(),
            phase: Phase::Lobby,
            round: 0,
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
        broadcast_lobby::<GameView>(seats, &self.options.describe());
    }

    fn view(&self, seats: &Seats, you: usize) -> GameView {
        let t = self.table.as_ref();
        let stage = match &self.phase {
            Phase::Lobby | Phase::Playing => Stage::Playing,
            Phase::RoundOver(results, blocked) => Stage::RoundOver {
                results: results.clone(),
                blocked: *blocked,
            },
            Phase::GameOver(results, standings) => Stage::GameOver {
                results: results.clone(),
                standings: standings.clone(),
            },
        };
        GameView {
            you: you as u8,
            host: 0,
            round: self.round,
            options: self.options,
            current: t.map_or(0, |t| t.current as u8),
            players: seats
                .iter()
                .enumerate()
                .map(|(i, s)| PlayerView {
                    id: i as u8,
                    nickname: s.nickname.clone(),
                    connected: s.connected,
                    tiles: t.map_or(0, |t| t.hands.get(i).map_or(0, |h| h.len() as u8)),
                    rounds: self.scores.get(i).cloned().unwrap_or_default(),
                    total: self.scores.get(i).map_or(0, |s| s.iter().sum()),
                })
                .collect(),
            hand: t
                .and_then(|t| t.hands.get(you).cloned())
                .unwrap_or_default(),
            board: t.map(|t| t.board.clone()),
            boneyard: t.map_or(0, |t| t.boneyard.len() as u8),
            drew: t.is_some_and(|t| t.drew),
            opener: t.map_or(0, |t| t.opener as u8),
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
        let double = round_double(self.round);
        let mut boneyard = full_set(TOP);
        shuffle(&mut boneyard);
        let size = hand_size(n);
        let mut hands: Vec<Vec<Tile>> = (0..n)
            .map(|_| boneyard.split_off(boneyard.len() - size))
            .collect();
        let has_double = |h: &Vec<Tile>| h.iter().any(|t| t.a == double && t.b == double);
        // Nobody has it: take turns drawing until someone does, starting
        // with a different player each round.
        let mut drawn = 0u8;
        let mut drawer = (self.round as usize).saturating_sub(1) % n;
        let opener = loop {
            if let Some(p) = (0..n).find(|&p| has_double(&hands[p])) {
                break p;
            }
            let Some(tile) = boneyard.pop() else {
                // Can't happen: every tile is in a hand or the boneyard.
                break 0;
            };
            hands[drawer].push(tile);
            drawn += 1;
            drawer = (drawer + 1) % n;
        };
        let pos = hands[opener]
            .iter()
            .position(|t| t.a == double && t.b == double)
            .expect("the opener holds the double");
        let center = hands[opener].remove(pos);
        for h in &mut hands {
            h.sort_by_key(|t| (t.a, t.b));
        }
        self.table = Some(Round {
            boneyard,
            hands,
            board: Board::new(center, self.options.sides),
            current: (opener + 1) % n,
            drew: false,
            opener,
        });
        self.phase = Phase::Playing;
        self.act(opener, ActionKind::Deal { drawn });
        if drawn > 0 {
            self.log.note(format!(
                "Round {}: nobody had the double {double}. After {drawn} draws, {} found it.",
                self.round,
                name(seats, opener)
            ));
        } else {
            self.log.note(format!(
                "Round {}: {} sets down the double {double}.",
                self.round,
                name(seats, opener)
            ));
        }
        self.advance_past_stuck(seats);
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
        self.round = 1;
        self.scores = vec![Vec::new(); seats.len()];
        self.log.clear();
        self.deal(seats);
    }

    /// The round, if it's `player`'s turn.
    fn turn(&mut self, seats: &Seats, player: usize) -> Option<&mut Round> {
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

    fn on_play(&mut self, seats: &Seats, player: usize, tile: u8, line: u8) {
        let Some(t) = self.turn(seats, player) else {
            return;
        };
        let Some(pos) = t.hands[player].iter().position(|x| x.id == tile) else {
            return;
        };
        let piece = t.hands[player][pos];
        if !t.board.targets(&piece).contains(&line) {
            error(seats, player, "That tile can't go there.");
            return;
        }
        t.hands[player].remove(pos);
        t.board.place(piece, line, player as u8);
        t.drew = false;
        let out = t.hands[player].is_empty();
        self.act(player, ActionKind::Played);
        let who = name(seats, player);
        if piece.is_double() {
            self.log.note(format!(
                "{who} played {}. Fill its chicken foot!",
                piece.label()
            ));
        } else {
            self.log.note(format!("{who} played {}.", piece.label()));
        }
        if out {
            self.log.note(format!("{who} is out of tiles!"));
            self.end_round(seats, Some(player));
        } else {
            self.next_turn(seats);
        }
        self.sync(seats);
    }

    fn on_draw(&mut self, seats: &Seats, player: usize) {
        let Some(t) = self.turn(seats, player) else {
            return;
        };
        if t.drew {
            error(seats, player, "You already drew this turn.");
            return;
        }
        if t.can_play(player) {
            error(
                seats,
                player,
                "You have a tile that fits, so you have to play it.",
            );
            return;
        }
        let Some(tile) = t.boneyard.pop() else {
            return;
        };
        t.hands[player].push(tile);
        t.hands[player].sort_by_key(|t| (t.a, t.b));
        let fits = t.board.can_play(&tile);
        t.drew = fits;
        self.act(player, ActionKind::Drew { passed: !fits });
        let who = name(seats, player);
        if fits {
            self.log.note(format!("{who} drew a tile that fits."));
        } else {
            self.log.note(format!("{who} drew and passed."));
            self.next_turn(seats);
        }
        self.sync(seats);
    }

    /// Move to the next player, passing anyone stuck with nothing to draw,
    /// or end the round if nobody can play.
    fn next_turn(&mut self, seats: &Seats) {
        let Some(t) = self.table.as_mut() else {
            return;
        };
        t.drew = false;
        t.current = (t.current + 1) % t.hands.len();
        self.advance_past_stuck(seats);
    }

    fn advance_past_stuck(&mut self, seats: &Seats) {
        let Some(t) = self.table.as_mut() else {
            return;
        };
        if t.blocked() {
            self.log
                .note("Nobody can play and the boneyard is empty.".to_string());
            self.end_round(seats, None);
            return;
        }
        let mut passed = Vec::new();
        while t.boneyard.is_empty() && !t.can_play(t.current) {
            passed.push(name(seats, t.current));
            t.current = (t.current + 1) % t.hands.len();
        }
        if !passed.is_empty() {
            self.log.note(format!("{} can't play.", passed.join(", ")));
        }
    }

    fn end_round(&mut self, seats: &Seats, out: Option<usize>) {
        let Some(t) = self.table.as_ref() else {
            return;
        };
        let current = t.current;
        let results: Vec<RoundResult> = t
            .hands
            .iter()
            .enumerate()
            .map(|(i, hand)| RoundResult {
                player: i as u8,
                left: hand.clone(),
                points: hand.iter().map(|x| tile_points(x, &self.options)).sum(),
                went_out: out == Some(i),
            })
            .collect();
        for r in &results {
            self.scores[r.player as usize].push(r.points);
        }
        self.act(current, ActionKind::RoundOver);
        if self.round >= ROUNDS {
            let totals: Vec<i32> = self.scores.iter().map(|s| s.iter().sum()).collect();
            let best = totals.iter().min().copied().unwrap_or_default();
            let winners: Vec<String> = (0..totals.len())
                .filter(|&i| totals[i] == best)
                .map(|i| name(seats, i))
                .collect();
            self.log.note(format!(
                "Game over! {} won with {best}.",
                winners.join(" and ")
            ));
            self.phase = Phase::GameOver(results, standings(&totals, true));
        } else {
            self.phase = Phase::RoundOver(results, out.is_none());
        }
    }

    fn on_next_round(&mut self, seats: &Seats, player: usize) {
        if player != 0 || !matches!(self.phase, Phase::RoundOver(..)) {
            return;
        }
        self.round += 1;
        self.deal(seats);
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

impl RoomGame for ChickenScratch {
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
            Phase::Lobby => format!("{} sides", self.options.sides),
            _ => format!("Round {} of {ROUNDS}", self.round),
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
            ClientMsg::Play { tile, line } => self.on_play(seats, player, tile, line),
            ClientMsg::Draw => self.on_draw(seats, player),
            ClientMsg::NextRound => self.on_next_round(seats, player),
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
    use crate::games::dominoes::set_size;
    use rand::RngExt;

    fn started(players: usize, options: Options) -> (Seats, ChickenScratch) {
        let mut seats = Seats::default();
        for _ in 0..players {
            seats.add_for_test();
        }
        let mut g = ChickenScratch::new(options);
        g.handle(&mut seats, 0, ClientMsg::StartGame);
        (seats, g)
    }

    #[test]
    fn opener_holds_the_double_and_the_next_player_starts() {
        let (_, g) = started(4, Options::default());
        let t = g.table.as_ref().unwrap();
        assert_eq!((t.board.center.a, t.board.center.b), (12, 12));
        assert_eq!(t.current, (t.opener + 1) % 4);
        assert!(t
            .hands
            .iter()
            .all(|h| !h.iter().any(|x| x.a == 12 && x.b == 12)));
        assert_eq!(t.tile_count(), set_size(TOP));
    }

    #[test]
    fn must_play_before_drawing() {
        let (mut seats, mut g) = started(2, Options::default());
        let t = g.table.as_mut().unwrap();
        let p = t.current;
        // Give the current player a tile that fits.
        let fits = Tile {
            id: 200,
            a: 12,
            b: 3,
        };
        t.hands[p].push(fits);
        let before = t.hands[p].len();
        g.handle(&mut seats, p, ClientMsg::Draw);
        assert_eq!(g.table.as_ref().unwrap().hands[p].len(), before);
        g.handle(&mut seats, p, ClientMsg::Play { tile: 200, line: 2 });
        let t = g.table.as_ref().unwrap();
        assert_eq!(t.board.placed.len(), 1);
        assert_ne!(t.current, p);
    }

    #[test]
    fn going_out_scores_the_others() {
        let (mut seats, mut g) = started(2, Options::default());
        let t = g.table.as_mut().unwrap();
        let p = t.current;
        let other = 1 - p;
        t.hands[p] = vec![Tile {
            id: 200,
            a: 12,
            b: 3,
        }];
        t.hands[other] = vec![
            Tile {
                id: 201,
                a: 0,
                b: 0,
            },
            Tile {
                id: 202,
                a: 4,
                b: 5,
            },
        ];
        g.handle(&mut seats, p, ClientMsg::Play { tile: 200, line: 0 });
        let Phase::RoundOver(results, blocked) = &g.phase else {
            panic!("round should be over");
        };
        assert!(!blocked);
        assert_eq!(results[p].points, 0);
        assert!(results[p].went_out);
        assert_eq!(results[other].points, 59);
        assert_eq!(g.scores[other], vec![59]);
    }

    /// Play whole games with random legal moves and check nothing panics,
    /// stalls or loses tiles.
    fn simulate(players: usize, options: Options) {
        let mut rng = rand::rng();
        let (mut seats, mut g) = started(players, options);
        for _ in 0..200_000 {
            match &g.phase {
                Phase::Lobby => panic!("game didn't start"),
                Phase::Playing => {
                    let t = g.table.as_ref().unwrap();
                    assert_eq!(t.tile_count(), set_size(TOP), "tiles went missing");
                    let p = t.current;
                    let moves: Vec<(u8, u8)> = t.hands[p]
                        .iter()
                        .flat_map(|x| t.board.targets(x).into_iter().map(|l| (x.id, l)))
                        .collect();
                    if moves.is_empty() {
                        assert!(
                            !t.boneyard.is_empty(),
                            "stuck player should have been passed"
                        );
                        g.handle(&mut seats, p, ClientMsg::Draw);
                    } else {
                        let (tile, line) = moves[rng.random_range(0..moves.len())];
                        g.handle(&mut seats, p, ClientMsg::Play { tile, line });
                    }
                }
                Phase::RoundOver(results, _) => {
                    assert_eq!(results.len(), players);
                    g.handle(&mut seats, 0, ClientMsg::NextRound);
                }
                Phase::GameOver(_, standings) => {
                    assert_eq!(standings.len(), players);
                    assert!(g.scores.iter().all(|s| s.len() == ROUNDS as usize));
                    return;
                }
            }
        }
        panic!("game never finished");
    }

    #[test]
    fn random_games_finish() {
        for players in MIN_PLAYERS..=MAX_PLAYERS {
            for sides in Options::SIDES {
                simulate(
                    players,
                    Options {
                        sides,
                        blank_fifty: true,
                    },
                );
            }
        }
    }
}
