// Jackson Coxson
// Five Crowns game server, running on the shared room engine.

use crate::games::cards::server::{broadcast_lobby, error, name, shuffle, standings, Log};
use crate::games::cards::Standing;
use crate::games::rooms::{RoomGame, Seats};

use super::logic::{
    best_meld, full_deck, hand_size, rank_label, Action, ActionKind, Card, ClientMsg, GameView,
    Meld, PlayerView, RoundResult, ServerMsg, Stage, MAX_PLAYERS, MIN_PLAYERS, ROUNDS,
};

struct Round {
    deck: Vec<Card>,
    discard: Vec<Card>,
    hands: Vec<Vec<Card>>,
    current: usize,
    drawn: bool,
    /// Who went out first. Everyone else then plays one last turn.
    out: Option<usize>,
    /// How each player's hand was laid down at the end of their last turn.
    finals: Vec<Option<Meld>>,
}

enum Phase {
    Lobby,
    Playing,
    RoundOver(Vec<RoundResult>),
    GameOver(Vec<RoundResult>, Vec<Standing>),
}

pub struct FiveCrowns {
    phase: Phase,
    /// 1-based; the wild rank is `hand_size(round)`.
    round: u8,
    dealer: usize,
    /// Points scored by each player in each finished round.
    scores: Vec<Vec<u32>>,
    hand: Option<Round>,
    log: Log,
    last: Option<Action>,
}

impl Default for FiveCrowns {
    fn default() -> Self {
        Self::new()
    }
}

fn card_label(card: &Card) -> String {
    match card.suit {
        Some(suit) => format!("{}{}", rank_label(card.rank), suit.symbol()),
        None => "a joker".into(),
    }
}

impl FiveCrowns {
    pub fn new() -> Self {
        Self {
            phase: Phase::Lobby,
            round: 0,
            dealer: 0,
            scores: Vec::new(),
            hand: None,
            log: Log::default(),
            last: None,
        }
    }

    fn wild(&self) -> u8 {
        hand_size(self.round)
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
        broadcast_lobby::<GameView>(seats, "Eleven rounds, 3s wild up to kings wild.");
    }

    fn view(&self, seats: &Seats, you: usize) -> GameView {
        let hand = self.hand.as_ref();
        let stage = match &self.phase {
            Phase::RoundOver(results) => Stage::RoundOver {
                results: results.clone(),
            },
            Phase::GameOver(results, standings) => Stage::GameOver {
                results: results.clone(),
                standings: standings.clone(),
            },
            _ => Stage::Playing {
                drawn: hand.is_some_and(|h| h.drawn),
            },
        };
        GameView {
            you: you as u8,
            host: 0,
            round: self.round,
            wild: self.wild(),
            dealer: self.dealer as u8,
            current: hand.map_or(0, |h| h.current as u8),
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
                    total: self.scores[i].iter().sum(),
                    rounds: self.scores[i].clone(),
                })
                .collect(),
            out: hand.and_then(|h| h.out.map(|o| o as u8)),
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

    fn deal(&mut self, players: usize) {
        let mut deck = full_deck();
        shuffle(&mut deck);
        let size = hand_size(self.round) as usize;
        let hands = (0..players)
            .map(|_| deck.split_off(deck.len() - size))
            .collect();
        let discard = vec![deck.pop().expect("deck holds every hand")];
        self.hand = Some(Round {
            deck,
            discard,
            hands,
            current: (self.dealer + 1) % players,
            drawn: false,
            out: None,
            finals: vec![None; players],
        });
        self.phase = Phase::Playing;
        self.act(self.dealer, ActionKind::Deal);
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
        self.log.clear();
        self.deal(seats.len());
        self.note(format!("Round 1: three cards each, 3s are wild."));
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

    fn on_draw(&mut self, seats: &Seats, player: usize, from_discard: bool) {
        let Some(round) = self.turn(seats, player) else {
            return;
        };
        if round.drawn {
            error(seats, player, "You already drew this turn.");
            return;
        }
        let card = if from_discard {
            let Some(card) = round.discard.pop() else {
                error(seats, player, "The discard pile is empty.");
                return;
            };
            card
        } else {
            if round.deck.is_empty() {
                // Shuffle everything but the top discard back into the deck.
                let top = round.discard.pop();
                round.deck.append(&mut round.discard);
                shuffle(&mut round.deck);
                round.discard.extend(top);
            }
            let Some(card) = round.deck.pop() else {
                error(seats, player, "There are no cards left to draw.");
                return;
            };
            card
        };
        round.hands[player].push(card);
        round.drawn = true;
        self.act(
            player,
            if from_discard {
                ActionKind::DrewDiscard { card }
            } else {
                ActionKind::DrewDeck
            },
        );
        let line = if from_discard {
            format!(
                "{} took {} from the discard pile.",
                name(seats, player),
                card_label(&card)
            )
        } else {
            format!("{} drew from the deck.", name(seats, player))
        };
        self.note(line);
        self.sync(seats);
    }

    /// Discard `card` to end the turn. With `go_out` the rest of the hand
    /// must lay down completely.
    fn on_discard(&mut self, seats: &Seats, player: usize, card: u8, go_out: bool) {
        let wild = self.wild();
        let Some(round) = self.turn(seats, player) else {
            return;
        };
        if !round.drawn {
            error(seats, player, "Draw a card first.");
            return;
        }
        let hand = &mut round.hands[player];
        let Some(pos) = hand.iter().position(|c| c.id == card) else {
            error(seats, player, "That card isn't in your hand.");
            return;
        };
        let mut rest = hand.clone();
        let discarded = rest.remove(pos);
        let final_turn = round.out.is_some();
        let meld = (go_out || final_turn).then(|| best_meld(&rest, wild));
        if go_out && meld.as_ref().is_some_and(|m| m.points > 0) {
            error(seats, player, "Those cards can't all be laid down yet.");
            return;
        }
        *hand = rest;
        round.discard.push(discarded);
        let name = name(seats, player);
        let line = match &meld {
            Some(m) if m.points == 0 && round.out.is_none() => {
                round.out = Some(player);
                format!("{name} went out! Everyone else gets one last turn.")
            }
            Some(m) if m.points == 0 => format!("{name} went out too."),
            Some(m) => format!("{name} laid down and kept {} points.", m.points),
            None => format!("{name} discarded {}.", card_label(&discarded)),
        };
        round.finals[player] = meld;
        let n = round.hands.len();
        let next = (round.current + 1) % n;
        let round_done = round.out == Some(next);
        round.current = next;
        round.drawn = false;
        self.act(player, ActionKind::Discarded { card: discarded });
        self.note(line);
        if round_done {
            self.end_round(seats);
        }
        self.sync(seats);
    }

    fn end_round(&mut self, seats: &Seats) {
        let Some(round) = self.hand.as_ref() else {
            return;
        };
        let results: Vec<RoundResult> = round
            .finals
            .iter()
            .enumerate()
            .map(|(i, meld)| RoundResult {
                player: i as u8,
                meld: meld.clone().unwrap_or_default(),
                went_out: round.out == Some(i),
            })
            .collect();
        for r in &results {
            self.scores[r.player as usize].push(r.meld.points);
        }
        if self.round >= ROUNDS {
            let totals: Vec<i32> = self
                .scores
                .iter()
                .map(|s| s.iter().sum::<u32>() as i32)
                .collect();
            let standings: Vec<Standing> = standings(&totals, true);
            let best = totals.iter().min().copied().unwrap_or_default();
            let winners: Vec<String> = (0..totals.len())
                .filter(|&i| totals[i] == best)
                .map(|i| name(seats, i))
                .collect();
            self.note(format!(
                "Game over! {} won with {best} points.",
                winners.join(" and ")
            ));
            self.phase = Phase::GameOver(results, standings);
        } else {
            self.phase = Phase::RoundOver(results);
        }
    }

    fn on_next_round(&mut self, seats: &Seats, player: usize) {
        if player != 0 || !matches!(self.phase, Phase::RoundOver(_)) {
            return;
        }
        self.round += 1;
        self.dealer = (self.dealer + 1) % seats.len();
        self.deal(seats.len());
        let wild = rank_label(self.wild());
        self.note(format!(
            "Round {}: {} cards each, {wild}s are wild.",
            self.round,
            hand_size(self.round)
        ));
        self.sync(seats);
    }

    fn on_arrange(&mut self, seats: &Seats, player: usize, order: Vec<u8>) {
        let Some(round) = self.hand.as_mut() else {
            return;
        };
        let hand = &mut round.hands[player];
        let mut sorted_order = order.clone();
        sorted_order.sort_unstable();
        let mut ids: Vec<u8> = hand.iter().map(|c| c.id).collect();
        ids.sort_unstable();
        if sorted_order != ids {
            return;
        }
        hand.sort_by_key(|c| order.iter().position(|&id| id == c.id));
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

impl RoomGame for FiveCrowns {
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
            Phase::Lobby => "Five Crowns".into(),
            _ => format!("Five Crowns · round {}", self.round),
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
            ClientMsg::Discard { card } => self.on_discard(seats, player, card, false),
            ClientMsg::GoOut { card } => self.on_discard(seats, player, card, true),
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
