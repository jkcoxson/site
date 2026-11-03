// Jackson Coxson
// Age of Monuments game server, running on the shared room engine.

use std::collections::VecDeque;

use crate::games::cards::seating::{left_of, pass_hands, right_of};
use crate::games::cards::server::{
    broadcast_lobby, error, name, shuffle, standings_with_tiebreak, Log, Picks,
};
use crate::games::cards::Standing;
use crate::games::rooms::{RoomGame, Seats};

use super::logic::{
    age_cards, age_name, check, coins_when_built, def, league_cards, pass_dir, score, war, Act,
    Breakdown, Choice, City, ClientMsg, Done, GameView, Gift, Move, MoveKind, Options, Pay,
    PlayerView, Reveal, ServerMsg, Side, Sides, Stage, AGES, BOARDS, HAND, MAX_PLAYERS,
    MIN_PLAYERS, SELL_COINS, TURNS,
};

enum Phase {
    Lobby,
    Choosing,
    Seventh(usize),
    Salvage(usize),
    Over(Vec<Breakdown>, Vec<Standing>),
}

/// Something one player does between turns, while everyone waits.
#[derive(Debug, Clone, Copy)]
enum Task {
    Seventh(usize),
    Salvage(usize),
}

/// The latest turn, as each player saw it, for card motion.
struct TurnRecord {
    played: Vec<Option<(u8, Act)>>,
    passed: Vec<Vec<u8>>,
    /// Hands went to the next player, or into the discards at an age's end.
    passed_on: bool,
    dealt: bool,
}

enum Last {
    Ready,
    Turn(TurnRecord),
    Other,
}

pub struct Monuments {
    options: Options,
    phase: Phase,
    age: u8,
    turn: u8,
    players: usize,
    cities: Vec<City>,
    hands: Vec<Vec<u8>>,
    picks: Picks<Choice>,
    reveals: Vec<Option<Reveal>>,
    discard: Vec<u8>,
    /// Cards tucked under each monument.
    under: Vec<Vec<u8>>,
    tasks: VecDeque<Task>,
    /// The age's last turn has been played; only tasks are left.
    age_done: bool,
    log: Log,
    seq: u32,
    last: Option<Last>,
}

impl Monuments {
    pub fn new(options: Options) -> Self {
        Self {
            options,
            phase: Phase::Lobby,
            age: 0,
            turn: 0,
            players: 0,
            cities: Vec::new(),
            hands: Vec::new(),
            picks: Picks::new(0),
            reveals: Vec::new(),
            discard: Vec::new(),
            under: Vec::new(),
            tasks: VecDeque::new(),
            age_done: false,
            log: Log::default(),
            seq: 0,
            last: None,
        }
    }

    fn broadcast_lobby(&self, seats: &Seats) {
        broadcast_lobby::<GameView>(seats, &self.options.describe());
    }

    fn bump(&mut self, last: Last) {
        self.seq += 1;
        self.last = Some(last);
    }

    fn view(&self, seats: &Seats, you: usize) -> GameView {
        let n = self.players.max(1);
        let stage = match &self.phase {
            Phase::Lobby | Phase::Choosing => Stage::Choosing,
            Phase::Seventh(p) => Stage::Seventh { player: *p as u8 },
            Phase::Salvage(p) => Stage::Salvage { player: *p as u8 },
            Phase::Over(scores, standings) => Stage::Over {
                scores: scores.clone(),
                standings: standings.clone(),
            },
        };
        let last = self.last.as_ref().map(|l| Move {
            seq: self.seq,
            kind: match l {
                Last::Ready => MoveKind::Ready,
                Last::Other => MoveKind::Other,
                Last::Turn(t) => {
                    let dir = pass_dir(self.age);
                    MoveKind::Turn {
                        played: t.played.get(you).copied().flatten(),
                        passed: t.passed.get(you).cloned().unwrap_or_default(),
                        to: t.passed_on.then(|| dir.to(n, you) as u8),
                        from: t.passed_on.then(|| dir.from(n, you) as u8),
                        dealt: t.dealt,
                    }
                }
            },
        });
        GameView {
            you: you as u8,
            host: 0,
            age: self.age,
            turn: self.turn,
            options: self.options.clone(),
            players: seats
                .iter()
                .enumerate()
                .take(self.cities.len())
                .map(|(i, s)| PlayerView {
                    id: i as u8,
                    nickname: s.nickname.clone(),
                    connected: s.connected,
                    city: self.cities[i].clone(),
                    ready: self.picks.is_in(i),
                    last: self.reveals.get(i).copied().flatten(),
                })
                .collect(),
            hand: self.hands.get(you).cloned().unwrap_or_default(),
            choice: self.picks.get(you).copied(),
            stage,
            discard: match self.phase {
                Phase::Salvage(p) if p == you => {
                    let mut d = self.discard.clone();
                    d.sort_by_key(|&id| (def(id).color as u8, def(id).name));
                    d
                }
                _ => Vec::new(),
            },
            log: self.log.lines(),
            last,
        }
    }

    fn sync(&self, seats: &Seats) {
        for p in 0..seats.len() {
            seats.send(p, &ServerMsg::Game(Box::new(self.view(seats, p))));
        }
    }

    fn on_start(&mut self, seats: &Seats, player: usize) {
        if player != 0 || !matches!(self.phase, Phase::Lobby) {
            return;
        }
        if seats.len() < MIN_PLAYERS {
            error(
                seats,
                player,
                "Age of Monuments needs at least three players.",
            );
            return;
        }
        if seats.len() > MAX_PLAYERS {
            error(seats, player, "Too many players for this game.");
            return;
        }
        use rand::RngExt;
        let mut rng = rand::rng();
        self.players = seats.len();
        let mut boards: Vec<u8> = (0..BOARDS.len() as u8).collect();
        shuffle(&mut boards);
        self.cities = boards
            .into_iter()
            .take(self.players)
            .map(|b| {
                let side = match self.options.sides {
                    Sides::A => Side::A,
                    Sides::B => Side::B,
                    Sides::Random => {
                        if rng.random_bool(0.5) {
                            Side::A
                        } else {
                            Side::B
                        }
                    }
                };
                City::new(b, side)
            })
            .collect();
        self.under = vec![Vec::new(); self.players];
        self.discard.clear();
        self.tasks.clear();
        self.log.clear();
        self.age = 0;
        self.deal_age(seats);
        self.sync(seats);
    }

    /// Start the next age: fresh hands of seven.
    fn deal_age(&mut self, seats: &Seats) {
        self.age += 1;
        self.turn = 1;
        self.age_done = false;
        let n = self.players;
        let mut cards = age_cards(self.age, n);
        if self.age == AGES {
            let mut leagues = league_cards();
            shuffle(&mut leagues);
            cards.extend(leagues.into_iter().take(n + 2));
        }
        shuffle(&mut cards);
        self.hands = (0..n)
            .map(|_| cards.split_off(cards.len() - HAND))
            .collect();
        for c in &mut self.cities {
            c.power_used = false;
        }
        self.picks = Picks::new(n);
        self.reveals = vec![None; n];
        self.phase = Phase::Choosing;
        match &mut self.last {
            Some(Last::Turn(t)) if self.age > 1 => {
                t.dealt = true;
                self.seq += 1;
            }
            _ => self.bump(Last::Turn(TurnRecord {
                played: vec![None; n],
                passed: vec![Vec::new(); n],
                passed_on: false,
                dealt: true,
            })),
        }
        let dir = pass_dir(self.age);
        self.log.note(format!(
            "Age {} begins. Hands pass {} to the {}.",
            age_name(self.age),
            dir.arrow(),
            dir.word()
        ));
        let _ = seats;
    }

    fn on_choose(&mut self, seats: &Seats, player: usize, choice: Choice) {
        let seventh = matches!(self.phase, Phase::Seventh(p) if p == player);
        if !matches!(self.phase, Phase::Choosing) && !seventh {
            return;
        }
        if !self.hands[player].contains(&choice.card) {
            error(seats, player, "That card isn't in your hand.");
            return;
        }
        if let Err(why) = check(&self.cities, player, &choice) {
            error(seats, player, why);
            return;
        }
        if seventh {
            self.hands[player].retain(|&c| c != choice.card);
            let reveal = self.place(player, choice);
            self.cash_in(player, choice);
            self.reveals[player] = Some(reveal);
            self.log.note(format!(
                "{} played their last card too: {}.",
                name(seats, player),
                done_text(reveal.done)
            ));
            self.bump(Last::Turn(TurnRecord {
                played: (0..self.players)
                    .map(|p| (p == player).then_some((choice.card, choice.act)))
                    .collect(),
                passed: vec![Vec::new(); self.players],
                passed_on: false,
                dealt: false,
            }));
            self.run_tasks(seats);
            self.sync(seats);
            return;
        }
        self.picks.set(player, choice);
        if self.picks.all_in() {
            self.play_turn(seats);
        } else {
            self.bump(Last::Ready);
        }
        self.sync(seats);
    }

    fn on_unchoose(&mut self, seats: &Seats, player: usize) {
        if !matches!(self.phase, Phase::Choosing) || !self.picks.is_in(player) {
            return;
        }
        self.picks.clear(player);
        self.bump(Last::Ready);
        self.sync(seats);
    }

    /// Pay for and put down one player's card. Coins from the card itself
    /// come later, in `cash_in`, once every city has its new card.
    fn place(&mut self, p: usize, c: Choice) -> Reveal {
        let n = self.players;
        let pay = c.pay;
        self.cities[p].coins -= pay.total();
        self.cities[left_of(n, p)].coins += pay.left as i32;
        self.cities[right_of(n, p)].coins += pay.right as i32;
        let done = match c.act {
            Act::Build => {
                self.cities[p].built.push(c.card);
                Done::Built(c.card)
            }
            Act::Power => {
                self.cities[p].built.push(c.card);
                self.cities[p].power_used = true;
                Done::Free(c.card)
            }
            Act::Stage => {
                self.cities[p].stages += 1;
                self.under[p].push(c.card);
                Done::Stage(self.cities[p].stages)
            }
            Act::Sell => {
                self.cities[p].coins += SELL_COINS;
                self.discard.push(c.card);
                Done::Sold
            }
        };
        Reveal { done, pay }
    }

    /// Coins a new card or stage pays, and abilities it starts.
    fn cash_in(&mut self, p: usize, c: Choice) {
        match c.act {
            Act::Build | Act::Power => {
                let coins = coins_when_built(&self.cities, p, def(c.card));
                self.cities[p].coins += coins;
            }
            Act::Stage => {
                let city = &self.cities[p];
                let gifts = city.stage_defs()[city.stages as usize - 1].gifts;
                for g in gifts {
                    match g {
                        Gift::Coins(k) => self.cities[p].coins += *k as i32,
                        Gift::Salvage => self.tasks.push_back(Task::Salvage(p)),
                        _ => {}
                    }
                }
            }
            Act::Sell => {}
        }
    }

    /// Everyone has picked: play the cards at once and pass the hands.
    fn play_turn(&mut self, seats: &Seats) {
        let Some(picks) = self.picks.take_all() else {
            return;
        };
        let n = self.players;
        for (p, c) in picks.iter().enumerate() {
            self.hands[p].retain(|&id| id != c.card);
        }
        for (p, &c) in picks.iter().enumerate() {
            self.reveals[p] = Some(self.place(p, c));
        }
        for (p, &c) in picks.iter().enumerate() {
            self.cash_in(p, c);
        }
        let summary: Vec<String> = (0..n)
            .map(|p| {
                format!(
                    "{} {}",
                    name(seats, p),
                    self.reveals[p]
                        .map(|r| done_text(r.done))
                        .unwrap_or_default()
                )
            })
            .collect();
        self.log
            .note(format!("Turn {}: {}.", self.turn, summary.join(", ")));

        let passed = self.hands.clone();
        let last_turn = self.turn >= TURNS;
        if last_turn {
            // The last card is discarded, unless the monument lets its owner play it.
            for p in 0..n {
                if self.cities[p].has_gift(Gift::Seventh) && !self.hands[p].is_empty() {
                    self.tasks.push_front(Task::Seventh(p));
                } else {
                    self.discard.append(&mut self.hands[p]);
                }
            }
            self.age_done = true;
        } else {
            pass_hands(&mut self.hands, pass_dir(self.age));
            self.turn += 1;
        }
        self.bump(Last::Turn(TurnRecord {
            played: picks.iter().map(|c| Some((c.card, c.act))).collect(),
            passed: (0..n)
                .map(|p| {
                    if last_turn && self.hands[p].len() == passed[p].len() {
                        // Kept for the seventh card.
                        Vec::new()
                    } else {
                        passed[p].clone()
                    }
                })
                .collect(),
            passed_on: !last_turn,
            dealt: false,
        }));
        self.run_tasks(seats);
    }

    /// Start the next waiting task, or move on to the next turn or age.
    fn run_tasks(&mut self, seats: &Seats) {
        while let Some(task) = self.tasks.pop_front() {
            match task {
                Task::Seventh(p) if !self.hands[p].is_empty() => {
                    self.phase = Phase::Seventh(p);
                    self.log.note(format!(
                        "{}'s {} lets them play their last card.",
                        name(seats, p),
                        self.cities[p].board().monument
                    ));
                    return;
                }
                Task::Salvage(p) if self.salvageable(p) => {
                    self.phase = Phase::Salvage(p);
                    self.log.note(format!(
                        "{} is searching the discards to build one for free.",
                        name(seats, p)
                    ));
                    return;
                }
                Task::Salvage(p) => self.log.note(format!(
                    "{} found nothing to build in the discards.",
                    name(seats, p)
                )),
                Task::Seventh(_) => {}
            }
        }
        if self.age_done {
            self.end_age(seats);
        } else {
            self.phase = Phase::Choosing;
        }
    }

    fn salvageable(&self, p: usize) -> bool {
        self.discard
            .iter()
            .any(|&id| !self.cities[p].owns(def(id).name))
    }

    fn on_salvage(&mut self, seats: &Seats, player: usize, card: Option<u8>) {
        if !matches!(self.phase, Phase::Salvage(p) if p == player) {
            return;
        }
        match card {
            Some(id) => {
                let Some(pos) = self.discard.iter().position(|&c| c == id) else {
                    error(seats, player, "That card isn't in the discards.");
                    return;
                };
                if self.cities[player].owns(def(id).name) {
                    error(seats, player, "You already have one.");
                    return;
                }
                self.discard.remove(pos);
                self.cities[player].built.push(id);
                let coins = coins_when_built(&self.cities, player, def(id));
                self.cities[player].coins += coins;
                self.reveals[player] = Some(Reveal {
                    done: Done::Salvaged(id),
                    pay: Pay::default(),
                });
                self.log.note(format!(
                    "{} built {} from the discards.",
                    name(seats, player),
                    def(id).name
                ));
            }
            None => self
                .log
                .note(format!("{} passed on the discards.", name(seats, player))),
        }
        self.bump(Last::Other);
        self.run_tasks(seats);
        self.sync(seats);
    }

    fn end_age(&mut self, seats: &Seats) {
        let tokens = war(&self.cities, self.age);
        let mut lines = Vec::new();
        for (p, won) in tokens.into_iter().enumerate() {
            if !won.is_empty() {
                let shown: Vec<String> = won
                    .iter()
                    .map(|&t| {
                        if t > 0 {
                            format!("+{t}")
                        } else {
                            t.to_string()
                        }
                    })
                    .collect();
                lines.push(format!("{} {}", name(seats, p), shown.join(" ")));
            }
            self.cities[p].tokens.extend(won);
        }
        self.log.note(if lines.is_empty() {
            format!("War at the end of Age {}: all quiet.", age_name(self.age))
        } else {
            format!(
                "War at the end of Age {}: {}.",
                age_name(self.age),
                lines.join(", ")
            )
        });
        if self.age >= AGES {
            self.finish(seats);
        } else {
            self.deal_age(seats);
        }
    }

    fn finish(&mut self, seats: &Seats) {
        let scores: Vec<Breakdown> = (0..self.players).map(|p| score(&self.cities, p)).collect();
        let keys: Vec<(i32, i32)> = scores
            .iter()
            .zip(&self.cities)
            .map(|(s, c)| (s.total, c.coins))
            .collect();
        let standings = standings_with_tiebreak(&keys);
        let winners: Vec<String> = standings
            .iter()
            .filter(|s| s.place == 1)
            .map(|s| name(seats, s.player as usize))
            .collect();
        let best = scores.iter().map(|s| s.total).max().unwrap_or_default();
        self.log.note(format!(
            "Game over! {} {} with {best} points.",
            winners.join(" and "),
            if winners.len() > 1 {
                "share the win"
            } else {
                "wins"
            }
        ));
        self.phase = Phase::Over(scores, standings);
        self.bump(Last::Other);
    }

    fn on_play_again(&mut self, seats: &mut Seats) {
        if !matches!(self.phase, Phase::Over(..)) {
            return;
        }
        seats.drop_disconnected();
        self.phase = Phase::Lobby;
        self.cities.clear();
        self.hands.clear();
        self.players = 0;
        self.last = None;
        self.broadcast_lobby(seats);
    }
}

/// What a player did, in a few words.
fn done_text(done: Done) -> String {
    match done {
        Done::Built(id) => format!("built {}", def(id).name),
        Done::Free(id) => format!("built {} for free", def(id).name),
        Done::Stage(k) => format!("raised stage {k} of their monument"),
        Done::Sold => "sold a card".into(),
        Done::Salvaged(id) => format!("built {} from the discards", def(id).name),
    }
}

impl RoomGame for Monuments {
    type ClientMsg = ClientMsg;

    fn capacity(&self) -> usize {
        MAX_PLAYERS
    }

    fn in_lobby(&self) -> bool {
        matches!(self.phase, Phase::Lobby)
    }

    fn is_over(&self) -> bool {
        matches!(self.phase, Phase::Over(..))
    }

    fn mode(&self) -> String {
        match self.phase {
            Phase::Lobby => self.options.describe(),
            Phase::Over(..) => "Game over".into(),
            _ => format!("Age {}, turn {}", age_name(self.age), self.turn),
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
        if !self.in_lobby() && player >= self.players {
            return;
        }
        match msg {
            ClientMsg::SetNickname { name } => {
                if self.in_lobby() && seats.set_nickname(player, &name) {
                    self.broadcast_lobby(seats);
                }
            }
            ClientMsg::StartGame => self.on_start(seats, player),
            ClientMsg::Choose { choice } => self.on_choose(seats, player, choice),
            ClientMsg::Unchoose => self.on_unchoose(seats, player),
            ClientMsg::Salvage { card } => self.on_salvage(seats, player, card),
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
    use crate::games::monuments::logic::{build_cost, deck, stage_cost, Cost};
    use rand::RngExt;

    /// A random legal move for player `p` with `hand`.
    fn random_choice(cities: &[City], p: usize, hand: &[u8]) -> Choice {
        let mut rng = rand::rng();
        let card = hand[rng.random_range(0..hand.len())];
        let mut options = vec![Choice {
            card,
            act: Act::Sell,
            pay: Pay::default(),
        }];
        match build_cost(cities, p, card) {
            Cost::Free | Cost::Chain(_) => options.push(Choice {
                card,
                act: Act::Build,
                pay: Pay::default(),
            }),
            Cost::Pay(ways) => options.extend(ways.into_iter().map(|pay| Choice {
                card,
                act: Act::Build,
                pay,
            })),
            Cost::Cannot(_) => {}
        }
        match stage_cost(cities, p) {
            Some(Cost::Free) => options.push(Choice {
                card,
                act: Act::Stage,
                pay: Pay::default(),
            }),
            Some(Cost::Pay(ways)) => options.extend(ways.into_iter().map(|pay| Choice {
                card,
                act: Act::Stage,
                pay,
            })),
            _ => {}
        }
        if super::super::logic::can_use_power(&cities[p], card) {
            options.push(Choice {
                card,
                act: Act::Power,
                pay: Pay::default(),
            });
        }
        // Lean toward building so cities fill up and abilities get used.
        if options.len() > 1 && rng.random_bool(0.85) {
            options.remove(0);
        }
        options[rng.random_range(0..options.len())]
    }

    /// Every card dealt so far is somewhere: a hand, a city, a monument or
    /// the discards.
    fn cards_accounted(g: &Monuments) {
        let mut all: Vec<u8> = g.hands.iter().flatten().copied().collect();
        all.extend(g.cities.iter().flat_map(|c| c.built.iter().copied()));
        all.extend(g.under.iter().flatten());
        all.extend(&g.discard);
        let count = all.len();
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), count, "a card is in two places");
        let n = g.players;
        let dealt = (g.age as usize) * n * HAND;
        assert_eq!(count, dealt, "cards went missing");
    }

    fn simulate(players: usize, sides: Sides) -> Vec<Breakdown> {
        let mut rng = rand::rng();
        let mut seats = Seats::default();
        for _ in 0..players {
            seats.add_for_test();
        }
        let mut g = Monuments::new(Options { sides });
        g.handle(&mut seats, 0, ClientMsg::StartGame);
        for _ in 0..10_000 {
            cards_accounted(&g);
            for c in &g.cities {
                assert!(c.coins >= 0, "negative coins");
            }
            match &g.phase {
                Phase::Lobby => panic!("game didn't start"),
                Phase::Choosing => {
                    assert_eq!(g.hands[0].len(), HAND + 1 - g.turn as usize);
                    for p in 0..players {
                        let c = random_choice(&g.cities, p, &g.hands[p]);
                        g.handle(&mut seats, p, ClientMsg::Choose { choice: c });
                        // Take it back sometimes and choose again.
                        if rng.random_bool(0.1)
                            && matches!(g.phase, Phase::Choosing)
                            && g.picks.is_in(p)
                        {
                            g.handle(&mut seats, p, ClientMsg::Unchoose);
                            g.handle(&mut seats, p, ClientMsg::Choose { choice: c });
                        }
                    }
                }
                Phase::Seventh(p) => {
                    let p = *p;
                    assert_eq!(g.hands[p].len(), 1);
                    let c = random_choice(&g.cities, p, &g.hands[p]);
                    g.handle(&mut seats, p, ClientMsg::Choose { choice: c });
                    assert!(!matches!(g.phase, Phase::Seventh(q) if q == p));
                }
                Phase::Salvage(p) => {
                    let p = *p;
                    let pick = g
                        .discard
                        .iter()
                        .copied()
                        .find(|&id| !g.cities[p].owns(def(id).name));
                    let card = if rng.random_bool(0.9) { pick } else { None };
                    g.handle(&mut seats, p, ClientMsg::Salvage { card });
                    assert!(!matches!(g.phase, Phase::Salvage(q) if q == p));
                }
                Phase::Over(scores, standings) => {
                    assert_eq!(scores.len(), players);
                    assert_eq!(standings.len(), players);
                    assert!(standings.iter().any(|s| s.place == 1));
                    assert!(g.hands.iter().all(|h| h.is_empty()));
                    assert!(g.cities.iter().all(|c| c.tokens.len() <= 6));
                    return scores.clone();
                }
            }
        }
        panic!("game never finished");
    }

    #[test]
    fn random_games() {
        for _ in 0..15 {
            for players in MIN_PLAYERS..=MAX_PLAYERS {
                for sides in [Sides::A, Sides::B, Sides::Random] {
                    simulate(players, sides);
                }
            }
        }
    }

    #[test]
    fn views_hide_hands() {
        let mut seats = Seats::default();
        for _ in 0..3 {
            seats.add_for_test();
        }
        let mut g = Monuments::new(Options::default());
        g.handle(&mut seats, 0, ClientMsg::StartGame);
        let v = g.view(&seats, 1);
        assert_eq!(v.hand, g.hands[1]);
        assert!(v.discard.is_empty());
        let card = g.hands[0][0];
        g.handle(
            &mut seats,
            0,
            ClientMsg::Choose {
                choice: Choice {
                    card,
                    act: Act::Sell,
                    pay: Pay::default(),
                },
            },
        );
        let v = g.view(&seats, 1);
        assert!(v.players[0].ready);
        assert!(v.choice.is_none());
        assert_eq!(g.view(&seats, 0).choice.map(|c| c.card), Some(card));
    }

    #[test]
    fn rejects_bad_picks() {
        let mut seats = Seats::default();
        for _ in 0..3 {
            seats.add_for_test();
        }
        let mut g = Monuments::new(Options::default());
        g.handle(&mut seats, 0, ClientMsg::StartGame);
        let not_mine = g.hands[1][0];
        g.handle(
            &mut seats,
            0,
            ClientMsg::Choose {
                choice: Choice {
                    card: not_mine,
                    act: Act::Sell,
                    pay: Pay::default(),
                },
            },
        );
        assert!(!g.picks.is_in(0));
        // Paying nothing for something that costs resources is refused.
        let costly = (0..deck().len() as u8)
            .find(|&id| def(id).cost.len() >= 3)
            .unwrap();
        g.hands[0][0] = costly;
        g.handle(
            &mut seats,
            0,
            ClientMsg::Choose {
                choice: Choice {
                    card: costly,
                    act: Act::Build,
                    pay: Pay::default(),
                },
            },
        );
        assert!(!g.picks.is_in(0));
    }
}
