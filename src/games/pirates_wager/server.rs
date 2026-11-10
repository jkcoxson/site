// Jackson Coxson
// Pirate's Wager game server, running on the shared room engine.

use crate::games::cards::server::{
    arrange_hand, broadcast_lobby, error, name, shuffle, standings, Log,
};
use crate::games::cards::Standing;
use crate::games::rooms::{RoomGame, Seats};

use super::logic::{
    can_play, full_deck, hand_size, resolve, round_points, Action, ActionKind, Card, ClientMsg,
    Crew, Face, GameView, Options, Played, PlayerView, RoundResult, ServerMsg, Stage, TrickView,
    GHOST, MAX_PLAYERS, MIN_PLAYERS,
};

struct Round {
    dealt: u8,
    /// One hand per player, then the ghost's if it plays.
    hands: Vec<Vec<Card>>,
    undealt: Vec<Card>,
    bids: Vec<Option<u8>>,
    won: Vec<u8>,
    /// Capture bonuses, paid only if the bid is made.
    bonus: Vec<i32>,
    bets: Vec<i32>,
    /// Loot alliances: (who played the loot, who won that trick).
    alliances: Vec<(usize, usize)>,
    trick: Vec<Played>,
    /// Hand index of whoever leads the current trick.
    leader: usize,
    /// Who led the round's first trick.
    starter: usize,
    tricks_left: u8,
    last_trick: Option<TrickView>,
}

enum Phase {
    Lobby,
    Bidding,
    Playing,
    /// A pirate's ability waits on a player.
    Ability(usize, Crew),
    RoundOver(Vec<RoundResult>),
    GameOver(Vec<RoundResult>, Vec<Standing>),
}

pub struct PiratesWager {
    options: Options,
    phase: Phase,
    /// Cards per hand in each round. A tie at the end adds a round.
    schedule: Vec<u8>,
    /// 1-based.
    round: u8,
    players: usize,
    ghost: bool,
    scores: Vec<Vec<i32>>,
    hand: Option<Round>,
    log: Log,
    last: Option<Action>,
}

impl PiratesWager {
    pub fn new(options: Options) -> Self {
        Self {
            schedule: options.length.rounds(),
            options,
            phase: Phase::Lobby,
            round: 0,
            players: 0,
            ghost: false,
            scores: Vec::new(),
            hand: None,
            log: Log::default(),
            last: None,
        }
    }

    fn broadcast_lobby(&self, seats: &Seats) {
        broadcast_lobby::<GameView>(seats, &self.options.describe());
    }

    /// A hand index as a player id on the wire.
    fn wire(&self, hand: usize) -> u8 {
        if self.ghost && hand == self.players {
            GHOST
        } else {
            hand as u8
        }
    }

    /// A player id from the wire as a hand index.
    fn hand_of(&self, id: u8) -> usize {
        if id == GHOST {
            self.players
        } else {
            id as usize
        }
    }

    fn who(&self, seats: &Seats, hand: usize) -> String {
        if self.ghost && hand == self.players {
            "The ghost".into()
        } else {
            name(seats, hand)
        }
    }

    fn act(&mut self, player: u8, kind: ActionKind) {
        let seq = self.last.map_or(1, |a| a.seq + 1);
        self.last = Some(Action { seq, player, kind });
    }

    /// The order hands play in the current trick. With the ghost, it always
    /// plays second; when it leads, the round's first player goes next.
    fn order(&self, round: &Round) -> Vec<usize> {
        let n = self.players;
        if self.ghost {
            let other = |p: usize| 1 - p;
            if round.leader == n {
                vec![n, round.starter, other(round.starter)]
            } else {
                vec![round.leader, n, other(round.leader)]
            }
        } else {
            (0..n).map(|k| (round.leader + k) % n).collect()
        }
    }

    fn current(&self) -> Option<usize> {
        if !matches!(self.phase, Phase::Playing) {
            return None;
        }
        let round = self.hand.as_ref()?;
        self.order(round).get(round.trick.len()).copied()
    }

    fn view(&self, seats: &Seats, you: usize) -> GameView {
        let round = self.hand.as_ref();
        let revealed = !matches!(self.phase, Phase::Bidding);
        let stage = match &self.phase {
            Phase::Lobby | Phase::Bidding => Stage::Bidding,
            Phase::Playing => Stage::Playing,
            Phase::Ability(player, crew) => Stage::Ability {
                player: *player as u8,
                crew: *crew,
                cards: match (crew, round) {
                    (Crew::Seer, Some(r)) if *player == you => {
                        let mut cards = r.undealt.clone();
                        cards.sort_by_key(|c| c.sort_key());
                        cards
                    }
                    _ => Vec::new(),
                },
            },
            Phase::RoundOver(results) => Stage::RoundOver {
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
            round: self.round,
            rounds: self.schedule.len() as u8,
            dealt: round.map_or(0, |r| r.dealt),
            options: self.options.clone(),
            ghost: self.ghost,
            players: seats
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    let bid = round.and_then(|r| r.bids.get(i).copied().flatten());
                    PlayerView {
                        id: i as u8,
                        nickname: s.nickname.clone(),
                        connected: s.connected,
                        total: self.scores.get(i).map_or(0, |s| s.iter().sum()),
                        bid: if revealed || i == you { bid } else { None },
                        has_bid: bid.is_some(),
                        won: round.map_or(0, |r| r.won[i]),
                        bet: round.map_or(0, |r| r.bets[i]),
                    }
                })
                .collect(),
            hand: round
                .and_then(|r| r.hands.get(you).cloned())
                .unwrap_or_default(),
            trick: round.map(|r| r.trick.clone()).unwrap_or_default(),
            last_trick: round.and_then(|r| r.last_trick.clone()),
            current: self.current().map(|h| self.wire(h)),
            starter: round.map_or(0, |r| r.starter as u8),
            tricks_left: round.map_or(0, |r| r.tricks_left),
            stage,
            log: self.log.lines(),
            last: self.last,
        }
    }

    /// Send every player their own view of the table.
    fn sync(&self, seats: &Seats) {
        for p in 0..seats.len() {
            seats.send(p, &ServerMsg::Game(Box::new(self.view(seats, p))));
        }
    }

    fn deal(&mut self, seats: &Seats, starter: usize) {
        let n = self.players;
        let hands_count = n + usize::from(self.ghost);
        let mut deck = full_deck(&self.options, n);
        shuffle(&mut deck);
        let scheduled = self.schedule[self.round as usize - 1];
        let dealt = hand_size(scheduled, deck.len(), hands_count);
        let hands = (0..hands_count)
            .map(|_| {
                let mut hand = deck.split_off(deck.len() - dealt as usize);
                hand.sort_by_key(|c| c.sort_key());
                hand
            })
            .collect();
        self.hand = Some(Round {
            dealt,
            hands,
            undealt: deck,
            bids: vec![None; n],
            won: vec![0; n],
            bonus: vec![0; n],
            bets: vec![0; n],
            alliances: Vec::new(),
            trick: Vec::new(),
            leader: starter,
            starter,
            tricks_left: dealt,
            last_trick: None,
        });
        self.phase = Phase::Bidding;
        self.act(starter as u8, ActionKind::Deal);
        let cards = if dealt == 1 {
            "one card".to_string()
        } else {
            format!("{dealt} cards")
        };
        self.log.note(format!(
            "Round {}: {cards} each. {} leads. Place your bids!",
            self.round,
            name(seats, starter)
        ));
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
        self.players = seats.len();
        self.ghost = self.players == 2;
        self.schedule = self.options.length.rounds();
        self.round = 1;
        self.scores = vec![Vec::new(); self.players];
        self.log.clear();
        if self.ghost {
            self.log
                .note("Two players: a ghost hand plays second in every trick.".into());
        }
        // The host deals; the player on their left starts.
        self.deal(seats, 1 % self.players);
        self.sync(seats);
    }

    fn on_bid(&mut self, seats: &Seats, player: usize, bid: u8) {
        if !matches!(self.phase, Phase::Bidding) {
            return;
        }
        let Some(round) = self.hand.as_mut() else {
            return;
        };
        if bid > round.dealt {
            error(seats, player, "You can't bid more tricks than there are.");
            return;
        }
        round.bids[player] = Some(bid);
        if round.bids.iter().all(|b| b.is_some()) {
            let bids: Vec<String> = round
                .bids
                .iter()
                .enumerate()
                .map(|(i, b)| format!("{} {}", name(seats, i), b.unwrap_or_default()))
                .collect();
            self.phase = Phase::Playing;
            self.act(player as u8, ActionKind::Bid);
            self.log
                .note(format!("Yo ho ho! Bids: {}.", bids.join(", ")));
        } else {
            self.act(player as u8, ActionKind::Bid);
        }
        self.sync(seats);
        self.ghost_turns(seats);
    }

    fn on_play(&mut self, seats: &Seats, player: usize, card: u8, pirate: bool) {
        if self.current() != Some(player) {
            error(seats, player, "It's not your turn.");
            return;
        }
        let Some(round) = self.hand.as_mut() else {
            return;
        };
        let hand = &round.hands[player];
        let Some(pos) = hand.iter().position(|c| c.id == card) else {
            error(seats, player, "That card isn't in your hand.");
            return;
        };
        if !can_play(hand, &round.trick, &hand[pos]) {
            error(seats, player, "You have to follow suit.");
            return;
        }
        self.play(seats, player, pos, pirate);
        self.sync(seats);
        self.ghost_turns(seats);
    }

    /// Put the card at `pos` in `hand`'s hand on the table, finishing the
    /// trick if everyone has played.
    fn play(&mut self, seats: &Seats, hand: usize, pos: usize, pirate: bool) {
        let id = self.wire(hand);
        let Some(round) = self.hand.as_mut() else {
            return;
        };
        let card = round.hands[hand].remove(pos);
        let pirate = pirate && card.face == Face::Turncoat;
        round.trick.push(Played {
            player: id,
            card,
            pirate,
        });
        if round.trick.len() == 1 {
            round.last_trick = None;
        }
        let done = round.trick.len() == round.hands.len();
        self.act(id, ActionKind::Played { card, done });
        if card.face == Face::Turncoat {
            let as_what = if pirate { "a pirate" } else { "a flag" };
            let who = self.who(seats, hand);
            self.log
                .note(format!("{who} played the Turncoat as {as_what}."));
        }
        if done {
            self.finish_trick(seats);
        }
    }

    fn finish_trick(&mut self, seats: &Seats) {
        let Some(round) = self.hand.as_ref() else {
            return;
        };
        let trick = round.trick.clone();
        let out = resolve(&trick);
        let next = self.hand_of(trick[out.next].player);
        let winner = out.winner.map(|w| self.hand_of(trick[w].player));
        let human = |h: usize| h < self.players;

        let mut line = match winner {
            Some(w) => {
                let mut s = format!("{} took the trick", self.who(seats, w));
                if out.bonus > 0 && human(w) {
                    s.push_str(&format!(" (+{} bonus if they make their bid)", out.bonus));
                }
                s.push('.');
                s
            }
            None => format!("No one takes it. {} leads next.", self.who(seats, next)),
        };
        if !out.why.is_empty() {
            line = format!("{} {line}", out.why);
        }
        self.log.note(line);

        // A pirate's ability, if the trick was won with a pirate card.
        let ability = match (winner, out.winner) {
            (Some(w), Some(i)) if human(w) && self.options.crew => match trick[i].card.face {
                Face::Pirate(crew) => Some((w, crew)),
                _ => None,
            },
            _ => None,
        };

        let players = self.players;
        let Some(round) = self.hand.as_mut() else {
            return;
        };
        if let Some(w) = winner.filter(|&w| w < players) {
            round.won[w] += 1;
            round.bonus[w] += out.bonus;
        }
        for p in &trick {
            if p.card.face == Face::Loot && p.player != GHOST {
                let looter = p.player as usize;
                if let Some(w) = winner.filter(|&w| w < players && w != looter) {
                    round.alliances.push((looter, w));
                }
            }
        }
        round.last_trick = Some(TrickView {
            cards: trick,
            winner: winner.map(|w| if w == players { GHOST } else { w as u8 }),
            next: if next == players { GHOST } else { next as u8 },
            bonus: out.bonus,
            why: out.why,
        });
        round.trick.clear();
        round.leader = next;
        round.tricks_left -= 1;
        let last = round.tricks_left == 0;

        match ability {
            // Only the Brute's ability works after the last trick.
            Some((w, crew)) if !last || crew == Crew::Brute => self.start_ability(seats, w, crew),
            _ if last => self.end_round(seats),
            _ => {}
        }
    }

    fn start_ability(&mut self, seats: &Seats, player: usize, crew: Crew) {
        let who = name(seats, player);
        if crew == Crew::Smuggler {
            let Some(round) = self.hand.as_mut() else {
                return;
            };
            let take = round.undealt.len().min(2);
            if take == 0 {
                return;
            }
            let drawn = round.undealt.split_off(round.undealt.len() - take);
            round.hands[player].extend(drawn);
            self.act(player as u8, ActionKind::Smuggled);
        }
        self.log.note(format!(
            "{who} won with {}: {}",
            crew.name(),
            crew.ability().to_lowercase()
        ));
        self.phase = Phase::Ability(player, crew);
    }

    /// Wrap up an ability and carry on with the round.
    fn finish_ability(&mut self, seats: &Seats) {
        self.phase = Phase::Playing;
        if self.hand.as_ref().is_some_and(|r| r.tricks_left == 0) {
            self.end_round(seats);
        } else {
            self.act(GHOST, ActionKind::Other);
        }
        self.sync(seats);
        self.ghost_turns(seats);
    }

    fn ability(&self, player: usize) -> Option<Crew> {
        match self.phase {
            Phase::Ability(p, crew) if p == player => Some(crew),
            _ => None,
        }
    }

    fn on_choose_leader(&mut self, seats: &Seats, player: usize, leader: u8) {
        if self.ability(player) != Some(Crew::Navigator) || leader as usize >= self.players {
            return;
        }
        if let Some(round) = self.hand.as_mut() {
            round.leader = leader as usize;
        }
        self.log.note(format!(
            "{} will lead the next trick.",
            name(seats, leader as usize)
        ));
        self.finish_ability(seats);
    }

    fn on_smuggle(&mut self, seats: &Seats, player: usize, cards: Vec<u8>) {
        if self.ability(player) != Some(Crew::Smuggler) {
            return;
        }
        let Some(round) = self.hand.as_mut() else {
            return;
        };
        let extra = round.hands[player].len() - round.tricks_left as usize;
        let mut cards = cards;
        cards.sort_unstable();
        cards.dedup();
        let hand = &mut round.hands[player];
        if cards.len() != extra || !cards.iter().all(|id| hand.iter().any(|c| c.id == *id)) {
            error(seats, player, &format!("Pick {extra} cards to discard."));
            return;
        }
        hand.retain(|c| !cards.contains(&c.id));
        self.log.note(format!(
            "{} swapped cards with the deck.",
            name(seats, player)
        ));
        self.finish_ability(seats);
    }

    fn on_bet(&mut self, seats: &Seats, player: usize, points: u8) {
        if self.ability(player) != Some(Crew::Gambler) || ![0, 10, 20].contains(&points) {
            return;
        }
        if let Some(round) = self.hand.as_mut() {
            round.bets[player] += points as i32;
        }
        let who = name(seats, player);
        self.log.note(if points == 0 {
            format!("{who} kept their gold.")
        } else {
            format!("{who} bet {points} points on making their bid.")
        });
        self.finish_ability(seats);
    }

    fn on_change_bid(&mut self, seats: &Seats, player: usize, by: i8) {
        if self.ability(player) != Some(Crew::Brute) || !(-1..=1).contains(&by) {
            return;
        }
        let Some(round) = self.hand.as_mut() else {
            return;
        };
        let old = round.bids[player].unwrap_or_default();
        let new = (old as i16 + by as i16).clamp(0, round.dealt as i16) as u8;
        round.bids[player] = Some(new);
        let who = name(seats, player);
        self.log.note(if new == old {
            format!("{who} kept their bid at {old}.")
        } else {
            format!("{who} changed their bid from {old} to {new}.")
        });
        self.finish_ability(seats);
    }

    fn on_done(&mut self, seats: &Seats, player: usize) {
        if self.ability(player) == Some(Crew::Seer) {
            self.finish_ability(seats);
        }
    }

    /// Let the ghost play whenever it's its turn, showing each card. Call
    /// after syncing the move before it.
    fn ghost_turns(&mut self, seats: &Seats) {
        while self.ghost && self.current() == Some(self.players) {
            // It flips the top card of its face-down hand; a Turncoat flees.
            self.play(seats, self.players, 0, false);
            self.sync(seats);
        }
    }

    fn end_round(&mut self, seats: &Seats) {
        let Some(round) = self.hand.as_ref() else {
            return;
        };
        let mut results: Vec<RoundResult> = (0..self.players)
            .map(|p| {
                let bid = round.bids[p].unwrap_or_default();
                let won = round.won[p];
                let (base, made) = round_points(bid, won, round.dealt, 0);
                let bonus = if made { round.bonus[p] } else { 0 };
                let bet = if made { round.bets[p] } else { -round.bets[p] };
                RoundResult {
                    player: p as u8,
                    bid,
                    won,
                    base,
                    bonus,
                    loot: 0,
                    bet,
                    total: 0,
                    made,
                }
            })
            .collect();
        for &(a, b) in &round.alliances {
            if results[a].made && results[b].made {
                results[a].loot += 20;
                results[b].loot += 20;
            }
        }
        for r in &mut results {
            r.total = r.base + r.bonus + r.loot + r.bet;
            self.scores[r.player as usize].push(r.total);
        }
        let made: Vec<String> = results
            .iter()
            .filter(|r| r.made)
            .map(|r| name(seats, r.player as usize))
            .collect();
        self.log.note(if made.is_empty() {
            "Round over. Nobody made their bid!".into()
        } else {
            format!("Round over. Made their bids: {}.", made.join(", "))
        });

        if self.round as usize >= self.schedule.len() {
            let totals: Vec<i32> = self.scores.iter().map(|s| s.iter().sum()).collect();
            let best = totals.iter().max().copied().unwrap_or_default();
            let leaders: Vec<usize> = (0..totals.len()).filter(|&i| totals[i] == best).collect();
            if leaders.len() > 1 {
                let size = self.schedule.last().copied().unwrap_or(10);
                self.schedule.push(size);
                self.log
                    .note("A tie for the lead! One more round to settle it.".into());
                self.phase = Phase::RoundOver(results);
                return;
            }
            self.log.note(format!(
                "Game over! {} wins with {best} points.",
                name(seats, leaders[0])
            ));
            self.phase = Phase::GameOver(results, standings(&totals, false));
        } else {
            self.phase = Phase::RoundOver(results);
        }
    }

    fn on_next_round(&mut self, seats: &Seats, player: usize) {
        if player != 0 || !matches!(self.phase, Phase::RoundOver(_)) {
            return;
        }
        let starter = self
            .hand
            .as_ref()
            .map_or(0, |r| (r.starter + 1) % self.players);
        self.round += 1;
        self.deal(seats, starter);
        self.sync(seats);
    }

    fn on_arrange(&mut self, seats: &Seats, player: usize, order: Vec<u8>) {
        let Some(round) = self.hand.as_mut() else {
            return;
        };
        let Some(hand) = round.hands.get_mut(player) else {
            return;
        };
        if !arrange_hand(hand, &order) {
            return;
        }
        seats.send(player, &ServerMsg::Game(Box::new(self.view(seats, player))));
    }

    fn on_play_again(&mut self, seats: &mut Seats) {
        if !matches!(self.phase, Phase::GameOver(..)) {
            return;
        }
        seats.drop_disconnected();
        self.phase = Phase::Lobby;
        self.hand = None;
        self.broadcast_lobby(seats);
    }
}

impl RoomGame for PiratesWager {
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
            Phase::Lobby => self.options.describe(),
            _ => format!("Round {} of {}", self.round, self.schedule.len()),
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
            ClientMsg::Bid { bid } => self.on_bid(seats, player, bid),
            ClientMsg::Play { card, pirate } => self.on_play(seats, player, card, pirate),
            ClientMsg::ChooseLeader { player: p } => self.on_choose_leader(seats, player, p),
            ClientMsg::Discard { cards } => self.on_smuggle(seats, player, cards),
            ClientMsg::Bet { points } => self.on_bet(seats, player, points),
            ClientMsg::ChangeBid { by } => self.on_change_bid(seats, player, by),
            ClientMsg::Done => self.on_done(seats, player),
            ClientMsg::Arrange { order } => self.on_arrange(seats, player, order),
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
    use crate::games::pirates_wager::logic::Length;
    use rand::RngExt;

    /// Play whole games with random legal moves and check nothing panics,
    /// stalls or loses cards.
    fn simulate(players: usize, options: Options) {
        let mut rng = rand::rng();
        let mut seats = Seats::default();
        for _ in 0..players {
            seats.add_for_test();
        }
        let mut g = PiratesWager::new(options);
        g.handle(&mut seats, 0, ClientMsg::StartGame);
        assert_eq!(g.ghost, players == 2);
        for _ in 0..20_000 {
            match &g.phase {
                Phase::Lobby => panic!("game didn't start"),
                Phase::Bidding => {
                    let dealt = g.hand.as_ref().unwrap().dealt;
                    for p in 0..players {
                        let bid = rng.random_range(0..=dealt);
                        g.handle(&mut seats, p, ClientMsg::Bid { bid });
                    }
                }
                Phase::Playing => {
                    let p = g.current().expect("someone must be up");
                    assert!(p < players, "the ghost should have played itself");
                    let round = g.hand.as_ref().unwrap();
                    let hand = &round.hands[p];
                    let legal: Vec<u8> = hand
                        .iter()
                        .filter(|c| can_play(hand, &round.trick, c))
                        .map(|c| c.id)
                        .collect();
                    assert!(!legal.is_empty(), "no legal card");
                    let card = legal[rng.random_range(0..legal.len())];
                    let pirate = rng.random_bool(0.5);
                    g.handle(&mut seats, p, ClientMsg::Play { card, pirate });
                }
                Phase::Ability(p, crew) => {
                    let (p, crew) = (*p, *crew);
                    let msg = match crew {
                        Crew::Navigator => ClientMsg::ChooseLeader {
                            player: rng.random_range(0..players) as u8,
                        },
                        Crew::Smuggler => {
                            let round = g.hand.as_ref().unwrap();
                            let extra = round.hands[p].len() - round.tricks_left as usize;
                            ClientMsg::Discard {
                                cards: round.hands[p].iter().take(extra).map(|c| c.id).collect(),
                            }
                        }
                        Crew::Gambler => ClientMsg::Bet { points: 20 },
                        Crew::Seer => ClientMsg::Done,
                        Crew::Brute => ClientMsg::ChangeBid {
                            by: rng.random_range(-1..=1),
                        },
                    };
                    g.handle(&mut seats, p, msg);
                    assert!(
                        !matches!(g.phase, Phase::Ability(q, c) if q == p && c == crew),
                        "ability wasn't resolved"
                    );
                }
                Phase::RoundOver(results) => {
                    assert_eq!(results.len(), players);
                    let round = g.hand.as_ref().unwrap();
                    assert!(round.hands.iter().all(|h| h.is_empty()), "cards left over");
                    g.handle(&mut seats, 0, ClientMsg::NextRound);
                }
                Phase::GameOver(_, standings) => {
                    assert_eq!(standings.len(), players);
                    assert!(g.scores.iter().all(|s| s.len() == g.schedule.len()));
                    assert_eq!(standings.iter().filter(|s| s.place == 1).count(), 1);
                    return;
                }
            }
        }
        panic!("game never finished");
    }

    #[test]
    fn random_games() {
        let all = Options {
            kraken: true,
            whale: true,
            loot: true,
            crew: true,
            length: Length::Classic,
        };
        for _ in 0..20 {
            for players in 2..=MAX_PLAYERS {
                simulate(players, Options::default());
                simulate(players, all.clone());
            }
        }
        for length in Length::ALL {
            simulate(
                4,
                Options {
                    length,
                    ..all.clone()
                },
            );
            simulate(
                2,
                Options {
                    length,
                    ..all.clone()
                },
            );
        }
    }

    #[test]
    fn ghost_plays_second() {
        let mut seats = Seats::default();
        seats.add_for_test();
        seats.add_for_test();
        let mut g = PiratesWager::new(Options {
            length: Length::Barrage,
            ..Options::default()
        });
        g.handle(&mut seats, 0, ClientMsg::StartGame);
        g.handle(&mut seats, 0, ClientMsg::Bid { bid: 1 });
        g.handle(&mut seats, 1, ClientMsg::Bid { bid: 1 });
        // Player 1 leads round one; the ghost answers at once.
        assert_eq!(g.current(), Some(1));
        let round = g.hand.as_ref().unwrap();
        let card = round.hands[1]
            .iter()
            .find(|c| can_play(&round.hands[1], &round.trick, c))
            .unwrap()
            .id;
        g.handle(&mut seats, 1, ClientMsg::Play { card, pirate: true });
        let round = g.hand.as_ref().unwrap();
        assert_eq!(round.trick.len(), 2);
        assert_eq!(round.trick[1].player, GHOST);
        assert_eq!(g.current(), Some(0));
    }
}
