// Jackson Coxson
// Five Crowns: cards, scoring, the meld solver and the wire messages shared by
// the server and the browser.

use serde::{Deserialize, Serialize};

use crate::games::cards::melds::{self, Natural};
use crate::games::cards::{HandCard, Motion, Spot, Standing, TableView};

pub const ROUNDS: u8 = 11;
pub const MIN_PLAYERS: usize = 2;
/// Two decks hold 116 cards; seven players at 13 cards each still leaves a
/// healthy draw pile.
pub const MAX_PLAYERS: usize = 7;

// ── Cards ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Suit {
    Stars,
    Hearts,
    Clubs,
    Spades,
    Diamonds,
}

impl Suit {
    pub const ALL: [Suit; 5] = [
        Suit::Stars,
        Suit::Hearts,
        Suit::Clubs,
        Suit::Spades,
        Suit::Diamonds,
    ];

    pub fn symbol(self) -> &'static str {
        match self {
            Suit::Stars => "★",
            Suit::Hearts => "♥",
            Suit::Clubs => "♣",
            Suit::Spades => "♠",
            Suit::Diamonds => "♦",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Suit::Stars => "stars",
            Suit::Hearts => "hearts",
            Suit::Clubs => "clubs",
            Suit::Spades => "spades",
            Suit::Diamonds => "diamonds",
        }
    }
}

/// One physical card. Two decks are shuffled together, so the `id` is what
/// tells duplicates apart. A joker has rank 0 and no suit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Card {
    pub id: u8,
    pub rank: u8,
    pub suit: Option<Suit>,
}

impl Card {
    pub fn is_joker(&self) -> bool {
        self.rank == 0
    }

    /// Jokers are always wild; so is every card of this round's wild rank.
    pub fn is_wild(&self, wild: u8) -> bool {
        self.is_joker() || self.rank == wild
    }

    /// How melds see this card: wilds stand in for anything.
    pub fn natural(&self, wild: u8) -> Natural {
        match self.suit {
            Some(suit) if !self.is_wild(wild) => Some((self.rank, suit as u8)),
            _ => None,
        }
    }

    /// Penalty points when left unmelded at the end of a round.
    pub fn points(&self, wild: u8) -> u32 {
        if self.is_joker() {
            50
        } else if self.rank == wild {
            20
        } else {
            self.rank as u32
        }
    }

    pub fn rank_label(&self) -> &'static str {
        rank_label(self.rank)
    }
}

pub fn rank_label(rank: u8) -> &'static str {
    match rank {
        0 => "Joker",
        3 => "3",
        4 => "4",
        5 => "5",
        6 => "6",
        7 => "7",
        8 => "8",
        9 => "9",
        10 => "10",
        11 => "J",
        12 => "Q",
        13 => "K",
        _ => "?",
    }
}

/// Cards dealt in a round (1-based): 3 in the first, 13 in the eleventh. The
/// same number is the wild rank, so kings are wild in the last round.
pub fn hand_size(round: u8) -> u8 {
    round + 2
}

/// Both decks in order: per deck, eleven ranks in each of five suits plus
/// three jokers.
pub fn full_deck() -> Vec<Card> {
    let mut cards = Vec::with_capacity(116);
    for _ in 0..2 {
        for suit in Suit::ALL {
            for rank in 3..=13 {
                cards.push(Card {
                    id: cards.len() as u8,
                    rank,
                    suit: Some(suit),
                });
            }
        }
        for _ in 0..3 {
            cards.push(Card {
                id: cards.len() as u8,
                rank: 0,
                suit: None,
            });
        }
    }
    cards
}

// ── Melds ─────────────────────────────────────────────────────────────────────

/// Whether `cards` form a book (three or more of a rank) or a run (three or
/// more consecutive cards of one suit), with wilds standing in for anything.
pub fn is_valid_group(cards: &[Card], wild: u8) -> bool {
    let naturals: Vec<Natural> = cards.iter().map(|c| c.natural(wild)).collect();
    cards.len() >= 3
        && (melds::set_rank(&naturals).is_some()
            || melds::run_start(&naturals, 3..=13, true).is_some())
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Meld {
    /// Books and runs, each sorted for display.
    pub groups: Vec<Vec<Card>>,
    pub leftover: Vec<Card>,
    pub points: u32,
}

/// The arrangement of `cards` into books and runs that leaves the fewest
/// penalty points. Hands are at most 14 cards, so an exact search over subsets
/// is cheap. Any group of six or more splits into two valid groups, so only
/// groups of three to five cards need considering.
pub fn best_meld(cards: &[Card], wild: u8) -> Meld {
    let n = cards.len();
    assert!(n <= 16, "hand too large for the meld solver");
    let full = (1u32 << n) - 1;

    // Valid groups keyed by their lowest card, so each subset only tries
    // groups that cover its first remaining card.
    let mut groups_by_low: Vec<Vec<u32>> = vec![Vec::new(); n];
    for mask in 1..=full {
        let size = mask.count_ones();
        if !(3..=5).contains(&size) {
            continue;
        }
        let members: Vec<Card> = (0..n)
            .filter(|i| mask & (1 << i) != 0)
            .map(|i| cards[i])
            .collect();
        if is_valid_group(&members, wild) {
            groups_by_low[mask.trailing_zeros() as usize].push(mask);
        }
    }

    // best[mask] = (fewest leftover points, chosen group or 0 to leave the low card)
    let mut best: Vec<(u32, u32)> = vec![(0, 0); (full as usize) + 1];
    for mask in 1..=full {
        let low = mask.trailing_zeros() as usize;
        let mut choice = (
            cards[low].points(wild) + best[(mask & !(1 << low)) as usize].0,
            0,
        );
        for &g in &groups_by_low[low] {
            if g & !mask == 0 {
                let score = best[(mask & !g) as usize].0;
                if score < choice.0 {
                    choice = (score, g);
                }
            }
        }
        best[mask as usize] = choice;
    }

    let mut meld = Meld {
        points: best[full as usize].0,
        ..Default::default()
    };
    let mut mask = full;
    while mask != 0 {
        let (_, g) = best[mask as usize];
        if g == 0 {
            let low = mask.trailing_zeros() as usize;
            meld.leftover.push(cards[low]);
            mask &= !(1 << low);
        } else {
            let mut group: Vec<Card> = (0..n)
                .filter(|i| g & (1 << i) != 0)
                .map(|i| cards[i])
                .collect();
            sort_for_display(&mut group, wild);
            meld.groups.push(group);
            mask &= !g;
        }
    }
    sort_for_display(&mut meld.leftover, wild);
    meld
}

/// Naturals by suit then rank, wilds last.
pub fn sort_for_display(cards: &mut [Card], wild: u8) {
    cards.sort_by_key(|c| (c.is_wild(wild), c.suit, c.rank, c.id));
}

// ── Wire messages ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerView {
    pub id: u8,
    pub nickname: String,
    pub connected: bool,
    pub total: u32,
    /// Points scored in each finished round.
    pub rounds: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoundResult {
    pub player: u8,
    pub meld: Meld,
    pub went_out: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Stage {
    /// `drawn` is whether the current player has drawn yet this turn.
    Playing {
        drawn: bool,
    },
    RoundOver {
        results: Vec<RoundResult>,
    },
    GameOver {
        results: Vec<RoundResult>,
        standings: Vec<Standing>,
    },
}

/// The latest move at the table, so clients can animate it. `seq` counts up
/// with every move; a client that sees a new `seq` animates `kind`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Action {
    pub seq: u32,
    pub player: u8,
    pub kind: ActionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ActionKind {
    /// A fresh round was dealt.
    Deal,
    DrewDeck,
    DrewDiscard {
        card: Card,
    },
    Discarded {
        card: Card,
    },
}

/// Everything one player may see, sent after every change so a reconnecting
/// client needs nothing else.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameView {
    pub you: u8,
    pub host: u8,
    pub round: u8,
    pub wild: u8,
    pub dealer: u8,
    pub current: u8,
    pub hand: Vec<Card>,
    pub discard_top: Option<Card>,
    pub deck_size: u8,
    pub players: Vec<PlayerView>,
    /// Who went out first this round; everyone else then gets one last turn.
    pub out: Option<u8>,
    pub stage: Stage,
    /// Recent happenings, newest last.
    pub log: Vec<String>,
    pub last: Option<Action>,
}

pub type ServerMsg = crate::games::cards::ServerMsg<GameView>;

impl HandCard for Card {
    fn id(&self) -> u8 {
        self.id
    }
}

impl TableView for GameView {
    fn you(&self) -> u8 {
        self.you
    }

    fn last_move(&self) -> Option<(u32, bool)> {
        self.last.map(|a| (a.seq, a.kind == ActionKind::Deal))
    }

    fn placeholder() -> Self {
        GameView {
            you: 0,
            host: 0,
            round: 1,
            wild: 3,
            dealer: 0,
            current: 0,
            hand: Vec::new(),
            discard_top: None,
            deck_size: 0,
            players: Vec::new(),
            out: None,
            stage: Stage::Playing { drawn: false },
            log: Vec::new(),
            last: None,
        }
    }

    /// The discard pile's top card carries `data-card`, so cards moving
    /// between a hand and the pile slide on their own.
    fn motion(&self) -> Motion {
        let Some(a) = self.last else {
            return Motion::default();
        };
        let mine = a.player == self.you;
        match a.kind {
            ActionKind::Deal => Motion {
                arrive_from: Some(Spot::Pile("deck")),
                deal: true,
                ..Motion::default()
            },
            ActionKind::DrewDeck if mine => Motion {
                arrive_from: Some(Spot::Pile("deck")),
                ..Motion::default()
            },
            ActionKind::DrewDeck => Motion {
                leave: vec![(Spot::Pile("deck"), Some(a.player))],
                ..Motion::default()
            },
            ActionKind::DrewDiscard { card } if !mine => Motion {
                leave: vec![(Spot::Card(card.id), Some(a.player))],
                ..Motion::default()
            },
            ActionKind::Discarded { .. } if !mine => Motion {
                arrive_from: Some(Spot::Seat(a.player)),
                ..Motion::default()
            },
            _ => Motion::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ClientMsg {
    SetNickname {
        name: String,
    },
    StartGame,
    /// Take the top of the draw pile, or the top of the discard pile.
    Draw {
        from_discard: bool,
    },
    Discard {
        card: u8,
    },
    /// Discard `card` and lay the rest of the hand down in books and runs.
    GoOut {
        card: u8,
    },
    /// Reorder the hand; `order` lists every card id in the hand.
    Arrange {
        order: Vec<u8>,
    },
    NextRound,
    PlayAgain,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(rank: u8, suit: Suit) -> Card {
        Card {
            id: 0,
            rank,
            suit: Some(suit),
        }
    }

    fn joker() -> Card {
        Card {
            id: 0,
            rank: 0,
            suit: None,
        }
    }

    fn ids(mut cards: Vec<Card>) -> Vec<Card> {
        for (i, card) in cards.iter_mut().enumerate() {
            card.id = i as u8;
        }
        cards
    }

    use Suit::*;

    #[test]
    fn deck_has_116_cards() {
        let deck = full_deck();
        assert_eq!(deck.len(), 116);
        assert_eq!(deck.iter().filter(|c| c.is_joker()).count(), 6);
        assert!(deck.iter().enumerate().all(|(i, c)| c.id as usize == i));
    }

    #[test]
    fn groups_from_the_rules() {
        // Books and runs from the rulebook, with 3s wild unless noted.
        assert!(is_valid_group(&[c(8, Clubs), c(8, Stars), c(8, Spades)], 3));
        assert!(is_valid_group(
            &[c(13, Spades), c(13, Diamonds), c(13, Hearts), c(13, Hearts)],
            3
        ));
        assert!(is_valid_group(
            &[c(8, Spades), c(12, Spades), c(12, Stars)],
            8
        ));
        assert!(is_valid_group(
            &[c(8, Stars), c(9, Diamonds), c(9, Diamonds), c(9, Hearts)],
            8
        ));
        assert!(is_valid_group(&[c(5, Clubs), c(6, Clubs), c(7, Clubs)], 3));
        assert!(is_valid_group(
            &[c(9, Stars), c(10, Stars), c(11, Stars), c(12, Stars)],
            3
        ));
        assert!(is_valid_group(
            &[c(9, Diamonds), c(7, Clubs), c(11, Diamonds)],
            7
        ));
        assert!(is_valid_group(
            &[c(6, Diamonds), c(7, Stars), c(7, Spades), c(9, Diamonds)],
            7
        ));
        assert!(is_valid_group(&[joker(), joker(), c(4, Hearts)], 3));
    }

    #[test]
    fn invalid_groups() {
        assert!(!is_valid_group(&[c(8, Clubs), c(8, Stars)], 3));
        assert!(!is_valid_group(
            &[c(5, Clubs), c(6, Clubs), c(7, Hearts)],
            3
        ));
        assert!(!is_valid_group(&[c(5, Clubs), c(6, Clubs), c(9, Clubs)], 3));
        assert!(!is_valid_group(&[c(5, Clubs), c(5, Clubs), c(6, Clubs)], 3));
        assert!(!is_valid_group(
            &[c(5, Clubs), c(6, Hearts), c(9, Stars)],
            3
        ));
    }

    #[test]
    fn going_out_with_three_cards() {
        let hand = ids(vec![c(3, Hearts), c(7, Clubs), c(7, Stars)]);
        // 3s are wild in round one, so the 3 completes a book of sevens.
        assert_eq!(best_meld(&hand, 3).points, 0);
        // Without a wild it's two sevens and a three left over.
        assert_eq!(best_meld(&hand, 5).points, 17);
    }

    #[test]
    fn leftover_is_minimised() {
        // K♠ Q♠ J♠ 10♠ plus a lone joker and a 4: the run takes the joker
        // only if that saves points, and it does (joker is 50).
        let hand = ids(vec![
            c(13, Spades),
            c(12, Spades),
            c(11, Spades),
            joker(),
            c(4, Hearts),
        ]);
        let meld = best_meld(&hand, 5);
        assert_eq!(meld.points, 4);
        assert_eq!(meld.groups.len(), 1);
        assert_eq!(meld.groups[0].len(), 4);
    }

    #[test]
    fn splits_into_books_and_runs() {
        let hand = ids(vec![
            c(4, Hearts),
            c(5, Hearts),
            c(6, Hearts),
            c(9, Stars),
            c(9, Clubs),
            c(9, Diamonds),
            c(12, Spades),
            c(13, Spades),
            c(11, Spades),
            c(10, Spades),
        ]);
        let meld = best_meld(&hand, 3);
        assert_eq!(meld.points, 0);
        assert_eq!(meld.groups.len(), 3);
    }

    #[test]
    fn full_thirteen_card_hand_is_fast() {
        let deck = full_deck();
        let hand: Vec<Card> = deck.iter().step_by(9).take(14).copied().collect();
        let start = std::time::Instant::now();
        let _ = best_meld(&hand, 13);
        assert!(start.elapsed().as_millis() < 500);
    }
}
