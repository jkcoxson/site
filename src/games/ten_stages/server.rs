// Jackson Coxson
// Ten Stages game server, running on the shared room engine.

use crate::games::cards::server::{
    arrange_hand, broadcast_lobby, draw_from_deck, error, name, shuffle, Log,
};
use crate::games::cards::Standing;
use crate::games::rooms::{RoomGame, Seats};

use super::logic::{
    describe, full_deck, needs, plan_hits, Action, ActionKind, Card, ClientMsg, GameView, Group,
    Options, PlayerView, RoundResult, ServerMsg, Status, HAND_SIZE, MAX_PLAYERS, MIN_PLAYERS,
    STAGES,
};

struct Round {
    deck: Vec<Card>,
    discard: Vec<Card>,
    hands: Vec<Vec<Card>>,
    /// What each player has laid down; empty until they make their stage.
    laid: Vec<Vec<Group>>,
    current: usize,
    drawn: bool,
    /// Players who lose their next turn.
    skipped: Vec<bool>,
    /// Players skipped since their last normal turn, who can't be skipped
    /// again until they've had one.
    skip_proof: Vec<bool>,
}

impl Round {
    #[cfg(test)]
    fn card_count(&self) -> usize {
        self.deck.len()
            + self.discard.len()
            + self.hands.iter().map(Vec::len).sum::<usize>()
            + self
                .laid
                .iter()
                .flatten()
                .map(|g| g.cards.len())
                .sum::<usize>()
    }

    /// Who a Skip from `player` may target.
    fn skip_targets(&self, player: usize) -> Vec<usize> {
        (0..self.hands.len())
            .filter(|&p| p != player && !self.skip_proof[p])
            .collect()
    }
}

enum Phase {
    Lobby,
    Playing,
    RoundOver(Vec<RoundResult>),
    GameOver(Vec<RoundResult>, Vec<Standing>),
}

pub struct TenStages {
    phase: Phase,
    options: Options,
    /// 1-based.
    round: u8,
    dealer: usize,
    /// Points scored by each player in each finished round.
    scores: Vec<Vec<u32>>,
    /// Stages each player has finished.
    done: Vec<u8>,
    hand: Option<Round>,
    log: Log,
    last: Option<Action>,
}

impl Default for TenStages {
    fn default() -> Self {
        Self::new(Options::default())
    }
}

impl TenStages {
    pub fn new(options: Options) -> Self {
        Self {
            phase: Phase::Lobby,
            options: Options {
                goal: options.goal.clamp(1, STAGES),
                ..options
            },
            round: 0,
            dealer: 0,
            scores: Vec::new(),
            done: Vec::new(),
            hand: None,
            log: Log::default(),
            last: None,
        }
    }

    /// The stage `player` is working on this round.
    fn stage_of(&self, player: usize) -> u8 {
        if self.options.fixed {
            self.round
        } else {
            self.done[player] + 1
        }
    }

    fn total(&self, player: usize) -> u32 {
        self.scores[player].iter().sum()
    }

    fn note(&mut self, line: String) {
        self.log.note(line);
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
        let hand = self.hand.as_ref();
        let status = match &self.phase {
            Phase::RoundOver(results) => Status::RoundOver {
                results: results.clone(),
            },
            Phase::GameOver(results, standings) => Status::GameOver {
                results: results.clone(),
                standings: standings.clone(),
            },
            _ => Status::Playing {
                drawn: hand.is_some_and(|h| h.drawn),
            },
        };
        let playing = matches!(self.phase, Phase::Playing);
        GameView {
            you: you as u8,
            host: 0,
            round: self.round,
            dealer: self.dealer as u8,
            current: hand.map_or(0, |h| h.current as u8),
            options: self.options,
            hand: hand.map(|h| h.hands[you].clone()).unwrap_or_default(),
            discard_top: hand.and_then(|h| h.discard.last().copied()),
            deck_size: hand.map_or(0, |h| h.deck.len() as u8),
            players: seats
                .iter()
                .enumerate()
                .map(|(i, s)| PlayerView {
                    id: i as u8,
                    nickname: s.nickname.clone(),
                    connected: s.connected,
                    total: self.total(i),
                    rounds: self.scores[i].clone(),
                    done: self.done[i],
                    stage: self.stage_of(i).min(self.options.goal),
                    cards: hand.map_or(0, |h| h.hands[i].len() as u8),
                    // Once the round is over, `stage` is already the next one.
                    laid: hand
                        .filter(|_| playing)
                        .map(|h| h.laid[i].clone())
                        .unwrap_or_default(),
                    skipped: playing && hand.is_some_and(|h| h.skipped[i]),
                    skip_proof: playing && hand.is_some_and(|h| h.skip_proof[i]),
                })
                .collect(),
            status,
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

    fn deal(&mut self, seats: &Seats) {
        let players = seats.len();
        let mut deck = full_deck();
        shuffle(&mut deck);
        let hands = (0..players)
            .map(|_| deck.split_off(deck.len() - HAND_SIZE))
            .collect();
        let discard = vec![deck.pop().expect("deck holds every hand")];
        let first = (self.dealer + 1) % players;
        let opens_on_skip = discard[0].is_skip();
        let mut round = Round {
            deck,
            discard,
            hands,
            laid: vec![Vec::new(); players],
            current: self.dealer,
            drawn: false,
            skipped: vec![false; players],
            skip_proof: vec![false; players],
        };
        self.phase = Phase::Playing;
        self.act(self.dealer, ActionKind::Deal);
        if opens_on_skip {
            round.skipped[first] = true;
            round.skip_proof[first] = true;
        }
        self.hand = Some(round);
        self.note(format!("Round {}: everyone gets ten cards.", self.round));
        if opens_on_skip {
            self.note(format!(
                "The deal turned up a Skip, so {} loses the first turn.",
                name(seats, first)
            ));
        }
        self.advance(seats);
    }

    /// Pass the turn on, past anyone who's been skipped.
    fn advance(&mut self, seats: &Seats) {
        let Some(round) = self.hand.as_mut() else {
            return;
        };
        let n = round.hands.len();
        let mut next = (round.current + 1) % n;
        let mut passed = Vec::new();
        while round.skipped[next] {
            round.skipped[next] = false;
            passed.push(next);
            next = (next + 1) % n;
        }
        round.skip_proof[next] = false;
        round.current = next;
        round.drawn = false;
        for p in passed {
            self.note(format!("{} is skipped.", name(seats, p)));
        }
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
        self.dealer = 0;
        self.scores = vec![Vec::new(); seats.len()];
        self.done = vec![0; seats.len()];
        self.log.clear();
        self.deal(seats);
        self.sync(seats);
    }

    /// The current player's round, if `player` may act right now.
    fn turn(&mut self, seats: &Seats, player: usize) -> Option<&mut Round> {
        if !matches!(self.phase, Phase::Playing) {
            return None;
        }
        let round = self.hand.as_mut()?;
        if round.current != player {
            error(seats, player, "It's not your turn.");
            return None;
        }
        Some(round)
    }

    /// The current player's round once they've drawn.
    fn drawn_turn(&mut self, seats: &Seats, player: usize) -> Option<&mut Round> {
        let round = self.turn(seats, player)?;
        if !round.drawn {
            error(seats, player, "Draw a card first.");
            return None;
        }
        Some(round)
    }

    fn on_draw(&mut self, seats: &Seats, player: usize, from_discard: bool) {
        let Some(round) = self.turn(seats, player) else {
            return;
        };
        if round.drawn {
            error(seats, player, "You already drew this turn.");
            return;
        }
        let card = if from_discard {
            match round.discard.last() {
                None => {
                    error(seats, player, "The discard pile is empty.");
                    return;
                }
                Some(c) if c.is_skip() => {
                    error(
                        seats,
                        player,
                        "A Skip can't be taken from the discard pile.",
                    );
                    return;
                }
                Some(_) => round.discard.pop().expect("checked above"),
            }
        } else {
            let Some(card) = draw_from_deck(&mut round.deck, &mut round.discard) else {
                error(seats, player, "There are no cards left to draw.");
                return;
            };
            card
        };
        round.hands[player].push(card);
        round.drawn = true;
        if from_discard {
            self.act(player, ActionKind::DrewDiscard { card });
            self.note(format!(
                "{} took {} from the discard pile.",
                name(seats, player),
                card.label()
            ));
        } else {
            self.act(player, ActionKind::DrewDeck);
            self.note(format!("{} drew from the deck.", name(seats, player)));
        }
        self.sync(seats);
    }

    fn on_lay_down(&mut self, seats: &Seats, player: usize, groups: Vec<Vec<u8>>) {
        let stage = self.stage_of(player);
        let Some(round) = self.drawn_turn(seats, player) else {
            return;
        };
        if !round.laid[player].is_empty() {
            error(seats, player, "You've already laid down this round.");
            return;
        }
        let list = needs(stage);
        let hand = &round.hands[player];
        let mut ids: Vec<u8> = groups.iter().flatten().copied().collect();
        ids.sort_unstable();
        let all_held = ids.iter().all(|id| hand.iter().any(|c| c.id == *id));
        if groups.len() != list.len() || ids.windows(2).any(|w| w[0] == w[1]) || !all_held {
            error(seats, player, "Those cards don't make your stage.");
            return;
        }
        let made: Option<Vec<Group>> = list
            .iter()
            .zip(&groups)
            .map(|(&need, ids)| {
                let cards: Vec<Card> = ids
                    .iter()
                    .filter_map(|id| hand.iter().find(|c| c.id == *id).copied())
                    .collect();
                Group::in_order(need, &cards)
            })
            .collect();
        let Some(made) = made else {
            error(seats, player, "Those cards don't make your stage.");
            return;
        };
        round.hands[player].retain(|c| !ids.contains(&c.id));
        round.laid[player] = made;
        let out = round.hands[player].is_empty();
        self.act(player, ActionKind::Played);
        self.note(format!(
            "{} laid down stage {stage}: {}.",
            name(seats, player),
            describe(stage)
        ));
        if out {
            self.end_round(seats, player);
        }
        self.sync(seats);
    }

    fn on_hit(&mut self, seats: &Seats, player: usize, card: u8, owner: u8, group: u8, low: bool) {
        let Some(round) = self.drawn_turn(seats, player) else {
            return;
        };
        if round.laid[player].is_empty() {
            error(seats, player, "Lay down your own stage before hitting.");
            return;
        }
        let Some(pos) = round.hands[player].iter().position(|c| c.id == card) else {
            error(seats, player, "That card isn't in your hand.");
            return;
        };
        let held = round.hands[player][pos];
        let Some(target) = round
            .laid
            .get_mut(owner as usize)
            .and_then(|l| l.get_mut(group as usize))
        else {
            error(seats, player, "There's no group there.");
            return;
        };
        let label = target.label();
        if !target.hit(held, low) {
            error(seats, player, "That card doesn't fit there.");
            return;
        }
        round.hands[player].remove(pos);
        let out = round.hands[player].is_empty();
        self.act(player, ActionKind::Played);
        let whose = if owner as usize == player {
            "their".to_string()
        } else {
            format!("{}'s", name(seats, owner as usize))
        };
        self.note(format!(
            "{} hit {} onto {whose} {}.",
            name(seats, player),
            held.label(),
            label.to_lowercase()
        ));
        if out {
            self.end_round(seats, player);
        }
        self.sync(seats);
    }

    fn on_hit_all(&mut self, seats: &Seats, player: usize) {
        let Some(round) = self.drawn_turn(seats, player) else {
            return;
        };
        if round.laid[player].is_empty() {
            error(seats, player, "Lay down your own stage before hitting.");
            return;
        }
        // Flatten the table so the planner sees every group at once.
        let owners: Vec<(usize, usize)> = round
            .laid
            .iter()
            .enumerate()
            .flat_map(|(p, l)| (0..l.len()).map(move |g| (p, g)))
            .collect();
        let mut table: Vec<Group> = round.laid.iter().flatten().cloned().collect();
        let hits = plan_hits(&mut table, &round.hands[player]);
        if hits.is_empty() {
            error(seats, player, "None of your cards fit on the table.");
            return;
        }
        for (group, &(p, g)) in table.into_iter().zip(&owners) {
            round.laid[p][g] = group;
        }
        round.hands[player].retain(|c| !hits.iter().any(|h| h.card.id == c.id));
        let out = round.hands[player].is_empty();
        self.act(player, ActionKind::Played);
        let count = hits.len();
        self.note(format!(
            "{} hit {count} card{} onto the table.",
            name(seats, player),
            if count == 1 { "" } else { "s" }
        ));
        if out {
            self.end_round(seats, player);
        }
        self.sync(seats);
    }

    fn on_discard(&mut self, seats: &Seats, player: usize, card: u8, skip: Option<u8>) {
        let Some(round) = self.drawn_turn(seats, player) else {
            return;
        };
        let Some(pos) = round.hands[player].iter().position(|c| c.id == card) else {
            error(seats, player, "That card isn't in your hand.");
            return;
        };
        let discarded = round.hands[player][pos];
        let mut target = None;
        if discarded.is_skip() {
            let targets = round.skip_targets(player);
            target = match skip {
                Some(t) if targets.contains(&(t as usize)) => Some(t as usize),
                Some(_) => {
                    error(
                        seats,
                        player,
                        "They were just skipped; they get a turn before they can be skipped again.",
                    );
                    return;
                }
                None if targets.len() > 1 => {
                    error(seats, player, "Choose who to skip.");
                    return;
                }
                // With nobody left to skip, it's a plain discard.
                None => targets.first().copied(),
            };
        }
        round.hands[player].remove(pos);
        round.discard.push(discarded);
        if let Some(t) = target {
            round.skipped[t] = true;
            round.skip_proof[t] = true;
        }
        let out = round.hands[player].is_empty();
        self.act(player, ActionKind::Discarded { card: discarded });
        self.note(match target {
            Some(t) => format!(
                "{} played a Skip on {}.",
                name(seats, player),
                name(seats, t)
            ),
            None => format!("{} discarded {}.", name(seats, player), discarded.label()),
        });
        if out {
            self.end_round(seats, player);
        } else {
            self.advance(seats);
        }
        self.sync(seats);
    }

    /// `winner` went out: score the hands and move on whoever made their stage.
    fn end_round(&mut self, seats: &Seats, winner: usize) {
        let Some(round) = self.hand.as_ref() else {
            return;
        };
        let results: Vec<RoundResult> = (0..round.hands.len())
            .map(|p| RoundResult {
                player: p as u8,
                stage: self.stage_of(p),
                advanced: !round.laid[p].is_empty(),
                went_out: p == winner,
                hand: round.hands[p].clone(),
                points: round.hands[p].iter().map(Card::points).sum(),
            })
            .collect();
        for r in &results {
            let p = r.player as usize;
            self.scores[p].push(r.points);
            if r.advanced {
                self.done[p] += 1;
            }
        }
        self.note(format!("{} went out!", name(seats, winner)));

        let goal = self.options.goal;
        let over = if self.options.fixed {
            self.round >= goal
        } else {
            self.done.iter().any(|&d| d >= goal)
        };
        if !over {
            self.phase = Phase::RoundOver(results);
            return;
        }
        // Most stages first (unless everyone moved on together), then the
        // lowest score. Players tied on both share the place.
        let key = |p: usize| {
            let stages = if self.options.fixed {
                0
            } else {
                self.done[p].min(goal)
            };
            (std::cmp::Reverse(stages), self.total(p))
        };
        let players = self.scores.len();
        let standings: Vec<Standing> = (0..players)
            .map(|p| Standing {
                player: p as u8,
                total: self.total(p) as i32,
                place: 1 + (0..players).filter(|&o| key(o) < key(p)).count() as u8,
            })
            .collect();
        let winners: Vec<String> = standings
            .iter()
            .filter(|s| s.place == 1)
            .map(|s| name(seats, s.player as usize))
            .collect();
        self.note(format!("Game over! {} won.", winners.join(" and ")));
        self.phase = Phase::GameOver(results, standings);
    }

    fn on_next_round(&mut self, seats: &Seats, player: usize) {
        if player != 0 || !matches!(self.phase, Phase::RoundOver(_)) {
            return;
        }
        self.round += 1;
        self.dealer = (self.dealer + 1) % seats.len();
        self.deal(seats);
        self.sync(seats);
    }

    fn on_arrange(&mut self, seats: &Seats, player: usize, order: Vec<u8>) {
        let Some(hand) = self.hand.as_mut().and_then(|r| r.hands.get_mut(player)) else {
            return;
        };
        if arrange_hand(hand, &order) {
            seats.send(player, &ServerMsg::Game(Box::new(self.view(seats, player))));
        }
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

impl RoomGame for TenStages {
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
            Phase::Lobby => "Ten Stages".into(),
            _ => format!("Ten Stages · round {}", self.round),
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
            ClientMsg::Draw { from_discard } => self.on_draw(seats, player, from_discard),
            ClientMsg::LayDown { groups } => self.on_lay_down(seats, player, groups),
            ClientMsg::Hit {
                card,
                owner,
                group,
                low,
            } => self.on_hit(seats, player, card, owner, group, low),
            ClientMsg::HitAll => self.on_hit_all(seats, player),
            ClientMsg::Discard { card, skip } => self.on_discard(seats, player, card, skip),
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
    use super::super::logic::{lay_downs, Face, Need};
    use super::*;
    use rand::RngExt;

    fn started(players: usize, options: Options) -> (Seats, TenStages) {
        let mut seats = Seats::default();
        for _ in 0..players {
            seats.add_for_test();
        }
        let mut g = TenStages::new(options);
        g.handle(&mut seats, 0, ClientMsg::StartGame);
        (seats, g)
    }

    fn round(g: &TenStages) -> &Round {
        g.hand.as_ref().unwrap()
    }

    /// Put `faces` in the current player's hand, ready to act.
    fn give(g: &mut TenStages, faces: &[Face]) -> usize {
        let r = g.hand.as_mut().unwrap();
        let p = r.current;
        r.hands[p] = faces
            .iter()
            .enumerate()
            .map(|(i, &face)| Card {
                id: 200 + i as u8,
                face,
            })
            .collect();
        r.drawn = true;
        p
    }

    #[test]
    fn skips_follow_the_rules() {
        let (mut seats, mut g) = started(3, Options::default());
        let p = give(&mut g, &[Face::Skip, Face::Wild]);
        let other = (p + 1) % 3;
        let third = (p + 2) % 3;
        // Two players could be skipped, so you have to choose.
        g.handle(
            &mut seats,
            p,
            ClientMsg::Discard {
                card: 200,
                skip: None,
            },
        );
        assert_eq!(round(&g).current, p);
        g.handle(
            &mut seats,
            p,
            ClientMsg::Discard {
                card: 200,
                skip: Some(other as u8),
            },
        );
        assert_eq!(round(&g).current, third);
        assert!(round(&g).skip_proof[other]);
        // The third player can't skip them again before their own turn.
        give(&mut g, &[Face::Skip, Face::Wild]);
        g.handle(
            &mut seats,
            third,
            ClientMsg::Discard {
                card: 200,
                skip: Some(other as u8),
            },
        );
        assert_eq!(round(&g).current, third);
        // With only one player left to skip, it picks them.
        g.handle(
            &mut seats,
            third,
            ClientMsg::Discard {
                card: 200,
                skip: None,
            },
        );
        assert_eq!(round(&g).current, other);
        // `p` lost their turn and can't be skipped again yet; `other` has
        // now played normally, so they're fair game again.
        assert!(!round(&g).skipped[p]);
        assert!(round(&g).skip_proof[p]);
        assert!(!round(&g).skip_proof[other]);
        // The skip can't be drawn back.
        let r = g.hand.as_mut().unwrap();
        r.drawn = false;
        g.handle(&mut seats, other, ClientMsg::Draw { from_discard: true });
        assert!(!round(&g).drawn);
    }

    #[test]
    fn lay_down_hit_and_go_out() {
        let (mut seats, mut g) = started(2, Options::default());
        let n = |rank, color| Face::Number { rank, color };
        use super::super::logic::Color::*;
        let p = give(
            &mut g,
            &[
                n(7, Red),
                n(7, Blue),
                n(7, Green),
                n(3, Red),
                n(3, Blue),
                Face::Wild,
                n(7, Yellow),
                n(9, Red),
            ],
        );
        // Hitting before laying down isn't allowed.
        g.handle(&mut seats, p, ClientMsg::HitAll);
        assert_eq!(round(&g).hands[p].len(), 8);
        let best = &lay_downs(1, &round(&g).hands[p], &[])[0];
        let groups = best
            .groups
            .iter()
            .map(|gr| gr.cards.iter().map(|c| c.id).collect())
            .collect();
        g.handle(&mut seats, p, ClientMsg::LayDown { groups });
        assert_eq!(round(&g).laid[p].len(), 2);
        g.handle(&mut seats, p, ClientMsg::HitAll);
        assert_eq!(round(&g).hands[p].len(), 1);
        g.handle(
            &mut seats,
            p,
            ClientMsg::Discard {
                card: 207,
                skip: None,
            },
        );
        let Phase::RoundOver(results) = &g.phase else {
            panic!("round should be over");
        };
        assert!(results[p].went_out && results[p].advanced);
        assert_eq!(g.done[p], 1);
        assert_eq!(g.stage_of(p), 2);
        assert_eq!(g.stage_of(1 - p), 1);
    }

    #[test]
    fn bad_lay_downs_are_refused() {
        let (mut seats, mut g) = started(2, Options::default());
        let p = give(
            &mut g,
            &[
                Face::Wild,
                Face::Wild,
                Face::Wild,
                Face::Wild,
                Face::Wild,
                Face::Wild,
            ],
        );
        let groups = vec![vec![200, 201, 202], vec![203, 204, 205]];
        g.handle(&mut seats, p, ClientMsg::LayDown { groups });
        assert!(round(&g).laid[p].is_empty());
        let groups = vec![vec![200, 200, 201], vec![203, 204, 205]];
        g.handle(&mut seats, p, ClientMsg::LayDown { groups });
        assert!(round(&g).laid[p].is_empty());
    }

    /// Bots that lay down the best option, hit everything, and discard their
    /// highest stuck card, until the game ends.
    fn play_out(players: usize, options: Options) -> TenStages {
        let (mut seats, mut g) = started(players, options);
        let mut rng = rand::rng();
        for _ in 0..200_000 {
            match &g.phase {
                Phase::GameOver(..) => return g,
                Phase::RoundOver(_) => {
                    g.handle(&mut seats, 0, ClientMsg::NextRound);
                    continue;
                }
                _ => {}
            }
            assert_eq!(round(&g).card_count(), 108);
            let p = round(&g).current;
            if !round(&g).drawn {
                let from_discard = rng.random_bool(0.3);
                g.handle(&mut seats, p, ClientMsg::Draw { from_discard });
                if !round(&g).drawn {
                    g.handle(
                        &mut seats,
                        p,
                        ClientMsg::Draw {
                            from_discard: false,
                        },
                    );
                }
                continue;
            }
            let r = round(&g);
            if r.laid[p].is_empty() {
                let table: Vec<Group> = r.laid.iter().flatten().cloned().collect();
                if let Some(best) = lay_downs(g.stage_of(p), &r.hands[p], &table).first() {
                    let groups = best
                        .groups
                        .iter()
                        .map(|gr| gr.cards.iter().map(|c| c.id).collect())
                        .collect();
                    g.handle(&mut seats, p, ClientMsg::LayDown { groups });
                    assert!(!round(&g).laid[p].is_empty(), "lay down was refused");
                    continue;
                }
            } else {
                let mut table: Vec<Group> = r.laid.iter().flatten().cloned().collect();
                if !plan_hits(&mut table, &r.hands[p]).is_empty() {
                    g.handle(&mut seats, p, ClientMsg::HitAll);
                    continue;
                }
            }
            let r = round(&g);
            // Skips first, never a wild, else the card that helps this
            // stage least: a lonely number for sets, a duplicate or far-off
            // number for runs, a rare color for colors.
            let h = &r.hands[p];
            let list = needs(g.stage_of(p));
            let company = |c: &Card| match c.face {
                Face::Skip => -1,
                Face::Wild => 100,
                Face::Number { rank, color } => match list[0] {
                    Need::Color(_) => h.iter().filter(|o| o.color() == Some(color)).count() as i32,
                    Need::Run(_) => {
                        let twin = h.iter().any(|o| o.rank() == Some(rank) && o.id < c.id);
                        let near = h
                            .iter()
                            .filter_map(Card::rank)
                            .filter(|&o| o != rank && o.abs_diff(rank) <= 4)
                            .count() as i32;
                        if twin {
                            0
                        } else {
                            near
                        }
                    }
                    Need::Set(_) => h.iter().filter(|o| o.rank() == Some(rank)).count() as i32,
                },
            };
            let card = *h.iter().min_by_key(|c| (company(c), c.id)).unwrap();
            let skip = card
                .is_skip()
                .then(|| r.skip_targets(p).first().map(|&t| t as u8))
                .flatten();
            g.handle(
                &mut seats,
                p,
                ClientMsg::Discard {
                    card: card.id,
                    skip,
                },
            );
        }
        let r = round(&g);
        panic!(
            "game didn't finish: round {} done {:?} deck {} discard {} hands {:?} current {} drawn {}",
            g.round,
            g.done,
            r.deck.len(),
            r.discard.len(),
            r.hands.iter().map(Vec::len).collect::<Vec<_>>(),
            r.current,
            r.drawn
        );
    }

    #[test]
    fn random_games_finish() {
        for players in 2..=MAX_PLAYERS {
            let g = play_out(players, Options::default());
            assert!(g.done.iter().any(|&d| d >= STAGES));
        }
        let g = play_out(
            3,
            Options {
                goal: 4,
                fixed: true,
            },
        );
        assert_eq!(g.round, 4);
    }
}
