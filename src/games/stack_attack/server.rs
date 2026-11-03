// Jackson Coxson
// Stack Attack game server, running on the shared room engine.

use crate::games::cards::server::{broadcast_lobby, error, name, shuffle, standings, Log};
use crate::games::cards::Standing;
use crate::games::number_cards::full_deck;
use crate::games::rooms::{RoomGame, Seats};

use super::logic::{
    fits, partner_of, Action, ActionKind, BuildTop, Card, ClientMsg, GameView, Options, PlayerView,
    ServerMsg, Source, Stage, BUILD_PILES, DISCARD_PILES, HAND_SIZE, MATCH_POINTS, MAX_PLAYERS,
    MIN_PLAYERS, TOP_RANK,
};

struct Table {
    draw: Vec<Card>,
    /// Finished building piles, shuffled back in when the draw pile runs out.
    done: Vec<Card>,
    building: Vec<Vec<Card>>,
    /// Top card last.
    stocks: Vec<Vec<Card>>,
    discards: Vec<Vec<Vec<Card>>>,
    hands: Vec<Vec<Card>>,
    current: usize,
    /// Turns passed in a row with nothing played or discarded.
    passes: usize,
}

impl Table {
    /// Every card on the table, for checking nothing goes missing.
    #[cfg(test)]
    fn card_count(&self) -> usize {
        self.draw.len()
            + self.done.len()
            + self.building.iter().map(Vec::len).sum::<usize>()
            + self.stocks.iter().map(Vec::len).sum::<usize>()
            + self.discards.iter().flatten().map(Vec::len).sum::<usize>()
            + self.hands.iter().map(Vec::len).sum::<usize>()
    }

    /// Fill `player`'s hand back up to five. Returns how many were drawn.
    fn refill(&mut self, player: usize) -> usize {
        let mut drawn = 0;
        while self.hands[player].len() < HAND_SIZE {
            if self.draw.is_empty() {
                if self.done.is_empty() {
                    break;
                }
                self.draw.append(&mut self.done);
                shuffle(&mut self.draw);
            }
            let Some(card) = self.draw.pop() else {
                break;
            };
            self.hands[player].push(card);
            drawn += 1;
        }
        drawn
    }
}

enum Phase {
    Lobby,
    Playing,
    GameOver {
        winners: Vec<u8>,
        points: Option<i32>,
        match_over: bool,
        standings: Vec<Standing>,
    },
}

pub struct StackAttack {
    options: Options,
    phase: Phase,
    /// Games dealt in this match.
    game: u8,
    /// Match points; partners carry the same score.
    scores: Vec<i32>,
    table: Option<Table>,
    log: Log,
    last: Option<Action>,
}

impl StackAttack {
    pub fn new(options: Options) -> Self {
        Self {
            options,
            phase: Phase::Lobby,
            game: 0,
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

    fn partner(&self, player: usize, players: usize) -> Option<usize> {
        self.options.partners.then(|| partner_of(player, players))
    }

    fn view(&self, seats: &Seats, you: usize) -> GameView {
        let t = self.table.as_ref();
        let n = seats.len();
        let stage = match &self.phase {
            Phase::GameOver {
                winners,
                points,
                match_over,
                standings,
            } => Stage::GameOver {
                winners: winners.clone(),
                points: *points,
                match_over: *match_over,
                standings: standings.clone(),
            },
            _ => Stage::Playing,
        };
        GameView {
            you: you as u8,
            host: 0,
            game: self.game,
            current: t.map_or(0, |t| t.current as u8),
            options: self.options,
            partner: self.partner(you, n).map(|p| p as u8),
            hand: t.map(|t| t.hands[you].clone()).unwrap_or_default(),
            draw_count: t.map_or(0, |t| t.draw.len() as u8),
            building: (0..BUILD_PILES)
                .map(|i| {
                    let pile = t.map(|t| &t.building[i])?;
                    pile.last().map(|&card| BuildTop {
                        card,
                        value: pile.len() as u8,
                    })
                })
                .collect(),
            players: seats
                .iter()
                .enumerate()
                .map(|(i, s)| PlayerView {
                    id: i as u8,
                    nickname: s.nickname.clone(),
                    connected: s.connected,
                    stock_left: t.map_or(0, |t| t.stocks[i].len() as u8),
                    stock_top: t.and_then(|t| t.stocks[i].last().copied()),
                    discards: t
                        .map(|t| t.discards[i].clone())
                        .unwrap_or_else(|| vec![Vec::new(); DISCARD_PILES]),
                    hand_size: t.map_or(0, |t| t.hands[i].len() as u8),
                    score: self.scores.get(i).copied().unwrap_or_default(),
                })
                .collect(),
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
        let size = self.options.stock_size(n);
        let stocks = (0..n).map(|_| deck.split_off(deck.len() - size)).collect();
        let first = (self.game as usize).saturating_sub(1) % n;
        self.table = Some(Table {
            draw: deck,
            done: Vec::new(),
            building: vec![Vec::new(); BUILD_PILES],
            stocks,
            discards: vec![vec![Vec::new(); DISCARD_PILES]; n],
            hands: vec![Vec::new(); n],
            current: first,
            passes: 0,
        });
        self.phase = Phase::Playing;
        self.act(first, ActionKind::Deal);
        self.log.note(format!(
            "Game {}: {size} cards in each stock pile.",
            self.game
        ));
        self.sync(seats);
        self.begin_turn(seats, first);
    }

    /// `player` fills their hand to start their turn.
    fn begin_turn(&mut self, seats: &Seats, player: usize) {
        let Some(t) = self.table.as_mut() else {
            return;
        };
        t.current = player;
        if t.refill(player) > 0 {
            self.act(player, ActionKind::Drew);
        }
        self.log.note(format!("{}'s turn.", name(seats, player)));
        self.sync(seats);
    }

    fn on_start(&mut self, seats: &Seats, player: usize) {
        if player != 0 || !matches!(self.phase, Phase::Lobby) {
            return;
        }
        let n = seats.len();
        if n < MIN_PLAYERS {
            error(seats, player, "Need at least two players to start.");
            return;
        }
        if n > MAX_PLAYERS {
            error(seats, player, "Too many players for this game.");
            return;
        }
        if self.options.partners && n != 4 && n != 6 {
            error(seats, player, "Partnerships need four or six players.");
            return;
        }
        self.game = 1;
        self.scores = vec![0; n];
        self.log.clear();
        self.deal(seats);
    }

    /// The table, if it's `player`'s turn.
    fn turn(&mut self, seats: &Seats, player: usize) -> Option<&mut Table> {
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

    fn on_play(&mut self, seats: &Seats, player: usize, card: u8, pile: u8) {
        let n = seats.len();
        let partner = self.partner(player, n);
        let Some(t) = self.turn(seats, player) else {
            return;
        };
        let pile = pile as usize;
        if pile >= BUILD_PILES {
            return;
        }
        // Find the card: in the hand, or on top of a stock or discard pile
        // this player may use.
        let mut found = t.hands[player]
            .iter()
            .find(|c| c.id == card)
            .map(|&c| (c, Source::Hand));
        for s in std::iter::once(player).chain(partner) {
            if found.is_some() {
                break;
            }
            if let Some(&c) = t.stocks[s].last().filter(|c| c.id == card) {
                found = Some((c, Source::Stock(s as u8)));
            } else if let Some(i) = t.discards[s]
                .iter()
                .position(|d| d.last().is_some_and(|c| c.id == card))
            {
                found = Some((*t.discards[s][i].last().unwrap(), Source::Discard(s as u8)));
            }
        }
        let Some((c, from)) = found else {
            error(seats, player, "You can't play that card.");
            return;
        };
        if !fits(&c, t.building[pile].len()) {
            let msg = if t.building[pile].is_empty() {
                "A new pile has to start with a 1 or a Wild."
            } else {
                "That card doesn't come next on that pile."
            };
            error(seats, player, msg);
            return;
        }
        match from {
            Source::Hand => t.hands[player].retain(|x| x.id != card),
            Source::Stock(s) => {
                t.stocks[s as usize].pop();
            }
            Source::Discard(s) => {
                for d in &mut t.discards[s as usize] {
                    if d.last().is_some_and(|x| x.id == card) {
                        d.pop();
                    }
                }
            }
        }
        t.building[pile].push(c);
        t.passes = 0;
        let value = t.building[pile].len();
        let finished = value == TOP_RANK as usize;
        if finished {
            let mut cards = std::mem::take(&mut t.building[pile]);
            t.done.append(&mut cards);
        }
        let hand_empty = t.hands[player].is_empty();
        let stock_empty = |s: usize| t.stocks[s].is_empty();
        let won = match from {
            Source::Stock(_) => stock_empty(player) && partner.is_none_or(stock_empty),
            _ => false,
        };
        let stock_left = match from {
            Source::Stock(s) => Some(t.stocks[s as usize].len()),
            _ => None,
        };

        let who = name(seats, player);
        let what = if c.is_wild() {
            format!("a Wild as {value}")
        } else {
            format!("a {}", c.rank)
        };
        let whence = match from {
            Source::Hand => String::new(),
            Source::Stock(s) if s as usize == player => " from their stock".into(),
            Source::Stock(s) => format!(" from {}'s stock", name(seats, s as usize)),
            Source::Discard(s) if s as usize == player => " from their discards".into(),
            Source::Discard(s) => format!(" from {}'s discards", name(seats, s as usize)),
        };
        self.log.note(format!("{who} played {what}{whence}."));
        if finished {
            self.log.note("A pile reached 12 and was cleared.".into());
        }
        if let (Some(left), false) = (stock_left, won) {
            if left > 0 && left <= 3 {
                self.log
                    .note(format!("Only {left} left in that stock pile!"));
            }
        }
        self.act(
            player,
            ActionKind::Played {
                card: c,
                from,
                pile: pile as u8,
                finished,
            },
        );
        if won {
            self.end_game(seats, player);
            self.sync(seats);
            return;
        }
        self.sync(seats);
        if hand_empty {
            // Played the whole hand: draw five more and keep going.
            if let Some(t) = self.table.as_mut() {
                if t.refill(player) > 0 {
                    self.act(player, ActionKind::Drew);
                    self.log
                        .note(format!("{who} emptied their hand and drew more."));
                    self.sync(seats);
                }
            }
        }
    }

    fn on_discard(&mut self, seats: &Seats, player: usize, card: u8, pile: u8) {
        let Some(t) = self.turn(seats, player) else {
            return;
        };
        let pile = pile as usize;
        if pile >= DISCARD_PILES {
            return;
        }
        let Some(pos) = t.hands[player].iter().position(|c| c.id == card) else {
            error(seats, player, "Discard a card from your hand.");
            return;
        };
        let c = t.hands[player].remove(pos);
        t.discards[player][pile].push(c);
        t.passes = 0;
        let next = (player + 1) % t.hands.len();
        self.log
            .note(format!("{} discarded {}.", name(seats, player), c.label()));
        self.act(player, ActionKind::Discarded { card: c });
        self.sync(seats);
        self.begin_turn(seats, next);
    }

    /// End a turn with an empty hand when there's nothing left to draw.
    fn on_pass(&mut self, seats: &Seats, player: usize) {
        let Some(t) = self.turn(seats, player) else {
            return;
        };
        if !t.hands[player].is_empty() {
            error(seats, player, "End your turn by discarding a card.");
            return;
        }
        let next = (player + 1) % t.hands.len();
        t.passes += 1;
        // Everyone is out of cards with nothing left to draw and no pile
        // finished. The rules don't cover this, so the unfinished building
        // piles become a new draw pile and the game goes on.
        if t.passes >= t.hands.len() {
            t.passes = 0;
            for pile in &mut t.building {
                t.draw.append(pile);
            }
            shuffle(&mut t.draw);
            self.log.note(
                "Nobody could move, so the building piles were shuffled into a new draw pile."
                    .into(),
            );
        }
        self.begin_turn(seats, next);
    }

    fn end_game(&mut self, seats: &Seats, player: usize) {
        let Some(t) = self.table.as_ref() else {
            return;
        };
        let n = seats.len();
        let partner = self.partner(player, n);
        let winners: Vec<usize> = std::iter::once(player).chain(partner).collect();
        let left: Vec<i32> = t.stocks.iter().map(|s| s.len() as i32).collect();
        let names = winners
            .iter()
            .map(|&w| name(seats, w))
            .collect::<Vec<_>>()
            .join(" and ");
        let (points, match_over, standings) = if self.options.scored {
            let opponents: i32 = (0..n)
                .filter(|p| !winners.contains(p))
                .map(|p| left[p])
                .sum();
            let points = 25 + 5 * opponents;
            for &w in &winners {
                self.scores[w] += points;
            }
            let over = self.scores.iter().any(|&s| s >= MATCH_POINTS);
            self.log
                .note(format!("{names} emptied the stock and scored {points}!"));
            if over {
                self.log.note("The match is over!".into());
            }
            (Some(points), over, standings(&self.scores, false))
        } else {
            self.log.note(format!("{names} emptied the stock and won!"));
            (None, true, standings(&left, true))
        };
        self.phase = Phase::GameOver {
            winners: winners.iter().map(|&w| w as u8).collect(),
            points,
            match_over,
            standings,
        };
    }

    fn on_next_game(&mut self, seats: &Seats, player: usize) {
        if player != 0
            || !matches!(
                self.phase,
                Phase::GameOver {
                    match_over: false,
                    ..
                }
            )
        {
            return;
        }
        self.game += 1;
        self.deal(seats);
    }

    fn on_arrange(&mut self, seats: &Seats, player: usize, order: Vec<u8>) {
        let Some(t) = self.table.as_mut() else {
            return;
        };
        let hand = &mut t.hands[player];
        let mut sorted = order.clone();
        sorted.sort_unstable();
        let mut ids: Vec<u8> = hand.iter().map(|c| c.id).collect();
        ids.sort_unstable();
        if sorted != ids {
            return;
        }
        hand.sort_by_key(|c| order.iter().position(|&id| id == c.id));
        seats.send(player, &ServerMsg::Game(Box::new(self.view(seats, player))));
    }

    fn on_play_again(&mut self, seats: &mut Seats) {
        if !matches!(
            self.phase,
            Phase::GameOver {
                match_over: true,
                ..
            }
        ) {
            return;
        }
        seats.drop_disconnected();
        self.phase = Phase::Lobby;
        self.table = None;
        self.broadcast_lobby(seats);
    }
}

impl RoomGame for StackAttack {
    type ClientMsg = ClientMsg;

    fn capacity(&self) -> usize {
        MAX_PLAYERS
    }

    fn in_lobby(&self) -> bool {
        matches!(self.phase, Phase::Lobby)
    }

    fn is_over(&self) -> bool {
        matches!(
            self.phase,
            Phase::GameOver {
                match_over: true,
                ..
            }
        )
    }

    fn mode(&self) -> String {
        let mut mode = String::from(if self.options.short {
            "Short game"
        } else {
            "Stack Attack"
        });
        if self.options.partners {
            mode.push_str(" · partners");
        }
        if self.options.scored {
            mode.push_str(" · to 500");
        }
        mode
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
            ClientMsg::Play { card, pile } => self.on_play(seats, player, card, pile),
            ClientMsg::Discard { card, pile } => self.on_discard(seats, player, card, pile),
            ClientMsg::Pass => self.on_pass(seats, player),
            ClientMsg::Arrange { order } => self.on_arrange(seats, player, order),
            ClientMsg::NextGame => self.on_next_game(seats, player),
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

    /// Every legal play for the current player: (card id, building pile).
    fn plays(g: &StackAttack, players: usize) -> Vec<(u8, u8)> {
        let t = g.table.as_ref().unwrap();
        let p = t.current;
        let mut cards: Vec<Card> = t.hands[p].clone();
        for s in std::iter::once(p).chain(g.partner(p, players)) {
            cards.extend(t.stocks[s].last());
            cards.extend(t.discards[s].iter().filter_map(|d| d.last()));
        }
        let mut out = Vec::new();
        for c in cards {
            for (i, b) in t.building.iter().enumerate() {
                if fits(&c, b.len()) {
                    out.push((c.id, i as u8));
                }
            }
        }
        out
    }

    /// Play whole games with random legal moves, preferring the stock, and
    /// check nothing panics, stalls or loses cards.
    fn simulate(players: usize, options: Options) -> u8 {
        let mut rng = rand::rng();
        let mut seats = seated(players);
        let mut g = StackAttack::new(options);
        g.handle(&mut seats, 0, ClientMsg::StartGame);
        for _ in 0..200_000 {
            match &g.phase {
                Phase::Lobby => panic!("game didn't start"),
                Phase::GameOver { match_over, .. } => {
                    if *match_over {
                        return g.game;
                    }
                    g.handle(&mut seats, 0, ClientMsg::NextGame);
                }
                Phase::Playing => {
                    let t = g.table.as_ref().unwrap();
                    assert_eq!(t.card_count(), DECK_SIZE, "cards went missing");
                    assert!(t.building.iter().all(|b| b.len() < 12));
                    let p = t.current;
                    let moves = plays(&g, players);
                    let stock_top = t.stocks[p].last().map(|c| c.id);
                    let stock_move = moves.iter().find(|m| Some(m.0) == stock_top);
                    if let Some(&(card, pile)) = stock_move {
                        g.handle(&mut seats, p, ClientMsg::Play { card, pile });
                    } else if !moves.is_empty() && rng.random_bool(0.8) {
                        let (card, pile) = moves[rng.random_range(0..moves.len())];
                        g.handle(&mut seats, p, ClientMsg::Play { card, pile });
                    } else if let Some(c) = t.hands[p].first() {
                        let card = c.id;
                        let pile = rng.random_range(0..DISCARD_PILES) as u8;
                        g.handle(&mut seats, p, ClientMsg::Discard { card, pile });
                    } else {
                        g.handle(&mut seats, p, ClientMsg::Pass);
                    }
                }
            }
        }
        let t = g.table.as_ref().unwrap();
        panic!(
            "game never finished: draw {} done {} building {:?} stocks {:?} hands {:?} discards {:?}",
            t.draw.len(),
            t.done.len(),
            t.building.iter().map(Vec::len).collect::<Vec<_>>(),
            t.stocks.iter().map(|s| (s.len(), s.last().map(|c| c.rank))).collect::<Vec<_>>(),
            t.hands.iter().map(|h| h.iter().map(|c| c.rank).collect::<Vec<_>>()).collect::<Vec<_>>(),
            t.discards.iter().map(|d| d.iter().map(Vec::len).collect::<Vec<_>>()).collect::<Vec<_>>(),
        );
    }

    #[test]
    fn random_games_finish() {
        for players in MIN_PLAYERS..=MAX_PLAYERS {
            for _ in 0..10 {
                simulate(players, Options::default());
            }
        }
    }

    #[test]
    fn random_short_and_partner_games_finish() {
        for _ in 0..20 {
            simulate(
                4,
                Options {
                    short: true,
                    partners: true,
                    scored: false,
                },
            );
            simulate(
                6,
                Options {
                    short: true,
                    partners: true,
                    scored: false,
                },
            );
        }
    }

    #[test]
    fn scored_match_reaches_500() {
        for _ in 0..3 {
            let games = simulate(
                3,
                Options {
                    short: true,
                    partners: false,
                    scored: true,
                },
            );
            assert!(games >= 2, "a 10-card short game can't score 500 at once");
        }
    }

    #[test]
    fn rejects_bad_plays() {
        let mut seats = seated(2);
        let mut g = StackAttack::new(Options::default());
        g.handle(&mut seats, 0, ClientMsg::StartGame);
        let t = g.table.as_mut().unwrap();
        assert_eq!(t.hands[0].len(), 5);
        assert_eq!(t.hands[1].len(), 0, "player two draws on their own turn");
        assert_eq!(t.stocks[0].len(), 30);
        t.hands[0] = vec![
            Card { id: 200, rank: 2 },
            Card { id: 201, rank: 1 },
            Card { id: 202, rank: 0 },
            Card { id: 203, rank: 5 },
            Card { id: 204, rank: 9 },
        ];
        // Not their turn.
        g.handle(&mut seats, 1, ClientMsg::Play { card: 201, pile: 0 });
        assert!(g.table.as_ref().unwrap().building[0].is_empty());
        // A 2 can't start a pile.
        g.handle(&mut seats, 0, ClientMsg::Play { card: 200, pile: 0 });
        assert!(g.table.as_ref().unwrap().building[0].is_empty());
        g.handle(&mut seats, 0, ClientMsg::Play { card: 201, pile: 0 });
        g.handle(&mut seats, 0, ClientMsg::Play { card: 200, pile: 0 });
        g.handle(&mut seats, 0, ClientMsg::Play { card: 202, pile: 0 });
        assert_eq!(g.table.as_ref().unwrap().building[0].len(), 3);
        // Wild counted as 3, so a 5 doesn't fit yet.
        g.handle(&mut seats, 0, ClientMsg::Play { card: 203, pile: 0 });
        assert_eq!(g.table.as_ref().unwrap().building[0].len(), 3);
        // Discarding ends the turn; player two draws five.
        g.handle(&mut seats, 0, ClientMsg::Discard { card: 204, pile: 2 });
        let t = g.table.as_ref().unwrap();
        assert_eq!(t.current, 1);
        assert_eq!(t.hands[1].len(), 5);
        assert_eq!(t.discards[0][2].len(), 1);
    }

    #[test]
    fn emptying_the_hand_draws_five_more() {
        let mut seats = seated(2);
        let mut g = StackAttack::new(Options::default());
        g.handle(&mut seats, 0, ClientMsg::StartGame);
        let t = g.table.as_mut().unwrap();
        // Swap the hand for 1–5 so it can all be played.
        let mut hand = Vec::new();
        for rank in 1..=5 {
            let pos = t.draw.iter().position(|c| c.rank == rank).unwrap();
            hand.push(t.draw.remove(pos));
        }
        t.draw.append(&mut t.hands[0]);
        t.hands[0] = hand.clone();
        for c in hand {
            g.handle(
                &mut seats,
                0,
                ClientMsg::Play {
                    card: c.id,
                    pile: 1,
                },
            );
        }
        let t = g.table.as_ref().unwrap();
        assert_eq!(t.building[1].len(), 5);
        assert_eq!(t.hands[0].len(), 5);
        assert_eq!(t.current, 0);
    }

    #[test]
    fn a_finished_pile_is_recycled() {
        let mut seats = seated(2);
        let mut g = StackAttack::new(Options::default());
        g.handle(&mut seats, 0, ClientMsg::StartGame);
        let t = g.table.as_mut().unwrap();
        // Cards from outside the deck, so the setup can't run short.
        t.building[0] = (200..211).map(|id| Card { id, rank: 0 }).collect();
        let twelve = Card { id: 220, rank: 12 };
        t.hands[0][0] = twelve;
        g.handle(
            &mut seats,
            0,
            ClientMsg::Play {
                card: twelve.id,
                pile: 0,
            },
        );
        let t = g.table.as_mut().unwrap();
        assert!(t.building[0].is_empty());
        assert_eq!(t.done.len(), 12);
        // Once the draw pile is gone the finished cards come back.
        t.draw.clear();
        t.hands[1].clear();
        assert_eq!(t.refill(1), 5);
        assert_eq!(t.done.len(), 0);
        assert_eq!(t.draw.len(), 7);
    }
}
